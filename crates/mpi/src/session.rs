//! Session identifiers and typed call responses.

use core::fmt;

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
