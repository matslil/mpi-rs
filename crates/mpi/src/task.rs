//! Task handles, task context, and minimal spawn support.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use crate::call::{
    CallResponseMessage, CallSession, QueuedCallRelease, QueuedCallResponse,
    suspended_call_waiter_with_on_drop,
};
use crate::error::{CallError, RecvError, SendError};
use crate::message::{
    HasSessionId, LateReplyAction, LateReplyKind, LateReplyPolicy, LateReplyRef,
    QueueSpaceWakeupMessage, TaskMessage,
};
use crate::queue::{QueueSpaceWakeupTarget, TaskQueue};
use crate::scope::TaskScope;
use crate::session::{
    EndpointId, Response, SessionId, SessionIdAllocator, SyncReplySender, sync_reply_channel,
};
use crate::stream::{
    QueuedStreamEvent, StreamCancel, StreamControl, StreamEvent, StreamEventMessage,
    StreamEventSender, StreamPull, StreamSession, add_stream_credit, cancel_stream,
    forget_stream_credit, suspended_stream_waiter_with_on_drop,
};

static NEXT_ENDPOINT_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_endpoint_id() -> EndpointId {
    EndpointId(NEXT_ENDPOINT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Runtime endpoint shared by task handles and in-flight sessions.
///
/// The endpoint keeps the queue allocation alive while any handle or session can
/// still attempt delivery. Its lifecycle flag is separate from object lifetime:
/// an endpoint may remain allocated through reference counting after the task
/// has stopped accepting messages.
pub struct TaskEndpoint<M, const N: usize>
where
    M: TaskMessage,
{
    endpoint: EndpointId,
    queue: Arc<TaskQueue<M, N>>,
    accepting: AtomicBool,
    next_external_sequence: AtomicU64,
}

impl<M, const N: usize> TaskEndpoint<M, N>
where
    M: TaskMessage,
{
    /// Create an endpoint for an existing queue with a fresh endpoint ID.
    #[must_use]
    pub fn new(queue: Arc<TaskQueue<M, N>>) -> Self {
        Self::with_endpoint(queue, allocate_endpoint_id())
    }

    /// Create an endpoint for an existing queue and explicit endpoint ID.
    #[must_use]
    pub fn with_endpoint(queue: Arc<TaskQueue<M, N>>, endpoint: EndpointId) -> Self {
        Self {
            endpoint,
            queue,
            accepting: AtomicBool::new(true),
            next_external_sequence: AtomicU64::new(0),
        }
    }

    /// Return the logical endpoint ID.
    #[must_use]
    pub const fn endpoint(&self) -> EndpointId {
        self.endpoint
    }

    /// Return the underlying queue.
    #[must_use]
    pub fn queue(&self) -> &Arc<TaskQueue<M, N>> {
        &self.queue
    }

    /// Return whether the endpoint is still accepting messages.
    #[must_use]
    pub fn is_accepting(&self) -> bool {
        self.accepting.load(Ordering::Acquire)
    }

    /// Enqueue one message through this endpoint.
    pub fn send_message(&self, message: M) -> Result<(), SendError> {
        if !self.is_accepting() {
            return Err(SendError::TaskStopped);
        }

        self.queue.try_send(message)
    }

    /// Enqueue one task-internal message from another endpoint.
    pub fn send_message_from(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        if !self.is_accepting() {
            return Err(SendError::TaskStopped);
        }

        self.queue.try_send_from(sender, message)
    }

    /// Enqueue one task-internal message, waiting for a framework wakeup.
    pub fn send_message_from_waiting(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        if !self.is_accepting() {
            return Err(SendError::TaskStopped);
        }

        self.queue.send_waiting_from(sender, message)
    }

    /// Receive one message through this endpoint.
    pub fn recv_message(&self) -> Result<M, RecvError> {
        self.queue.recv()
    }

    /// Try to receive one message through this endpoint without blocking.
    #[must_use]
    pub fn try_recv_message(&self) -> Option<M> {
        self.queue.try_recv()
    }

    /// Allocate a session ID for an external blocking call.
    pub fn next_external_session_id(&self) -> SessionId {
        let sequence = self.next_external_sequence.fetch_add(1, Ordering::Relaxed);
        SessionId::new(self.endpoint, sequence)
    }

    /// Stop accepting messages and close the underlying queue.
    pub fn close(&self) {
        self.accepting.store(false, Ordering::Release);
        self.queue.close();
    }
}

/// Public send surface for a task.
pub struct TaskHandle<M, const N: usize>
where
    M: TaskMessage,
{
    endpoint: Arc<TaskEndpoint<M, N>>,
}

impl<M, const N: usize> Clone for TaskHandle<M, N>
where
    M: TaskMessage,
{
    fn clone(&self) -> Self {
        Self {
            endpoint: Arc::clone(&self.endpoint),
        }
    }
}

impl<M, const N: usize> TaskHandle<M, N>
where
    M: TaskMessage,
{
    /// Create a handle from an existing queue.
    #[must_use]
    pub fn new(queue: Arc<TaskQueue<M, N>>) -> Self {
        Self::from_endpoint(Arc::new(TaskEndpoint::new(queue)))
    }

    /// Create a handle from an existing queue and explicit endpoint ID.
    #[must_use]
    pub fn with_endpoint(queue: Arc<TaskQueue<M, N>>, endpoint: EndpointId) -> Self {
        Self::from_endpoint(Arc::new(TaskEndpoint::with_endpoint(queue, endpoint)))
    }

    /// Create a handle from a shared endpoint.
    #[must_use]
    pub fn from_endpoint(endpoint: Arc<TaskEndpoint<M, N>>) -> Self {
        Self { endpoint }
    }

    /// Return the shared endpoint object represented by this handle.
    #[must_use]
    pub fn task_endpoint(&self) -> &Arc<TaskEndpoint<M, N>> {
        &self.endpoint
    }

    /// Return the logical endpoint represented by this handle.
    #[must_use]
    pub fn endpoint(&self) -> EndpointId {
        self.endpoint.endpoint()
    }

    /// Return the underlying queue.
    #[must_use]
    pub fn queue(&self) -> &Arc<TaskQueue<M, N>> {
        self.endpoint.queue()
    }

    /// Return whether the target endpoint still accepts messages.
    #[must_use]
    pub fn is_accepting(&self) -> bool {
        self.endpoint.is_accepting()
    }

    /// Enqueue one already-constructed message.
    pub fn send_message(&self, message: M) -> Result<(), SendError> {
        self.endpoint.send_message(message)
    }

    /// Enqueue one task-internal message from another endpoint.
    pub fn send_message_from(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        self.endpoint.send_message_from(sender, message)
    }

    /// Enqueue one task-internal message, waiting for a framework wakeup.
    pub fn send_message_from_waiting(
        &self,
        sender: Arc<dyn QueueSpaceWakeupTarget>,
        message: M,
    ) -> Result<(), SendError> {
        self.endpoint.send_message_from_waiting(sender, message)
    }

    /// Receive one message from this handle's endpoint.
    pub fn recv_message(&self) -> Result<M, RecvError> {
        self.endpoint.recv_message()
    }

    /// Try to receive one message from this handle's endpoint without blocking.
    #[must_use]
    pub fn try_recv_message(&self) -> Option<M> {
        self.endpoint.try_recv_message()
    }

    /// Allocate a session ID for an external blocking call.
    pub fn next_external_session_id(&self) -> SessionId {
        self.endpoint.next_external_session_id()
    }

    /// Send a synchronous request from code that is outside the task model and
    /// block until exactly one typed response is returned.
    ///
    /// This is intentionally distinct from task-internal call APIs. It may block
    /// the current operating-system thread because external callers do not have a
    /// task queue or task-local receive state.
    pub fn call_blocking<R, F>(&self, make_message: F) -> Result<Response<R>, CallError>
    where
        R: Send + 'static,
        F: FnOnce(SessionId, SyncReplySender<R>) -> M,
    {
        let session_id = self.next_external_session_id();
        let (reply_tx, reply_rx) = sync_reply_channel();
        self.send_message(make_message(session_id, reply_tx))?;
        reply_rx.recv().map_err(|_| CallError::ReplyDisconnected)
    }

    /// Close the target task queue.
    pub fn close(&self) {
        self.endpoint.close();
    }
}

impl<M, const N: usize> QueueSpaceWakeupTarget for TaskHandle<M, N>
where
    M: TaskMessage + QueueSpaceWakeupMessage,
{
    fn endpoint(&self) -> EndpointId {
        TaskHandle::endpoint(self)
    }

    fn try_wake(&self) -> Result<(), SendError> {
        self.send_message(M::queue_space_wakeup())
    }
}

impl<M, const N: usize> TaskHandle<M, N>
where
    M: TaskMessage + QueueSpaceWakeupMessage,
{
    /// Return this handle as a framework queue-space wakeup target.
    #[must_use]
    pub fn queue_space_wakeup_target(&self) -> Arc<dyn QueueSpaceWakeupTarget> {
        Arc::new(self.clone())
    }
}

trait ErasedCallWaiter {
    fn deliver(self: Box<Self>, response: QueuedCallResponse) -> Result<(), CallError>;
}

struct TypedCallWaiter<T> {
    sender: Sender<Response<T>>,
}

impl<T: Send + 'static> ErasedCallWaiter for TypedCallWaiter<T> {
    fn deliver(self: Box<Self>, response: QueuedCallResponse) -> Result<(), CallError> {
        let value = response
            .value
            .downcast::<T>()
            .map_err(|_| CallError::UnexpectedReplyType)?;
        self.sender
            .send(Response::new(response.session_id, *value))
            .map_err(|_| CallError::ReplyDisconnected)
    }
}

trait ErasedStreamWaiter {
    fn deliver(&self, event: QueuedStreamEvent) -> Result<bool, SendError>;
}

struct TypedStreamWaiter<T, E> {
    sender: Sender<StreamEvent<T, E>>,
}

impl<T: Send + 'static, E: Send + 'static> ErasedStreamWaiter for TypedStreamWaiter<T, E> {
    fn deliver(&self, event: QueuedStreamEvent) -> Result<bool, SendError> {
        let event = event
            .event
            .downcast::<StreamEvent<T, E>>()
            .expect("queued stream event carried unexpected type");
        let finished = matches!(&*event, StreamEvent::End { .. } | StreamEvent::Error { .. });
        self.sender
            .send(*event)
            .map_err(|_| SendError::TaskStopped)?;
        Ok(finished)
    }
}

struct TaskContextState<M, const N: usize>
where
    M: TaskMessage,
{
    self_handle: TaskHandle<M, N>,
    session_ids: SessionIdAllocator,
    call_waiters: HashMap<SessionId, Box<dyn ErasedCallWaiter>>,
    stream_waiters: HashMap<SessionId, Box<dyn ErasedStreamWaiter>>,
    stream_credits: HashMap<SessionId, u32>,
    released_calls: HashSet<SessionId>,
    stopped: bool,
}

/// Read-only diagnostic snapshot of stream credit for one session.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct StreamCreditSnapshot {
    /// Stream session that currently has recorded credit.
    pub session_id: SessionId,

    /// Recorded item credit.
    pub credit: u32,
}

