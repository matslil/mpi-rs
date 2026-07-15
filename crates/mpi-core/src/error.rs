//! Error types used by the runtime primitives.

use core::fmt;

use crate::lifecycle::TaskTermination;

/// Error returned when a message cannot be enqueued.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SendError {
    /// The target task queue has reached its compile-time capacity.
    QueueFull,

    /// The stream producer cannot send another item yet.
    StreamFlowLimited,

    /// The stream session has been cancelled by the consumer.
    StreamCancelled,

    /// The target task has stopped accepting messages.
    TaskStopped,
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => f.write_str("target task queue is full"),
            Self::StreamFlowLimited => f.write_str("stream producer cannot send another item yet"),
            Self::StreamCancelled => f.write_str("stream session has been cancelled"),
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

/// Error returned by synchronous calls.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CallError {
    /// The request could not be enqueued.
    Send(SendError),

    /// The callee stopped before sending a reply.
    ReplyDisconnected,

    /// The target endpoint terminated before sending a reply.
    TargetTerminated(TaskTermination),

    /// A queued reply carried a different type than the registered waiter.
    UnexpectedReplyType,
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Send(error) => write!(f, "call request could not be sent: {error}"),
            Self::ReplyDisconnected => f.write_str("call reply channel disconnected"),
            Self::TargetTerminated(termination) => {
                write!(f, "call target terminated: {termination:?}")
            }
            Self::UnexpectedReplyType => f.write_str("call reply carried unexpected type"),
        }
    }
}

impl std::error::Error for CallError {}

impl From<SendError> for CallError {
    fn from(error: SendError) -> Self {
        Self::Send(error)
    }
}
