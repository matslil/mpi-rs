use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};

use crate::error::SendError;
use crate::message::HasSessionId;
use crate::scope::TaskScope;
use crate::session::SessionId;

const STREAM_PULL_CREDIT: u32 = 1;
const STREAM_INITIAL_CREDIT: u32 = 64;
static STREAM_CREDITS: OnceLock<Mutex<HashMap<SessionId, u32>>> = OnceLock::new();

fn stream_credits() -> &'static Mutex<HashMap<SessionId, u32>> {
    STREAM_CREDITS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn add_stream_credit(pull: StreamPull) {
    let mut credits = stream_credits().lock().expect("stream credit table poisoned");
    let credit = credits.entry(pull.session_id).or_insert(0);
    *credit = credit.saturating_add(pull.credit);
}

pub fn consume_stream_credit(session_id: SessionId, count: usize) -> Result<(), SendError> {
    if count == 0 {
        return Ok(());
    }
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let mut credits = stream_credits().lock().expect("stream credit table poisoned");
    let Some(available) = credits.get_mut(&session_id) else {
        return Err(SendError::StreamFlowLimited);
    };
    if *available < count {
        return Err(SendError::StreamFlowLimited);
    }
    *available -= count;
    if *available == 0 {
        credits.remove(&session_id);
    }
    Ok(())
}

pub fn forget_stream_credit(session_id: SessionId) {
    stream_credits()
        .lock()
        .expect("stream credit table poisoned")
        .remove(&session_id);
}