/// Read-only diagnostic snapshot of task-local runtime state.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskDiagnosticsSnapshot {
    /// Endpoint represented by this task context.
    pub endpoint: EndpointId,

    /// Whether the task has been asked to stop.
    pub stopped: bool,

    /// Sessions with active call waiters.
    pub active_call_waiters: Vec<SessionId>,

    /// Sessions with active stream waiters.
    pub active_stream_waiters: Vec<SessionId>,

    /// Recorded producer credit by stream session.
    pub stream_credits: Vec<StreamCreditSnapshot>,

    /// Released call sessions that have not yet been observed by the callee.
    pub released_calls: Vec<SessionId>,
}

/// Generated handler context state shared by task handlers.
///
/// The context uses task-local interior mutability. Generated handlers and the
/// main task loop are still single-threaded, but suspended handler futures may
/// retain owned capabilities derived from the context while the loop continues
/// to manage sessions and control state. Borrow violations indicate a runtime
/// implementation bug and are allowed to panic.
pub struct TaskContext<M, const N: usize>
where
    M: TaskMessage,
{
    inner: Rc<RefCell<TaskContextState<M, N>>>,
}

impl<M, const N: usize> Clone for TaskContext<M, N>
where
    M: TaskMessage,
{
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<M, const N: usize> TaskContext<M, N>
where
    M: TaskMessage,
{
    /// Create a task context for a task handle.
    #[must_use]
    pub fn new(self_handle: TaskHandle<M, N>) -> Self {
        let endpoint = self_handle.endpoint();
        Self {
            inner: Rc::new(RefCell::new(TaskContextState {
                self_handle,
                session_ids: SessionIdAllocator::new(endpoint),
                call_waiters: HashMap::new(),
                stream_waiters: HashMap::new(),
                stream_credits: HashMap::new(),
                released_calls: HashSet::new(),
                stopped: false,
            })),
        }
    }

    /// Return a clone of this task's own handle.
    #[must_use]
    pub fn self_handle(&self) -> TaskHandle<M, N> {
        self.inner.borrow().self_handle.clone()
    }

    /// Return this task context's endpoint.
    #[must_use]
    pub fn endpoint(&self) -> EndpointId {
        self.inner.borrow().self_handle.endpoint()
    }

    /// Allocate the next task-local session ID.
    pub fn next_session_id(&self) -> SessionId {
        self.inner.borrow_mut().session_ids.next_session_id()
    }

    /// Route a queued call response to the registered waiter, if any.
    pub fn deliver_call_response(
        &self,
        response: QueuedCallResponse,
    ) -> Result<LateReplyAction, CallError> {
        self.deliver_call_response_with_late_reply_handler(response, |_| LateReplyAction::Ignore)
    }

    /// Route a queued call response using an explicit late-reply handler.
    pub fn deliver_call_response_with_late_reply_handler<F>(
        &self,
        response: QueuedCallResponse,
        handler: F,
    ) -> Result<LateReplyAction, CallError>
    where
        F: FnOnce(LateReplyRef<'_>) -> LateReplyAction,
    {
        let waiter = self
            .inner
            .borrow_mut()
            .call_waiters
            .remove(&response.session_id);
        match waiter {
            Some(waiter) => {
                waiter.deliver(response)?;
                Ok(LateReplyAction::Ignore)
            }
            None if response.late_reply_policy == LateReplyPolicy::Ignore => {
                Ok(LateReplyAction::Ignore)
            }
            None => {
                let reply = LateReplyRef::new(
                    response.session_id,
                    LateReplyKind::CallResponse,
                    response.value.as_ref(),
                );
                let action = handler(reply);
                if action == LateReplyAction::Terminate {
                    self.stop();
                }
                Ok(action)
            }
        }
    }

    /// Record that a queued call has been released by its caller.
    pub fn record_call_release(&self, release: QueuedCallRelease) {
        self.inner
            .borrow_mut()
            .released_calls
            .insert(release.session_id);
    }

    /// Return whether a call was released, consuming the marker if present.
    pub fn take_call_released(&self, session_id: SessionId) -> bool {
        self.inner.borrow_mut().released_calls.remove(&session_id)
    }

    /// Record additional item credit for a stream producer.
    pub fn record_stream_pull(&self, pull: StreamPull) {
        if !add_stream_credit(pull) {
            return;
        }
        let mut state = self.inner.borrow_mut();
        let credit = state.stream_credits.entry(pull.session_id).or_insert(0);
        *credit = credit.saturating_add(pull.credit);
    }

    /// Record that a stream consumer has cancelled the session.
    pub fn record_stream_cancel(&self, cancel: StreamCancel) {
        self.inner
            .borrow_mut()
            .stream_credits
            .remove(&cancel.session_id);
        cancel_stream(cancel.session_id);
    }

    /// Consume one stream item credit if any has been granted.
    pub fn take_stream_credit(&self, session_id: SessionId) -> bool {
        let mut state = self.inner.borrow_mut();
        let Some(credit) = state.stream_credits.get_mut(&session_id) else {
            return false;
        };
        if *credit == 0 {
            return false;
        }
        *credit -= 1;
        if *credit == 0 {
            state.stream_credits.remove(&session_id);
        }
        true
    }

    /// Return currently recorded stream credit for diagnostics and tests.
    #[must_use]
    pub fn stream_credit(&self, session_id: SessionId) -> u32 {
        self.inner
            .borrow()
            .stream_credits
            .get(&session_id)
            .copied()
            .unwrap_or(0)
    }

    /// Return a read-only diagnostic snapshot of task-local runtime state.
    #[must_use]
    pub fn diagnostics_snapshot(&self) -> TaskDiagnosticsSnapshot {
        let state = self.inner.borrow();
        let mut active_call_waiters = state.call_waiters.keys().copied().collect::<Vec<_>>();
        active_call_waiters.sort_unstable();
        let mut active_stream_waiters = state.stream_waiters.keys().copied().collect::<Vec<_>>();
        active_stream_waiters.sort_unstable();
        let mut stream_credits = state
            .stream_credits
            .iter()
            .map(|(session_id, credit)| StreamCreditSnapshot {
                session_id: *session_id,
                credit: *credit,
            })
            .collect::<Vec<_>>();
        stream_credits.sort_unstable_by_key(|entry| entry.session_id);
        let mut released_calls = state.released_calls.iter().copied().collect::<Vec<_>>();
        released_calls.sort_unstable();

        TaskDiagnosticsSnapshot {
            endpoint: state.self_handle.endpoint(),
            stopped: state.stopped,
            active_call_waiters,
            active_stream_waiters,
            stream_credits,
            released_calls,
        }
    }

    /// Route a queued stream event to the registered stream waiter, if any.
    pub fn deliver_stream_event(
        &self,
        event: QueuedStreamEvent,
    ) -> Result<LateReplyAction, SendError> {
        self.deliver_stream_event_with_late_reply_handler(event, |_| LateReplyAction::Ignore)
    }

    /// Route a queued stream event using an explicit late-reply handler.
    pub fn deliver_stream_event_with_late_reply_handler<F>(
        &self,
        event: QueuedStreamEvent,
        handler: F,
    ) -> Result<LateReplyAction, SendError>
    where
        F: FnOnce(LateReplyRef<'_>) -> LateReplyAction,
    {
        let session_id = event.session_id;
        let mut state = self.inner.borrow_mut();
        let finished = match state.stream_waiters.get(&session_id) {
            Some(waiter) => waiter.deliver(event)?,
            None if event.late_reply_policy == LateReplyPolicy::Ignore => {
                return Ok(LateReplyAction::Ignore);
            }
            None => {
                drop(state);
                let reply =
                    LateReplyRef::new(session_id, LateReplyKind::StreamEvent, event.event.as_ref());
                let action = handler(reply);
                if action == LateReplyAction::Terminate {
                    self.stop();
                }
                return Ok(action);
            }
        };
        if finished {
            state.stream_waiters.remove(&session_id);
            state.stream_credits.remove(&session_id);
            forget_stream_credit(session_id);
        }
        Ok(LateReplyAction::Ignore)
    }

    /// Request that the task dispatch loop stops.
    pub fn stop(&self) {
        let mut state = self.inner.borrow_mut();
        state.stopped = true;
        state.self_handle.close();
    }

    /// Return whether the task has been asked to stop.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.inner.borrow().stopped
    }
}

impl<M, const N: usize> TaskContext<M, N>
where
    M: TaskMessage + CallResponseMessage + QueueSpaceWakeupMessage,
{
    /// Allocate one task-local call session and its queued reply sender.
    pub fn begin_call<T: Send + 'static>(&self) -> CallSession<T> {
        self.begin_call_with_late_reply_policy(LateReplyPolicy::Report)
    }

    /// Allocate one task-local call session with an explicit late-reply policy.
    pub fn begin_call_with_late_reply_policy<T: Send + 'static>(
        &self,
        late_reply_policy: LateReplyPolicy,
    ) -> CallSession<T> {
        let session_id = self.next_session_id();
        let inner = Rc::clone(&self.inner);
        let (waiter, future) = suspended_call_waiter_with_on_drop(session_id, move |session_id| {
            inner.borrow_mut().call_waiters.remove(&session_id);
        });
        self.inner.borrow_mut().call_waiters.insert(
            session_id,
            Box::new(TypedCallWaiter::<T> { sender: waiter }),
        );

        let self_handle = self.self_handle();
        let reply = SyncReplySender::new_with_sender(move |sender, response: Response<T>| {
            assert_eq!(
                response.session_id, session_id,
                "queued reply sender received response for wrong session"
            );
            let message = M::call_response_with_late_reply_policy(
                response.session_id,
                Box::new(response.value) as Box<dyn Any + Send>,
                late_reply_policy,
            );
            match sender {
                Some(sender) => self_handle.send_message_from_waiting(sender, message),
                None => self_handle.send_message(message),
            }
        });

        (session_id, reply, future)
    }
}

