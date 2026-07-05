//! Message traits, placement definitions, and late-reply policy types.

use std::any::Any;

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
    /// Pass the late reply to the receiving task's late-reply handler.
    Report,

    /// Treat the late reply as an allowed obsolete protocol message.
    Ignore,
}

/// Kind of late reply observed by a receiving task.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LateReplyKind {
    /// Late one-shot call response.
    CallResponse,

    /// Late stream reply event.
    StreamEvent,
}

/// Action returned by a task's late-reply handler.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LateReplyAction {
    /// Ignore the late reply and continue running the task.
    Ignore,

    /// Stop the receiving task.
    Terminate,
}

/// Borrowed view of a late reply passed to a task late-reply handler.
pub struct LateReplyRef<'a> {
    session_id: SessionId,
    kind: LateReplyKind,
    payload: &'a (dyn Any + Send),
}

impl<'a> LateReplyRef<'a> {
    /// Construct a borrowed late-reply view.
    #[must_use]
    pub const fn new(
        session_id: SessionId,
        kind: LateReplyKind,
        payload: &'a (dyn Any + Send),
    ) -> Self {
        Self {
            session_id,
            kind,
            payload,
        }
    }

    /// Return the session ID carried by the late reply.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Return the late reply kind.
    #[must_use]
    pub const fn kind(&self) -> LateReplyKind {
        self.kind
    }

    /// Attempt to inspect the late reply payload as a concrete type.
    #[must_use]
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
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
