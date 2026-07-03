//! Error types used by the runtime primitives.

use core::fmt;

/// Error returned when a message cannot be enqueued.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SendError {
    /// The target task queue has reached its compile-time capacity.
    QueueFull,

    /// The target task has stopped accepting messages.
    TaskStopped,
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => f.write_str("target task queue is full"),
            Self::TaskStopped => f.write_str("target task has stopped"),
        }
    }
}

impl std::error::Error for SendError {}

/// Error returned when receiving from a task queue fails.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RecvError {
    /// The queue was closed and no further messages are available.
    Closed,
}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("queue is closed"),
        }
    }
}

impl std::error::Error for RecvError {}
