//! Stream protocol messages and consumer/producer helpers.

use ctx_future::{CtxFuture, CtxPoll};
use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};

use crate::error::SendError;
use crate::message::{HasSessionId, LateReplyPolicy};
use crate::scope::TaskScope;
use crate::session::SessionId;

const STREAM_PULL_CREDIT: u32 = 1;
const STREAM_INITIAL_CREDIT: u32 = 64;

static STREAM_CREDITS: OnceLock<Mutex<HashMap<SessionId, u32>>> = OnceLock::new();
static STREAM_CANCELLED: OnceLock<Mutex<HashSet<SessionId>>> = OnceLock::new();

fn stream_credits() -> &'static Mutex<HashMap<SessionId, u32>> {
    STREAM_CREDITS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn stream_cancelled() -> &'static Mutex<HashSet<SessionId>> {
    STREAM_CANCELLED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Add producer credit for a stream session.
pub fn add_stream_credit(pull: StreamPull) -> bool {
    if stream_cancelled()
        .lock()
        .expect("stream cancellation table poisoned")
        .contains(&pull.session_id)
    {
        return false;
    }

    let mut credits = stream_credits()
        .lock()
        .expect("stream credit table poisoned");
    let credit = credits.entry(pull.session_id).or_insert(0);
    *credit = credit.saturating_add(pull.credit);
    true
}