impl<M, const N: usize> TaskContext<M, N>
where
    M: TaskMessage + StreamEventMessage + QueueSpaceWakeupMessage,
{
    /// Allocate one task-local stream session and its queued event sender.
    pub fn begin_stream<T: Send + 'static, E: Send + 'static>(
        &self,
        control: Arc<dyn StreamControl>,
    ) -> StreamSession<T, E> {
        self.begin_stream_with_late_reply_policy(control, LateReplyPolicy::Report)
    }

    /// Allocate one task-local stream session with an explicit late-reply policy.
    pub fn begin_stream_with_late_reply_policy<T: Send + 'static, E: Send + 'static>(
        &self,
        control: Arc<dyn StreamControl>,
        late_reply_policy: LateReplyPolicy,
    ) -> StreamSession<T, E> {
        let session_id = self.next_session_id();
        let inner = Rc::clone(&self.inner);
        let (waiter, stream) =
            suspended_stream_waiter_with_on_drop(session_id, control, move |session_id| {
                let mut state = inner.borrow_mut();
                state.stream_waiters.remove(&session_id);
                state.stream_credits.remove(&session_id);
                forget_stream_credit(session_id);
            });
        self.inner.borrow_mut().stream_waiters.insert(
            session_id,
            Box::new(TypedStreamWaiter::<T, E> { sender: waiter }),
        );

        let self_handle = self.self_handle();
        let events = StreamEventSender::new_with_sender(move |sender, event: StreamEvent<T, E>| {
            let session_id = event.session_id();
            let message = M::stream_event_with_late_reply_policy(
                session_id,
                Box::new(event) as Box<dyn Any + Send>,
                late_reply_policy,
            );
            match sender {
                Some(sender) => self_handle.send_message_from_waiting(sender, message),
                None => self_handle.send_message(message),
            }
        });

        (session_id, events, stream)
    }
}

