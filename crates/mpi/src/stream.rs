//! Stream protocol messages and consumer/producer helpers.

use std::collections::VecDeque;
use std::marker::PhantomData;
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
    pub fn error(session_id: SessionId, error: E) -> Self {
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

/// Receives producer-side stream events from a `StreamSink`.
pub trait StreamEventSink<T, E> {
    /// Send one stream event to the consumer side.
    fn send_event(&mut self, event: StreamEvent<T, E>) -> Result<(), SendError>;
}

impl<T, E, F> StreamEventSink<T, E> for F
where
    F: FnMut(StreamEvent<T, E>) -> Result<(), SendError>,
{
    fn send_event(&mut self, event: StreamEvent<T, E>) -> Result<(), SendError> {
        self(event)
    }
}

/// Producer-side stream helper that batches items and emits end/error events.
pub struct StreamSink<T, E, S>
where
    S: StreamEventSink<T, E>,
{
    session_id: SessionId,
    batch_size: usize,
    sink: S,
    buffer: Vec<T>,
    finished: bool,
    _error: PhantomData<fn() -> E>,
}

impl<T, E, S> StreamSink<T, E, S>
where
    S: StreamEventSink<T, E>,
{
    /// Create a stream sink with the given positive batch size.
    #[must_use]
    pub fn new(session_id: SessionId, batch_size: usize, sink: S) -> Self {
        assert!(
            batch_size > 0,
            "stream batch size must be greater than zero"
        );
        Self {
            session_id,
            batch_size,
            sink,
            buffer: Vec::with_capacity(batch_size),
            finished: false,
            _error: PhantomData,
        }
    }

    /// Return this stream's session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Push one item, flushing a batch when the configured batch size is reached.
    pub fn push(&mut self, value: T) -> Result<(), SendError> {
        assert!(!self.finished, "cannot push to a finished stream");
        self.buffer.push(value);
        if self.buffer.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    /// Flush a non-empty batch.
    pub fn flush(&mut self) -> Result<(), SendError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let values = std::mem::take(&mut self.buffer);
        self.sink
            .send_event(StreamEvent::batch(self.session_id, values))
    }

    /// Flush any buffered items and send normal stream end.
    pub fn finish(&mut self) -> Result<(), SendError> {
        if self.finished {
            return Ok(());
        }
        self.flush()?;
        self.finished = true;
        self.sink.send_event(StreamEvent::end(self.session_id))
    }

    /// Flush any buffered items and send stream error.
    pub fn fail(&mut self, error: E) -> Result<(), SendError> {
        if self.finished {
            return Ok(());
        }
        self.flush()?;
        self.finished = true;
        self.sink
            .send_event(StreamEvent::error(self.session_id, error))
    }
}

/// Consumer-side stream helper that hides batching.
pub struct MessageStream<T, E> {
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
    buffer: VecDeque<T>,
    finished: bool,
    _error: PhantomData<fn() -> E>,
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
            _error: PhantomData,
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
