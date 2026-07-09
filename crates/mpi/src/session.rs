//! Session identifiers and typed call responses.

use core::fmt;
use std::sync::mpsc;

use crate::error::SendError;
use crate::message::HasSessionId;

/// Identifies the task or external endpoint that allocated a session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EndpointId(pub u64);

impl fmt::Display for EndpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "endpoint:{}", self.0)
    }
}

/// Logical interaction identifier for calls, streams, cancellation, matching,
/// tracing, and debugging.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SessionId {
    /// Endpoint that created the session.
    pub origin: EndpointId,

    /// Sequence local to `origin`.
    pub sequence: u64,
}

impl SessionId {
    /// Construct a session identifier from origin and sequence.
    #[must_use]
    pub const fn new(origin: EndpointId, sequence: u64) -> Self {
        Self { origin, sequence }
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.origin, self.sequence)
    }
}

/// Allocates task-local session IDs without global synchronization.
#[derive(Debug, Clone)]
pub struct SessionIdAllocator {
    origin: EndpointId,
    next_sequence: u64,
}

impl SessionIdAllocator {
    /// Create a new allocator for one endpoint.
    #[must_use]
    pub const fn new(origin: EndpointId) -> Self {
        Self {
            origin,
            next_sequence: 0,
        }
    }

    /// Return the allocator's endpoint.
    #[must_use]
    pub const fn origin(&self) -> EndpointId {
        self.origin
    }

    /// Allocate the next session ID.
    pub fn next_session_id(&mut self) -> SessionId {
        let session_id = SessionId::new(self.origin, self.next_sequence);
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("session sequence exhausted");
        session_id
    }
}

/// Typed response wrapper for synchronous calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response<T> {
    /// Session completed by this response.
    pub session_id: SessionId,

    /// Reply payload.
    pub value: T,
}

impl<T> Response<T> {
    /// Construct a typed response.
    #[must_use]
    pub const fn new(session_id: SessionId, value: T) -> Self {
        Self { session_id, value }
    }

    /// Return the response session ID.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Consume the response and return the payload.
    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> HasSessionId for Response<T> {
    fn session_id(&self) -> SessionId {
        self.session_id
    }
}

type ReplySendFn<T> =
    Box<dyn FnOnce(Option<EndpointId>, Response<T>) -> Result<(), SendError> + Send + 'static>;

/// Sender endpoint used by call handlers to return one typed reply.
///
/// The sender is intentionally a one-shot abstraction. External calls back it
/// with an `mpsc` channel; task-internal suspended calls back it with a queued
/// response message to the caller task.
pub struct SyncReplySender<T> {
    send: Option<ReplySendFn<T>>,
}

impl<T> SyncReplySender<T> {
    /// Create a reply sender from a one-shot send function.
    #[must_use]
    pub fn new<F>(send: F) -> Self
    where
        F: FnOnce(Response<T>) -> Result<(), SendError> + Send + 'static,
    {
        Self::new_with_sender(move |_sender, response| send(response))
    }

    /// Create a reply sender from a one-shot send function that can observe the
    /// task endpoint that produced a task-internal reply.
    #[must_use]
    pub fn new_with_sender<F>(send: F) -> Self
    where
        F: FnOnce(Option<EndpointId>, Response<T>) -> Result<(), SendError> + Send + 'static,
    {
        Self {
            send: Some(Box::new(send)),
        }
    }

    /// Send the reply payload.
    pub fn send(mut self, response: Response<T>) -> Result<(), SendError> {
        let send = self.send.take().expect("reply sender used after send");
        send(None, response)
    }

    /// Send a task-internal reply produced by the given endpoint.
    pub fn send_from(mut self, sender: EndpointId, response: Response<T>) -> Result<(), SendError> {
        let send = self.send.take().expect("reply sender used after send");
        send(Some(sender), response)
    }
}

/// Receiver endpoint used by callers waiting for one typed reply.
pub type SyncReplyReceiver<T> = mpsc::Receiver<Response<T>>;

/// Create a synchronous reply channel for external blocking calls.
#[must_use]
pub fn sync_reply_channel<T: Send + 'static>() -> (SyncReplySender<T>, SyncReplyReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    (
        SyncReplySender::new(move |response| {
            sender.send(response).map_err(|_| SendError::TaskStopped)
        }),
        receiver,
    )
}
