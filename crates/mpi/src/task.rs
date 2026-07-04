//! Task handles, task context, and minimal spawn support.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use crate::call::{SuspendedCall, suspended_call_channel};
use crate::error::{CallError, SendError};
use crate::message::TaskMessage;
use crate::queue::TaskQueue;
use crate::session::{EndpointId, Response, SessionId, SessionIdAllocator, SyncReplySender, sync_reply_channel};

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

struct TaskContextState<M, const N: usize>
where
    M: TaskMessage,
{
    self_handle: TaskHandle<M, N>,
    session_ids: SessionIdAllocator,
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

    /// Allocate one task-local call session and its owned suspended future.
    pub fn begin_call<T: Send + 'static>(&self) -> (SessionId, SyncReplySender<T>, SuspendedCall<T>) {
        let session_id = self.next_session_id();
        let (reply, future) = suspended_call_channel(session_id);
        (session_id, reply, future)
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
