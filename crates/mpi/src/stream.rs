//! Stream protocol messages and consumer helper.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::error::SendError;
use crate::message::HasSessionId;
use crate::session::SessionId;

/// Event sent from a stream producer to a stream consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamEvent<T, E> {
    /// A batch of stream items for one session.
    Batch {
        /// Stream session.
        session_id: SessionId,
        /// Batched item values.
        values: Vec<T>,
    },

    /// Normal end of stream for one session.
    End {
        /// Stream session.
        session_id: SessionId,
    },

    /// Stream failure for one session.
    Error {
        /// Stream session.
        session_id: SessionId,
        /// Producer error.
        error: E,
    },
}

impl<T, E> StreamEvent<T, E> {
    /// Construct a batch event.
    #[must_use]
    pub fn batch(session_id: SessionId, values: impl Into<Vec<T>>) -> Self {
        Self::Batch {
            session_id,
            values: values.into(),
        }
    }

    /// Construct an end event.
    #[must_use]
    pub const fn end(session_id: SessionId) -> Self {
        Self::End { session_id }
    }

    /// Construct an error event.
    #[must_use]
    pub const fn error(session_id: SessionId, error: E) -> Self {
        Self::Error { session_id, error }
    }
}

impl<T, E> HasSessionId for StreamEvent<T, E> {
    fn session_id(&self) -> SessionId {
        match self {
            Self::Batch { session_id, .. }
            | Self::End { session_id }
            | Self::Error { session_id, .. } => *session_id,
        }
    }
}

/// Hidden stream pull control message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamPull {
    /// Stream session.
    pub session_id: SessionId,

    /// Additional item credit granted by the consumer.
    pub credit: u32,
}

impl StreamPull {
    /// Construct a pull control message.
    #[must_use]
    pub const fn new(session_id: SessionId, credit: u32) -> Self {
        Self { session_id, credit }
    }
}

impl HasSessionId for StreamPull {
    fn session_id(&self) -> SessionId {
        self.session_id
    }
}

/// Hidden stream cancel control message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCancel {
    /// Stream session.
    pub session_id: SessionId,
}

impl StreamCancel {
    /// Construct a stream cancellation message.
    #[must_use]
    pub const fn new(session_id: SessionId) -> Self {
        Self { session_id }
    }
}

impl HasSessionId for StreamCancel {
    fn session_id(&self) -> SessionId {
        self.session_id
    }
}

/// Sends cancellation for a live stream.
pub trait StreamControl: Send + Sync + 'static {
    /// Try to cancel a stream session.
    fn try_cancel(&self, session_id: SessionId) -> Result<(), SendError>;
}

/// Consumer-side stream helper that hides batching.
pub struct MessageStream<T, E> {
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
    buffer: VecDeque<T>,
    finished: bool,
}

impl<T, E> MessageStream<T, E> {
    /// Construct a stream object.
    #[must_use]
    pub fn new(session_id: SessionId, control: Arc<dyn StreamControl>) -> Self {
        Self {
            session_id,
            control,
            buffer: VecDeque::new(),
            finished: false,
        }
    }

    /// Return this stream's logical session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Return whether the stream has reached end or error.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// Consume one incoming stream event and return at most one item.
    ///
    /// This is the synchronous core of the future `next(ctx).await` API. It
    /// drains the local buffer before using another incoming event.
    pub fn next_from_event(&mut self, event: StreamEvent<T, E>) -> Result<Option<T>, E> {
        if let Some(value) = self.buffer.pop_front() {
            return Ok(Some(value));
        }

        if self.finished {
            return Ok(None);
        }

        match event {
            StreamEvent::Batch { session_id, values } => {
                assert_eq!(session_id, self.session_id, "stream event session mismatch");
                self.buffer.extend(values);
                Ok(self.buffer.pop_front())
            }
            StreamEvent::End { session_id } => {
                assert_eq!(session_id, self.session_id, "stream event session mismatch");
                self.finished = true;
                Ok(None)
            }
            StreamEvent::Error { session_id, error } => {
                assert_eq!(session_id, self.session_id, "stream event session mismatch");
                self.finished = true;
                Err(error)
            }
        }
    }

    /// Return the next buffered item, without consuming a new event.
    pub fn next_buffered(&mut self) -> Result<Option<T>, E> {
        if let Some(value) = self.buffer.pop_front() {
            return Ok(Some(value));
        }

        Ok(None)
    }
}

impl<T, E> Drop for MessageStream<T, E> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.control.try_cancel(self.session_id);
        }
    }
}