impl<M, const N: usize> TaskScope for TaskContext<M, N>
where
    M: TaskMessage + CallResponseMessage + StreamEventMessage + QueueSpaceWakeupMessage,
{
    type Message = M;

    fn endpoint(&self) -> EndpointId {
        TaskContext::endpoint(self)
    }

    fn begin_call<T: Send + 'static>(&mut self) -> CallSession<T> {
        TaskContext::begin_call::<T>(self)
    }

    fn begin_call_with_late_reply_policy<T: Send + 'static>(
        &mut self,
        late_reply_policy: LateReplyPolicy,
    ) -> CallSession<T> {
        TaskContext::begin_call_with_late_reply_policy::<T>(self, late_reply_policy)
    }

    fn begin_stream<T: Send + 'static, E: Send + 'static>(
        &mut self,
        control: Arc<dyn StreamControl>,
    ) -> StreamSession<T, E> {
        TaskContext::begin_stream::<T, E>(self, control)
    }

    fn begin_stream_with_late_reply_policy<T: Send + 'static, E: Send + 'static>(
        &mut self,
        control: Arc<dyn StreamControl>,
        late_reply_policy: LateReplyPolicy,
    ) -> StreamSession<T, E> {
        TaskContext::begin_stream_with_late_reply_policy::<T, E>(self, control, late_reply_policy)
    }
}