/// Consume producer credit for stream items before an event is sent.
pub fn consume_stream_credit(session_id: SessionId, count: usize) -> Result<(), SendError> {
    if count == 0 {
        return Ok(());
    }
    if stream_cancelled()
        .lock()
        .expect("stream cancellation table poisoned")
        .contains(&session_id)
    {
        return Err(SendError::StreamCancelled);
    }

    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let mut credits = stream_credits()
        .lock()
        .expect("stream credit table poisoned");
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

fn stream_credit_available(session_id: SessionId, count: usize) -> Result<bool, SendError> {
    if count == 0 {
        return Ok(true);
    }
    if stream_cancelled()
        .lock()
        .expect("stream cancellation table poisoned")
        .contains(&session_id)
    {
        return Err(SendError::StreamCancelled);
    }

    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let credits = stream_credits()
        .lock()
        .expect("stream credit table poisoned");
    Ok(credits.get(&session_id).copied().unwrap_or(0) >= count)
}

/// Forget producer credit for a stream session.
pub fn forget_stream_credit(session_id: SessionId) {
    stream_credits()
        .lock()
        .expect("stream credit table poisoned")
        .remove(&session_id);
    stream_cancelled()
        .lock()
        .expect("stream cancellation table poisoned")
        .remove(&session_id);
}

/// Record producer-side cancellation for a stream session.
pub(crate) fn cancel_stream(session_id: SessionId) {
    stream_credits()
        .lock()
        .expect("stream credit table poisoned")
        .remove(&session_id);
    stream_cancelled()
        .lock()
        .expect("stream cancellation table poisoned")
        .insert(session_id);
}

/// Return producer credit stored for a stream session.
#[must_use]
pub fn stream_credit(session_id: SessionId) -> u32 {
    stream_credits()
        .lock()
        .expect("stream credit table poisoned")
        .get(&session_id)
        .copied()
        .unwrap_or(0)
}

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

    /// Return whether this event ends the stream session.
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

/// Type-erased queued stream event carried by a caller task message.
pub struct QueuedStreamEvent {
    /// Stream session completed or advanced by this event.
    pub session_id: SessionId,

    /// Type-erased stream event value.
    pub event: Box<dyn Any + Send>,

    /// Policy for this stream reply if the session no longer has an active waiter.
    pub late_reply_policy: LateReplyPolicy,
}

impl QueuedStreamEvent {
    /// Construct a queued stream event.
    #[must_use]
    pub fn new(session_id: SessionId, event: Box<dyn Any + Send>) -> Self {
        Self::with_late_reply_policy(session_id, event, LateReplyPolicy::Report)
    }

    /// Construct a queued stream event with an explicit late-reply policy.
    #[must_use]
    pub fn with_late_reply_policy(
        session_id: SessionId,
        event: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self {
        Self {
            session_id,
            event,
            late_reply_policy,
        }
    }
}

/// Message enums that can carry queued task-internal stream events.
pub trait StreamEventMessage: Sized {
    /// Wrap a typed stream event into this task's message enum.
    fn stream_event(session_id: SessionId, event: Box<dyn Any + Send>) -> Self {
        Self::stream_event_with_late_reply_policy(session_id, event, LateReplyPolicy::Report)
    }

    /// Wrap a typed stream event with an explicit late-reply policy.
    fn stream_event_with_late_reply_policy(
        session_id: SessionId,
        event: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self;

    /// Extract a queued stream event from this message, if it is one.
    fn into_stream_event(self) -> Result<QueuedStreamEvent, Self>;
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

/// Message enums that can carry queued stream pull control.
pub trait StreamPullMessage: Sized {
    /// Wrap stream pull control into this task's message enum.
    fn stream_pull(session_id: SessionId, credit: u32) -> Self;

    /// Extract stream pull control from this message, if it is one.
    fn into_stream_pull(self) -> Result<StreamPull, Self>;
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

/// Message enums that can carry queued stream cancellation control.
pub trait StreamCancelMessage: Sized {
    /// Wrap stream cancellation control into this task's message enum.
    fn stream_cancel(session_id: SessionId) -> Self;

    /// Extract stream cancellation control from this message, if it is one.
    fn into_stream_cancel(self) -> Result<StreamCancel, Self>;
}

/// Sends stream lifecycle control for a live stream.
pub trait StreamControl: Send + Sync + 'static {
    /// Try to grant additional item credit to a stream producer.
    fn try_pull(&self, _session_id: SessionId, _credit: u32) -> Result<(), SendError> {
        Ok(())
    }

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

impl<T, E> StreamEventSink<T, E> for Box<dyn StreamEventSink<T, E> + Send> {
    fn send_event(&mut self, event: StreamEvent<T, E>) -> Result<(), SendError> {
        (**self).send_event(event)
    }
}

/// Owned sender endpoint used by stream producers to emit events.
pub struct StreamEventSender<T, E> {
    sink: Box<dyn StreamEventSink<T, E> + Send>,
}

impl<T, E> StreamEventSender<T, E> {
    /// Create a stream-event sender from a sink implementation.
    #[must_use]
    pub fn new(sink: Box<dyn StreamEventSink<T, E> + Send>) -> Self {
        Self { sink }
    }

    /// Send one stream event.
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

/// Boxed producer-side stream sink used by generated stream handlers.
pub type BoxStreamSink<T, E> = StreamSink<T, E, Box<dyn StreamEventSink<T, E> + Send>>;

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
    flow_controlled: bool,
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
            flow_controlled: false,
            _error: PhantomData,
        }
    }

    /// Create a stream sink whose yield operations wait for stream credit.
    #[must_use]
    pub fn new_flow_controlled(session_id: SessionId, batch_size: usize, sink: S) -> Self {
        let mut stream = Self::new(session_id, batch_size, sink);
        stream.flow_controlled = true;
        stream
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

    /// Send buffered items plus one new item, then yield the producer
    /// continuation back to the task-local runtime.
    pub fn yield_item(&mut self, value: T) -> StreamYield<'_, T, E, S> {
        self.buffer.push(value);
        StreamYield::new(self)
    }

    /// Send buffered items plus a batch of new items, then yield the producer
    /// continuation back to the task-local runtime.
    pub fn yield_batch(&mut self, values: impl IntoIterator<Item = T>) -> StreamYield<'_, T, E, S> {
        self.buffer.extend(values);
        StreamYield::new(self)
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

enum StreamYieldState {
    Send,
    Yield,
    Done,
}

/// Context-returning producer-side stream yield operation.
///
/// The first resume sends one batch stream reply and returns `Pending`; the next
/// resume returns `Ready`. This lets native `CtxFuture` stream producers emit an
/// item or batch and then give the task scheduler a chance to route cancellation,
/// flow-control, or ordinary messages before the producer continues.
pub struct StreamYield<'a, T, E, S>
where
    S: StreamEventSink<T, E>,
{
    sink: &'a mut StreamSink<T, E, S>,
    state: StreamYieldState,
}

impl<'a, T, E, S> StreamYield<'a, T, E, S>
where
    S: StreamEventSink<T, E>,
{
    fn new(sink: &'a mut StreamSink<T, E, S>) -> Self {
        Self {
            sink,
            state: StreamYieldState::Send,
        }
    }

    fn try_resume(&mut self) -> CtxPoll<Result<(), SendError>> {
        match self.state {
            StreamYieldState::Send => {
                if self.sink.flow_controlled {
                    match stream_credit_available(self.sink.session_id, self.sink.buffer.len()) {
                        Ok(true) => {}
                        Ok(false) => return CtxPoll::Pending,
                        Err(error) => {
                            self.state = StreamYieldState::Done;
                            return CtxPoll::Ready(Err(error));
                        }
                    }
                }

                match self.sink.flush() {
                    Ok(()) => {
                        self.state = StreamYieldState::Yield;
                        CtxPoll::Pending
                    }
                    Err(SendError::StreamFlowLimited) if self.sink.flow_controlled => {
                        CtxPoll::Pending
                    }
                    Err(error) => {
                        self.state = StreamYieldState::Done;
                        CtxPoll::Ready(Err(error))
                    }
                }
            }
            StreamYieldState::Yield => {
                self.state = StreamYieldState::Done;
                CtxPoll::Ready(Ok(()))
            }
            StreamYieldState::Done => {
                panic!("stream yield resumed after completion");
            }
        }
    }
}

impl<Cx, T, E, S> CtxFuture<Cx> for StreamYield<'_, T, E, S>
where
    S: StreamEventSink<T, E>,
{
    type Output = Result<(), SendError>;

    fn resume(&mut self, _cx: &mut Cx, (): ()) -> CtxPoll<Self::Output> {
        self.try_resume()
    }
}

/// Compatibility bridge for Rust `.await` syntax. Native stream producer
/// continuations should drive this operation through `CtxFuture`.
impl<T, E, S> Future for StreamYield<'_, T, E, S>
where
    T: Unpin,
    E: Unpin,
    S: StreamEventSink<T, E> + Unpin,
{
    type Output = Result<(), SendError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut().try_resume() {
            CtxPoll::Ready(value) => Poll::Ready(value),
            CtxPoll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
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
    /// Construct a stream object and grant the producer initial item credit.
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

    fn grant_credit(&self, credit: u32) {
        let _ = self.control.try_pull(self.session_id, credit);
    }

    fn grant_one_credit(&self) {
        self.grant_credit(STREAM_PULL_CREDIT);
    }

    /// Consume one incoming stream event and return at most one item.
    ///
    /// This is the synchronous core of the future `next(ctx).await` API. It
    /// drains the local buffer before using another incoming event.
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

    /// Return the next buffered item, without consuming a new event.
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

/// Owned task-internal stream session state returned by a task context.
pub type StreamSession<T, E> = (
    SessionId,
    StreamEventSender<T, E>,
    SuspendedMessageStream<T, E>,
);

type StreamOnDrop = Box<dyn FnOnce(SessionId) + 'static>;

/// Task-internal stream consumer for generated handlers.
pub struct SuspendedMessageStream<T, E> {
    stream: MessageStream<T, E>,
    receiver: Receiver<StreamEvent<T, E>>,
    on_drop: Option<StreamOnDrop>,
}

impl<T, E> SuspendedMessageStream<T, E> {
    /// Construct a task-internal stream from a stream session, cancellation
    /// control, and a receiver for producer events routed through the task queue.
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

    /// Construct a task-internal stream with a drop hook.
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

    /// Return this stream's logical session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.stream.session_id()
    }

    /// Return whether the stream has reached end or error.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.stream.is_finished()
    }

    /// Return a context-returning computation for the next item, end, or stream error.
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

/// Context-returning computation returned by `SuspendedMessageStream::next`.
pub struct SuspendedStreamNext<'a, T, E> {
    stream: &'a mut SuspendedMessageStream<T, E>,
}

/// Compatibility bridge for Rust `.await` syntax in user-authored async
/// handlers. The task-local runtime drives the same state through `CtxFuture`.
impl<T, E> Future for SuspendedStreamNext<'_, T, E> {
    type Output = Result<Option<T>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.get_mut().try_resume() {
            CtxPoll::Ready(value) => Poll::Ready(value),
            CtxPoll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

impl<T, E> SuspendedStreamNext<'_, T, E> {
    fn try_resume(&mut self) -> CtxPoll<Result<Option<T>, E>> {
        match self.stream.stream.next_buffered() {
            Ok(Some(value)) => return CtxPoll::Ready(Ok(Some(value))),
            Ok(None) => {}
            Err(error) => return CtxPoll::Ready(Err(error)),
        }

        if self.stream.stream.is_finished() {
            self.stream.disarm_on_drop();
            return CtxPoll::Ready(Ok(None));
        }

        match self.stream.receiver.try_recv() {
            Ok(event) => {
                let result = self.stream.stream.next_from_event(event);
                if self.stream.stream.is_finished() {
                    self.stream.disarm_on_drop();
                }
                CtxPoll::Ready(result)
            }
            Err(TryRecvError::Empty) => CtxPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.stream.disarm_on_drop();
                CtxPoll::Ready(Ok(None))
            }
        }
    }
}

impl<Cx, T, E> CtxFuture<Cx> for SuspendedStreamNext<'_, T, E> {
    type Output = Result<Option<T>, E>;

    fn resume(&mut self, _cx: &mut Cx, (): ()) -> CtxPoll<Self::Output> {
        self.try_resume()
    }
}

/// Create the waiter sender and suspended stream for one queued task-internal stream.
#[must_use]
pub fn suspended_stream_waiter<T, E>(
    session_id: SessionId,
    control: Arc<dyn StreamControl>,
) -> (Sender<StreamEvent<T, E>>, SuspendedMessageStream<T, E>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (
        sender,
        SuspendedMessageStream::new(session_id, control, receiver),
    )
}

/// Create the waiter sender and suspended stream for one queued task-internal
/// stream with a hook for dropped streams.
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

/// Blocking stream consumer for callers outside the task model.
pub struct BlockingMessageStream<T, E> {
    stream: MessageStream<T, E>,
    receiver: Receiver<StreamEvent<T, E>>,
}

impl<T, E> BlockingMessageStream<T, E> {
    /// Construct a blocking stream from a stream session, cancellation control,
    /// and a receiver for producer events.
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

    /// Return this stream's logical session identifier.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.stream.session_id()
    }

    /// Return whether the stream has reached end or error.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.stream.is_finished()
    }

    /// Block the current OS thread until one item, end, error, or producer
    /// disconnect is observed.
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
    fn req_112_stream_consumer_grants_credit_on_create_and_item_consumption() {
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
    fn req_112_stream_event_sender_enforces_credit() {
        let session_id = SessionId::new(crate::EndpointId(8), 1);
        forget_stream_credit(session_id);
        let events = Arc::new(Mutex::new(Vec::<StreamEvent<u32, String>>::new()));
        let captured_events = events.clone();
        let mut sender = StreamEventSender::new(Box::new(move |event| {
            captured_events
                .lock()
                .expect("events lock poisoned")
                .push(event);
            Ok(())
        }));

        assert_eq!(
            sender.send(StreamEvent::batch(session_id, vec![1])),
            Err(SendError::StreamFlowLimited)
        );
        assert!(events.lock().expect("events lock poisoned").is_empty());

        add_stream_credit(StreamPull::new(session_id, 1));
        assert_eq!(
            sender.send(StreamEvent::batch(session_id, vec![2, 3])),
            Err(SendError::StreamFlowLimited)
        );
        assert!(events.lock().expect("events lock poisoned").is_empty());
        assert_eq!(stream_credit(session_id), 1);

        sender
            .send(StreamEvent::batch(session_id, vec![4]))
            .unwrap();
        assert_eq!(stream_credit(session_id), 0);
        assert_eq!(
            events.lock().expect("events lock poisoned").as_slice(),
            &[StreamEvent::batch(session_id, vec![4])],
        );
    }

    #[test]
    fn req_106_req_112_stream_event_sender_reports_cancelled_session() {
        let session_id = SessionId::new(crate::EndpointId(9), 1);
        forget_stream_credit(session_id);
        add_stream_credit(StreamPull::new(session_id, 1));
        cancel_stream(session_id);
        let mut sender = StreamEventSender::new(Box::new(|_| Ok(())));

        assert_eq!(
            sender.send(StreamEvent::<u32, String>::batch(session_id, vec![1])),
            Err(SendError::StreamCancelled)
        );
        assert!(!add_stream_credit(StreamPull::new(session_id, 1)));

        forget_stream_credit(session_id);
    }
}
