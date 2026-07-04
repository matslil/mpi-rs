//! Task handles, task context, and minimal spawn support.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use crate::call::{
    CallResponseMessage, CallSession, QueuedCallRelease, QueuedCallResponse,
    suspended_call_waiter_with_on_drop,
};
use crate::error::{CallError, SendError};
use crate::message::{HasSessionId, TaskMessage};
use crate::queue::TaskQueue;
use crate::session::{
    EndpointId, Response, SessionId, SessionIdAllocator, SyncReplySender, sync_reply_channel,
};
use crate::stream::{
    QueuedStreamEvent, StreamControl, StreamEvent, StreamEventMessage, StreamEventSender,
    StreamSession, suspended_stream_waiter_with_on_drop,
};

static NEXT_ENDPOINT_ID: AtomicU64 = AtomicU64::new(1);

fn allocate_endpoint_id() -> EndpointId {
    EndpointId(NEXT_ENDPOINT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Public send surface for a task.
pub struct TaskHandle<M, const N: usize>
where
    M: TaskMessage,
{
    endpoint: EndpointId,
    queue: Arc<TaskQueue<M, N>>,
    next_external_sequence: Arc<AtomicU64>,
}

impl<M, const N: usize> Clone for TaskHandle<M, N>
where
    M: TaskMessage,
{
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint,
            queue: Arc::clone(&self.queue),
            next_external_sequence: Arc::clone(&self.next_external_sequence),
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
        Self::with_endpoint(queue, allocate_endpoint_id())
    }

    /// Create a handle from an existing queue and explicit endpoint ID.
    #[must_use]
    pub fn with_endpoint(queue: Arc<TaskQueue<M, N>>, endpoint: EndpointId) -> Self {
        Self {
            endpoint,
            queue,
            next_external_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Return the logical endpoint represented by this handle.
    #[must_use]
    pub const fn endpoint(&self) -> EndpointId {
        self.endpoint
    }

    /// Return the underlying queue.
    #[must_use]
    pub fn queue(&self) -> &Arc<TaskQueue<M, N>> {
        &self.queue
    }

    /// Enqueue one already-constructed message.
    pub fn send_message(&self, message: M) -> Result<(), SendError> {
        self.queue.try_send(message)
    }

    /// Allocate a session ID for an external blocking call.
    pub fn next_external_session_id(&self) -> SessionId {
        let sequence = self.next_external_sequence.fetch_add(1, Ordering::Relaxed);
        SessionId::new(self.endpoint, sequence)
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
        self.queue.close();
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
    released_calls: HashSet<SessionId>,
    stopped: bool,
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

    /// Allocate the next task-local session ID.
    pub fn next_session_id(&self) -> SessionId {
        self.inner.borrow_mut().session_ids.next_session_id()
    }

    /// Route a queued call response to the registered waiter, if any.
    pub fn deliver_call_response(&self, response: QueuedCallResponse) -> Result<(), CallError> {
        let waiter = self
            .inner
            .borrow_mut()
            .call_waiters
            .remove(&response.session_id);
        match waiter {
            Some(waiter) => waiter.deliver(response),
            None => Ok(()),
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

    /// Route a queued stream event to the registered stream waiter, if any.
    pub fn deliver_stream_event(&self, event: QueuedStreamEvent) -> Result<(), SendError> {
        let session_id = event.session_id;
        let mut state = self.inner.borrow_mut();
        let finished = match state.stream_waiters.get(&session_id) {
            Some(waiter) => waiter.deliver(event)?,
            None => return Ok(()),
        };
        if finished {
            state.stream_waiters.remove(&session_id);
        }
        Ok(())
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
    M: TaskMessage + CallResponseMessage,
{
    /// Allocate one task-local call session and its queued reply sender.
    pub fn begin_call<T: Send + 'static>(&self) -> CallSession<T> {
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
        let reply = SyncReplySender::new(move |response: Response<T>| {
            assert_eq!(
                response.session_id, session_id,
                "queued reply sender received response for wrong session"
            );
            self_handle.send_message(M::call_response(
                response.session_id,
                Box::new(response.value) as Box<dyn Any + Send>,
            ))
        });

        (session_id, reply, future)
    }
}

impl<M, const N: usize> TaskContext<M, N>
where
    M: TaskMessage + StreamEventMessage,
{
    /// Allocate one task-local stream session and its queued event sender.
    pub fn begin_stream<T: Send + 'static, E: Send + 'static>(
        &self,
        control: Arc<dyn StreamControl>,
    ) -> StreamSession<T, E> {
        let session_id = self.next_session_id();
        let inner = Rc::clone(&self.inner);
        let (waiter, stream) =
            suspended_stream_waiter_with_on_drop(session_id, control, move |session_id| {
                inner.borrow_mut().stream_waiters.remove(&session_id);
            });
        self.inner.borrow_mut().stream_waiters.insert(
            session_id,
            Box::new(TypedStreamWaiter::<T, E> { sender: waiter }),
        );

        let self_handle = self.self_handle();
        let events = StreamEventSender::new(Box::new(move |event: StreamEvent<T, E>| {
            let session_id = event.session_id();
            self_handle.send_message(M::stream_event(
                session_id,
                Box::new(event) as Box<dyn Any + Send>,
            ))
        }));

        (session_id, events, stream)
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
    run: F,
) -> Result<(TaskHandle<M, N>, TaskRuntime<T>), SendError>
where
    M: TaskMessage,
    T: Send + 'static,
    F: FnOnce(TaskHandle<M, N>) -> T + Send + 'static,
{
    let queue = Arc::new(TaskQueue::<M, N>::new());
    queue.try_send(start_message)?;
    let handle = TaskHandle::new(Arc::clone(&queue));
    let runtime_handle = handle.clone();
    let join = thread::spawn(move || run(runtime_handle));
    Ok((handle, TaskRuntime { join }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::CallReleaseMessage;
    use crate::message::MessagePlacement;

    enum TestMessage {
        CallResponse {
            session_id: SessionId,
            value: Box<dyn Any + Send>,
        },
        CallRelease {
            session_id: SessionId,
        },
        StreamEvent {
            session_id: SessionId,
            event: Box<dyn Any + Send>,
        },
    }

    impl TaskMessage for TestMessage {
        fn placement(&self) -> MessagePlacement {
            MessagePlacement::Priority
        }
    }

    impl CallResponseMessage for TestMessage {
        fn call_response(session_id: SessionId, value: Box<dyn Any + Send>) -> Self {
            Self::CallResponse { session_id, value }
        }

        fn into_call_response(self) -> Result<QueuedCallResponse, Self> {
            match self {
                Self::CallResponse { session_id, value } => {
                    Ok(QueuedCallResponse::new(session_id, value))
                }
                other => Err(other),
            }
        }
    }

    impl CallReleaseMessage for TestMessage {
        fn call_release(session_id: SessionId) -> Self {
            Self::CallRelease { session_id }
        }

        fn into_call_release(self) -> Result<QueuedCallRelease, Self> {
            match self {
                Self::CallRelease { session_id } => Ok(QueuedCallRelease::new(session_id)),
                other => Err(other),
            }
        }
    }

    impl StreamEventMessage for TestMessage {
        fn stream_event(session_id: SessionId, event: Box<dyn Any + Send>) -> Self {
            Self::StreamEvent { session_id, event }
        }

        fn into_stream_event(self) -> Result<QueuedStreamEvent, Self> {
            match self {
                Self::StreamEvent { session_id, event } => {
                    Ok(QueuedStreamEvent::new(session_id, event))
                }
                other => Err(other),
            }
        }
    }

    struct TestControl;

    impl StreamControl for TestControl {
        fn try_cancel(&self, _session_id: SessionId) -> Result<(), SendError> {
            Ok(())
        }
    }

    fn context() -> TaskContext<TestMessage, 4> {
        let queue = Arc::new(TaskQueue::<TestMessage, 4>::new());
        TaskContext::new(TaskHandle::new(queue))
    }

    #[test]
    fn dropped_suspended_call_removes_registered_waiter() {
        let ctx = context();
        let (_session_id, _reply, future) = ctx.begin_call::<u32>();
        assert_eq!(ctx.inner.borrow().call_waiters.len(), 1);

        drop(future);

        assert_eq!(ctx.inner.borrow().call_waiters.len(), 0);
    }

    #[test]
    fn call_release_marker_is_consumed_once() {
        let ctx = context();
        let session_id = SessionId::new(ctx.self_handle().endpoint(), 9);

        ctx.record_call_release(QueuedCallRelease::new(session_id));

        assert!(ctx.take_call_released(session_id));
        assert!(!ctx.take_call_released(session_id));
    }

    #[test]
    fn dropped_suspended_stream_removes_registered_waiter() {
        let ctx = context();
        let (_session_id, _events, stream) = ctx.begin_stream::<u32, String>(Arc::new(TestControl));
        assert_eq!(ctx.inner.borrow().stream_waiters.len(), 1);

        drop(stream);

        assert_eq!(ctx.inner.borrow().stream_waiters.len(), 0);
    }
}