#[must_use]
pub fn stream_credit(session_id: SessionId) -> u32 {
    stream_credits()
        .lock()
        .expect("stream credit table poisoned")
        .get(&session_id)
        .copied()
        .unwrap_or(0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamEvent<T, E> {
    Batch { session_id: SessionId, values: Vec<T> },
    End { session_id: SessionId },
    Error { session_id: SessionId, error: E },
}

impl<T, E> StreamEvent<T, E> {
    #[must_use]
    pub fn batch(session_id: SessionId, values: impl Into<Vec<T>>) -> Self {
        Self::Batch { session_id, values: values.into() }
    }

    #[must_use]
    pub const fn end(session_id: SessionId) -> Self {
        Self::End { session_id }
    }

    #[must_use]
    pub fn error(session_id: SessionId, error: E) -> Self {
        Self::Error { session_id, error }
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::End { .. } | Self::Error { .. })
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

pub struct QueuedStreamEvent {
    pub session_id: SessionId,
    pub event: Box<dyn Any + Send>,
}

impl QueuedStreamEvent {
    #[must_use]
    pub fn new(session_id: SessionId, event: Box<dyn Any + Send>) -> Self {
        Self { session_id, event }
    }
}

pub trait StreamEventMessage: Sized {
    fn stream_event(session_id: SessionId, event: Box<dyn Any + Send>) -> Self;
    fn into_stream_event(self) -> Result<QueuedStreamEvent, Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamPull {
    pub session_id: SessionId,
    pub credit: u32,
}

impl StreamPull {
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

pub trait StreamPullMessage: Sized {
    fn stream_pull(session_id: SessionId, credit: u32) -> Self;
    fn into_stream_pull(self) -> Result<StreamPull, Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCancel {
    pub session_id: SessionId,
}

impl StreamCancel {
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

pub trait StreamControl: Send + Sync + 'static {
    fn try_pull(&self, _session_id: SessionId, _credit: u32) -> Result<(), SendError> {
        Ok(())
    }

    fn try_cancel(&self, session_id: SessionId) -> Result<(), SendError>;
}

pub trait StreamEventSink<T, E> {
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

impl<T, E> StreamEventSink<T, E> for Box<dyn StreamEventSink<T, E> + Send> {
    fn send_event(&mut self, event: StreamEvent<T, E>) -> Result<(), SendError> {
        (**self).send_event(event)
    }
}

pub struct StreamEventSender<T, E> {
    sink: Box<dyn StreamEventSink<T, E> + Send>,
}

impl<T, E> StreamEventSender<T, E> {
    #[must_use]
    pub fn new(sink: Box<dyn StreamEventSink<T, E> + Send>) -> Self {
        Self { sink }
    }

    pub fn send(&mut self, event: StreamEvent<T, E>) -> Result<(), SendError> {
        if let StreamEvent::Batch { session_id, values } = &event {
            consume_stream_credit(*session_id, values.len())?;
        }
        let terminal = event.is_terminal();
        let session_id = event.session_id();
        let result = self.sink.send_event(event);
        if terminal {
            forget_stream_credit(session_id);
        }
        result
    }
}

pub type BoxStreamSink<T, E> = StreamSink<T, E, Box<dyn StreamEventSink<T, E> + Send>>;

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
    #[must_use]
    pub fn new(session_id: SessionId, batch_size: usize, sink: S) -> Self {
        assert!(batch_size > 0, "stream batch size must be greater than zero");
        Self {
            session_id,
            batch_size,
            sink,
            buffer: Vec::with_capacity(batch_size),
            finished: false,
            _error: PhantomData,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn push(&mut self, value: T) -> Result<(), SendError> {
        assert!(!self.finished, "cannot push to a finished stream");
        self.buffer.push(value);
        if self.buffer.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), SendError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let values = std::mem::take(&mut self.buffer);
        self.sink.send_event(StreamEvent::batch(self.session_id, values))
    }

    pub fn finish(&mut self) -> Result<(), SendError> {
        if self.finished {
            return Ok(());
        }
        self.flush()?;
        self.finished = true;
        self.sink.send_event(StreamEvent::end(self.session_id))
    }

    pub fn fail(&mut self, error: E) -> Result<(), SendError> {
        if self.finished {
            return Ok(());
        }
        self.flush()?;
        self.finished = true;
        self.sink.send_event(StreamEvent::error(self.session_id, error))
    }
}

pub struct MessageStream<T, E> {
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
    buffer: VecDeque<T>,
    finished: bool,
    _error: PhantomData<fn() -> E>,
}

impl<T, E> MessageStream<T, E> {
    #[must_use]
    pub fn new(session_id: SessionId, control: Arc<dyn StreamControl>) -> Self {
        let stream = Self {
            session_id,
            control,
            buffer: VecDeque::new(),
            finished: false,
            _error: PhantomData,
        };
        stream.grant_credit(STREAM_INITIAL_CREDIT);
        stream
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    fn grant_credit(&self, credit: u32) {
        let _ = self.control.try_pull(self.session_id, credit);
    }

    fn grant_one_credit(&self) {
        self.grant_credit(STREAM_PULL_CREDIT);
    }

    pub fn next_from_event(&mut self, event: StreamEvent<T, E>) -> Result<Option<T>, E> {
        if let Some(value) = self.buffer.pop_front() {
            self.grant_one_credit();
            return Ok(Some(value));
        }
        if self.finished {
            return Ok(None);
        }
        match event {
            StreamEvent::Batch { session_id, values } => {
                assert_eq!(session_id, self.session_id, "stream event session mismatch");
                self.buffer.extend(values);
                let value = self.buffer.pop_front();
                if value.is_some() {
                    self.grant_one_credit();
                }
                Ok(value)
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

    pub fn next_buffered(&mut self) -> Result<Option<T>, E> {
        if let Some(value) = self.buffer.pop_front() {
            self.grant_one_credit();
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

pub type StreamSession<T, E> = (
    SessionId,
    StreamEventSender<T, E>,
    SuspendedMessageStream<T, E>,
);

type StreamOnDrop = Box<dyn FnOnce(SessionId) + 'static>;

pub struct SuspendedMessageStream<T, E> {
    stream: MessageStream<T, E>,
    receiver: Receiver<StreamEvent<T, E>>,
    on_drop: Option<StreamOnDrop>,
}

impl<T, E> SuspendedMessageStream<T, E> {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        control: Arc<dyn StreamControl>,
        receiver: Receiver<StreamEvent<T, E>>,
    ) -> Self {
        Self {
            stream: MessageStream::new(session_id, control),
            receiver,
            on_drop: None,
        }
    }

    #[must_use]
    pub fn new_with_on_drop<F>(
        session_id: SessionId,
        control: Arc<dyn StreamControl>,
        receiver: Receiver<StreamEvent<T, E>>,
        on_drop: F,
    ) -> Self
    where
        F: FnOnce(SessionId) + 'static,
    {
        Self {
            stream: MessageStream::new(session_id, control),
            receiver,
            on_drop: Some(Box::new(on_drop)),
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.stream.session_id()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.stream.is_finished()
    }

    pub fn next<'a>(&'a mut self, _ctx: &mut impl TaskScope) -> SuspendedStreamNext<'a, T, E> {
        SuspendedStreamNext { stream: self }
    }

    fn disarm_on_drop(&mut self) {
        self.on_drop = None;
    }
}

impl<T, E> Drop for SuspendedMessageStream<T, E> {
    fn drop(&mut self) {
        if !self.stream.is_finished()
            && let Some(on_drop) = self.on_drop.take()
        {
            on_drop(self.stream.session_id());
        }
    }
}

pub struct SuspendedStreamNext<'a, T, E> {
    stream: &'a mut SuspendedMessageStream<T, E>,
}

impl<T, E> Future for SuspendedStreamNext<'_, T, E> {
    type Output = Result<Option<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(value) = this.stream.stream.next_buffered()? {
            return Poll::Ready(Ok(Some(value)));
        }
        if this.stream.stream.is_finished() {
            this.stream.disarm_on_drop();
            return Poll::Ready(Ok(None));
        }
        match this.stream.receiver.try_recv() {
            Ok(event) => {
                let result = this.stream.stream.next_from_event(event);
                if this.stream.stream.is_finished() {
                    this.stream.disarm_on_drop();
                }
                Poll::Ready(result)
            }
            Err(TryRecvError::Empty) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(TryRecvError::Disconnected) => {
                this.stream.disarm_on_drop();
                Poll::Ready(Ok(None))
            }
        }
    }
}

#[must_use]
pub fn suspended_stream_waiter<T, E>(
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
) -> (Sender<StreamEvent<T, E>>, SuspendedMessageStream<T, E>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (sender, SuspendedMessageStream::new(session_id, control, receiver))
}

#[must_use]
pub(crate) fn suspended_stream_waiter_with_on_drop<T, E, F>(
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
    on_drop: F,
) -> (Sender<StreamEvent<T, E>>, SuspendedMessageStream<T, E>)
where
    F: FnOnce(SessionId) + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    (
        sender,
        SuspendedMessageStream::new_with_on_drop(session_id, control, receiver, on_drop),
    )
}

pub struct BlockingMessageStream<T, E> {
    stream: MessageStream<T, E>,
    receiver: Receiver<StreamEvent<T, E>>,
}

impl<T, E> BlockingMessageStream<T, E> {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        control: Arc<dyn StreamControl>,
        receiver: Receiver<StreamEvent<T, E>>,
    ) -> Self {
        Self {
            stream: MessageStream::new(session_id, control),
            receiver,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.stream.session_id()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.stream.is_finished()
    }

    pub fn next_blocking(&mut self) -> Result<Option<T>, E> {
        if let Some(value) = self.stream.next_buffered()? {
            return Ok(Some(value));
        }
        if self.stream.is_finished() {
            return Ok(None);
        }
        match self.receiver.recv() {
            Ok(event) => self.stream.next_from_event(event),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingControl {
        pulls: Mutex<Vec<StreamPull>>,
        cancels: Mutex<Vec<SessionId>>,
    }

    impl RecordingControl {
        fn pulls(&self) -> Vec<StreamPull> {
            self.pulls.lock().expect("pulls lock poisoned").clone()
        }
    }

    impl StreamControl for RecordingControl {
        fn try_pull(&self, session_id: SessionId, credit: u32) -> Result<(), SendError> {
            self.pulls
                .lock()
                .expect("pulls lock poisoned")
                .push(StreamPull::new(session_id, credit));
            Ok(())
        }

        fn try_cancel(&self, session_id: SessionId) -> Result<(), SendError> {
            self.cancels
                .lock()
                .expect("cancels lock poisoned")
                .push(session_id);
            Ok(())
        }
    }

    #[test]
    fn stream_consumer_grants_credit_on_create_and_item_consumption() {
        let session_id = SessionId::new(crate::EndpointId(7), 3);
        let control = Arc::new(RecordingControl::default());
        let mut stream = MessageStream::<u32, String>::new(session_id, control.clone());

        assert_eq!(
            control.pulls(),
            vec![StreamPull::new(session_id, STREAM_INITIAL_CREDIT)],
        );
        assert_eq!(
            stream.next_from_event(StreamEvent::batch(session_id, vec![10, 11])),
            Ok(Some(10)),
        );
        assert_eq!(
            control.pulls(),
            vec![
                StreamPull::new(session_id, STREAM_INITIAL_CREDIT),
                StreamPull::new(session_id, 1),
            ],
        );
        assert_eq!(stream.next_buffered(), Ok(Some(11)));
        assert_eq!(
            control.pulls(),
            vec![
                StreamPull::new(session_id, STREAM_INITIAL_CREDIT),
                StreamPull::new(session_id, 1),
                StreamPull::new(session_id, 1),
            ],
        );
    }

    #[test]
    fn stream_event_sender_enforces_flow() {
        let session_id = SessionId::new(crate::EndpointId(8), 1);
        forget_stream_credit(session_id);
        let events = Arc::new(Mutex::new(Vec::<StreamEvent<u32, String>>::new()));
        let captured_events = events.clone();
        let mut sender = StreamEventSender::new(Box::new(move |event| {
            captured_events.lock().expect("events lock poisoned").push(event);
            Ok(())
        }));

        assert_eq!(
            sender.send(StreamEvent::batch(session_id, vec![1])),
            Err(SendError::StreamFlowLimited),
        );
        assert!(events.lock().expect("events lock poisoned").is_empty());

        add_stream_credit(StreamPull::new(session_id, 1));
        sender.send(StreamEvent::batch(session_id, vec![2])).unwrap();
        assert_eq!(stream_credit(session_id), 0);
        assert_eq!(
            events.lock().expect("events lock poisoned").as_slice(),
            &[StreamEvent::batch(session_id, vec![2])],
        );
    }
}