/// Join handle for a spawned task runtime.
pub struct TaskRuntime<T> {
    join: JoinHandle<T>,
}

impl<T> TaskRuntime<T> {
    /// Wait for the task thread to finish.
    pub fn join(self) -> Result<T, TaskJoinError> {
        self.join.join().map_err(|_| TaskJoinError)
    }
}

/// Error returned when a task thread panics before joining.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TaskJoinError;

impl core::fmt::Display for TaskJoinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("task thread panicked")
    }
}

impl std::error::Error for TaskJoinError {}

/// Spawn a minimal task loop with a start message already enqueued.
///
/// The start message is enqueued before the OS thread is spawned. If the start
/// message has priority placement, it is therefore guaranteed to be received as
/// the first application message.
pub fn spawn_task<M, T, F, const N: usize>(
    start_message: M,
    priority_reserved: usize,
    run: F,
) -> Result<(TaskHandle<M, N>, TaskRuntime<T>), SendError>
where
    M: TaskMessage,
    T: Send + 'static,
    F: FnOnce(TaskHandle<M, N>) -> T + Send + 'static,
{
    let endpoint = Arc::new(TaskEndpoint::new(Arc::new(
        TaskQueue::<M, N>::with_priority_reserved(priority_reserved),
    )));
    endpoint.send_message(start_message)?;
    let handle = TaskHandle::from_endpoint(endpoint);
    let runtime_handle = handle.clone();
    let join = thread::spawn(move || run(runtime_handle));
    Ok((handle, TaskRuntime { join }))
}
