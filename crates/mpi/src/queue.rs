//! Bounded task queue with normal and priority FIFO placement.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

use crate::error::{RecvError, SendError};
use crate::message::{MessagePlacement, TaskMessage};

#[derive(Debug)]
struct QueueState<M> {
    priority: VecDeque<M>,
    normal: VecDeque<M>,
    closed: bool,
}

impl<M> Default for QueueState<M> {
    fn default() -> Self {
        Self {
            priority: VecDeque::new(),
            normal: VecDeque::new(),
            closed: false,
        }
    }
}

/// A bounded queue owned by a task.
///
/// The queue has one compile-time capacity shared by the normal and priority
/// internal FIFO queues. Message placement is read from `TaskMessage`, so the
/// sender cannot override the receiver's placement declaration.
pub struct TaskQueue<M, const N: usize>
where
    M: TaskMessage,
{
    state: Mutex<QueueState<M>>,
    available: Condvar,
}

/// Read-only diagnostic snapshot of a task queue.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TaskQueueSnapshot {
    /// Compile-time total queue capacity.
    pub capacity: usize,

    /// Total number of queued messages.
    pub total_len: usize,

    /// Number of queued priority messages.
    pub priority_len: usize,

    /// Number of queued normal messages.
    pub normal_len: usize,

    /// Whether the queue has been closed.
    pub closed: bool,
}

impl<M, const N: usize> Default for TaskQueue<M, N>
where
    M: TaskMessage,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M, const N: usize> TaskQueue<M, N>
where
    M: TaskMessage,
{
    /// Create an empty open queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(QueueState::default()),
            available: Condvar::new(),
        }
    }

    /// Return the compile-time capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Return the total number of queued messages.
    #[must_use]
    pub fn len(&self) -> usize {
        let state = self.state.lock().expect("queue mutex poisoned");
        state.priority.len() + state.normal.len()
    }

    /// Return `true` when no messages are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return `true` when the shared capacity is exhausted.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() == N
    }

    /// Return a read-only diagnostic snapshot of the queue state.
    #[must_use]
    pub fn snapshot(&self) -> TaskQueueSnapshot {
        let state = self.state.lock().expect("queue mutex poisoned");
        let priority_len = state.priority.len();
        let normal_len = state.normal.len();
        TaskQueueSnapshot {
            capacity: N,
            total_len: priority_len + normal_len,
            priority_len,
            normal_len,
            closed: state.closed,
        }
    }

    /// Close the queue and wake blocked receivers.
    pub fn close(&self) {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        state.closed = true;
        self.available.notify_all();
    }

    /// Try to enqueue one message.
    ///
    /// This method never blocks. It returns `SendError::QueueFull` if the shared
    /// normal/priority capacity has been reached.
    pub fn try_send(&self, message: M) -> Result<(), SendError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");

        if state.closed {
            return Err(SendError::TaskStopped);
        }

        if state.priority.len() + state.normal.len() >= N {
            return Err(SendError::QueueFull);
        }

        match message.placement() {
            MessagePlacement::Normal => state.normal.push_back(message),
            MessagePlacement::Priority => state.priority.push_back(message),
        }

        drop(state);
        self.available.notify_one();
        Ok(())
    }

    /// Receive one message, blocking until a message is available or the queue closes.
    pub fn recv(&self) -> Result<M, RecvError> {
        let mut state = self.state.lock().expect("queue mutex poisoned");

        loop {
            if let Some(message) = state.priority.pop_front() {
                return Ok(message);
            }

            if let Some(message) = state.normal.pop_front() {
                return Ok(message);
            }

            if state.closed {
                return Err(RecvError::Closed);
            }

            state = self.available.wait(state).expect("queue mutex poisoned");
        }
    }

    /// Try to receive one message without blocking.
    #[must_use]
    pub fn try_recv(&self) -> Option<M> {
        let mut state = self.state.lock().expect("queue mutex poisoned");
        state
            .priority
            .pop_front()
            .or_else(|| state.normal.pop_front())
    }
}
