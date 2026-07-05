//! Message traits and placement definitions.

use crate::session::SessionId;

/// Placement class for a message in the receiving task's queue.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MessagePlacement {
    /// Default FIFO queue.
    Normal,

    /// Priority FIFO queue, received before normal messages.
    Priority,
}

/// Runtime policy for protocol replies that arrive after their session has no
/// active waiter.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LateReplyPolicy {
    /// Surface the late reply to the task's fallback policy/diagnostics state.
    Report,

    /// Treat the late reply as an allowed obsolete protocol message.
    Ignore,
}

/// Trait implemented by message enums that can be stored in a task queue.
pub trait TaskMessage: Sized + Send + 'static {
    /// Return the receiver-declared placement for this message.
    fn placement(&self) -> MessagePlacement;
}

/// Marker/conversion trait used for compile-time receive checks.
pub trait CanReceive<T>: TaskMessage {
    /// Wrap a received protocol value into the task's message enum.
    fn wrap(value: T) -> Self;
}

/// Trait for protocol messages that belong to a logical session.
pub trait HasSessionId {
    /// Return the logical interaction identifier carried by this message.
    fn session_id(&self) -> SessionId;
}
