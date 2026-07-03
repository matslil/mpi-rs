//! Task handles and minimal spawn support.

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::error::SendError;
use crate::message::TaskMessage;
use crate::queue::TaskQueue;

/// Public send surface for a task.
pub struct TaskHandle<M, const N: usize>
where
    M: TaskMessage,
{
    queue: Arc<TaskQueue<M, N>>,
}

impl<M, const N: usize> Clone for TaskHandle<M, N>
where
    M: TaskMessage,
{
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
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
        Self { queue }
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

    /// Close the target task queue.
    pub fn close(&self) {
        self.queue.close();
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
