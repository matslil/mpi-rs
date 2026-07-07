use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use mpi::{
    CallReleaseMessage, CallResponseMessage, CtxFuture, CtxPoll, EndpointId, HasSessionId,
    LateReplyAction, LateReplyKind, LateReplyPolicy, MessagePlacement, MessageStream,
    QueuedCallRelease, QueuedCallResponse, QueuedStreamEvent, Response, SendError, SessionId,
    SessionIdAllocator, StreamCancel, StreamCancelMessage, StreamControl, StreamEvent,
    StreamEventMessage, StreamPull, StreamPullMessage, StreamSink, SyncReplySender, TaskContext,
    TaskHandle, TaskMessage, TaskQueue, block_on_ctx_task, spawn_task, stream_credit,
};

#[derive(Debug, Clone, Eq, PartialEq)]
enum TestMessage {
    Start,
    Normal(u8),
    Priority(u8),
}

impl TaskMessage for TestMessage {
    fn placement(&self) -> MessagePlacement {
        match self {
            Self::Start | Self::Priority(_) => MessagePlacement::Priority,
            Self::Normal(_) => MessagePlacement::Normal,
        }
    }
}

#[test]
fn req_012_req_013_req_014_queue_capacity_is_static_shared_and_explicit() {
    let queue = TaskQueue::<TestMessage, 2>::new();
    assert_eq!(queue.capacity(), 2);

    queue.try_send(TestMessage::Normal(1)).unwrap();
    queue.try_send(TestMessage::Priority(2)).unwrap();
    assert_eq!(queue.len(), 2);
    assert_eq!(
        queue.try_send(TestMessage::Normal(3)),
        Err(SendError::QueueFull)
    );
}

#[test]
fn req_140_queue_snapshot_reports_read_only_queue_diagnostics() {
    let queue = TaskQueue::<TestMessage, 4>::new();
    queue.try_send(TestMessage::Normal(1)).unwrap();
    queue.try_send(TestMessage::Priority(2)).unwrap();

    assert_eq!(
        queue.snapshot(),
        mpi::TaskQueueSnapshot {
            capacity: 4,
            total_len: 2,
            priority_len: 1,
            normal_len: 1,
            closed: false,
        }
    );

    queue.close();
    assert!(queue.snapshot().closed);
}

#[test]
fn req_032_normal_messages_are_fifo() {
    let queue = TaskQueue::<TestMessage, 4>::new();
    queue.try_send(TestMessage::Normal(1)).unwrap();
    queue.try_send(TestMessage::Normal(2)).unwrap();

    assert_eq!(queue.try_recv(), Some(TestMessage::Normal(1)));
    assert_eq!(queue.try_recv(), Some(TestMessage::Normal(2)));
}

#[test]
fn req_033_priority_messages_are_fifo() {
    let queue = TaskQueue::<TestMessage, 4>::new();
    queue.try_send(TestMessage::Priority(1)).unwrap();
    queue.try_send(TestMessage::Priority(2)).unwrap();

    assert_eq!(queue.try_recv(), Some(TestMessage::Priority(1)));
    assert_eq!(queue.try_recv(), Some(TestMessage::Priority(2)));
}

#[test]
fn req_034_priority_messages_are_received_before_normal_messages() {
    let queue = TaskQueue::<TestMessage, 4>::new();
    queue.try_send(TestMessage::Normal(1)).unwrap();
    queue.try_send(TestMessage::Priority(2)).unwrap();
    queue.try_send(TestMessage::Normal(3)).unwrap();

    assert_eq!(queue.try_recv(), Some(TestMessage::Priority(2)));
    assert_eq!(queue.try_recv(), Some(TestMessage::Normal(1)));
    assert_eq!(queue.try_recv(), Some(TestMessage::Normal(3)));
}

#[test]
fn req_040_req_041_req_042_start_message_is_first() {
    let (handle, runtime) = spawn_task::<TestMessage, _, _, 4>(TestMessage::Start, |handle| {
        handle.send_message(TestMessage::Priority(9)).unwrap();
        let first = handle.queue().recv().unwrap();
        let second = handle.queue().recv().unwrap();
        (first, second)
    })
    .unwrap();

    handle.send_message(TestMessage::Normal(1)).unwrap();
    let (first, second) = runtime.join().unwrap();

    assert_eq!(first, TestMessage::Start);
    assert_eq!(second, TestMessage::Priority(9));
}

#[test]
fn req_080_req_083_req_084_session_ids_are_origin_plus_local_sequence() {
    let mut allocator = SessionIdAllocator::new(EndpointId(7));

    assert_eq!(
        allocator.next_session_id(),
        SessionId::new(EndpointId(7), 0)
    );
    assert_eq!(
        allocator.next_session_id(),
        SessionId::new(EndpointId(7), 1)
    );
}

#[test]
fn req_084_task_context_allocates_task_local_session_ids() {
    let queue = Arc::new(TaskQueue::<TestMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue, EndpointId(55));
    let ctx = TaskContext::new(handle);

    assert_eq!(ctx.next_session_id(), SessionId::new(EndpointId(55), 0));
    assert_eq!(ctx.next_session_id(), SessionId::new(EndpointId(55), 1));
    assert!(!ctx.is_stopped());

    ctx.stop();
    assert!(ctx.is_stopped());
    assert_eq!(
        ctx.self_handle().send_message(TestMessage::Normal(1)),
        Err(SendError::TaskStopped)
    );
}

#[test]
fn req_140_task_context_snapshot_reports_session_and_stream_state() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 8>::new());
    let handle = TaskHandle::with_endpoint(queue, EndpointId(56));
    let ctx = TaskContext::new(handle);
    let (call_session, _reply, _call) = ctx.begin_call::<u32>();
    let (stream_session, _events, _stream) =
        ctx.begin_stream::<u8, &'static str>(Arc::new(CancelRecorder::default()));
    let released_session = SessionId::new(EndpointId(99), 9);

    ctx.record_stream_pull(StreamPull::new(stream_session, 5));
    ctx.record_call_release(QueuedCallRelease::new(released_session));

    let snapshot = ctx.diagnostics_snapshot();

    assert_eq!(snapshot.endpoint, EndpointId(56));
    assert!(!snapshot.stopped);
    assert_eq!(snapshot.active_call_waiters, vec![call_session]);
    assert_eq!(snapshot.active_stream_waiters, vec![stream_session]);
    assert_eq!(
        snapshot.stream_credits,
        vec![mpi::StreamCreditSnapshot {
            session_id: stream_session,
            credit: 5,
        }]
    );
    assert_eq!(snapshot.released_calls, vec![released_session]);
}

#[test]
fn req_090_response_carries_session_id_and_value() {
    let session_id = SessionId::new(EndpointId(1), 42);
    let response = Response::new(session_id, "ok");

    assert_eq!(response.session_id(), session_id);
    assert_eq!(response.into_value(), "ok");
}

enum CallMessage {
    Start,
    Get {
        session_id: SessionId,
        reply: SyncReplySender<u32>,
    },
}

impl TaskMessage for CallMessage {
    fn placement(&self) -> MessagePlacement {
        match self {
            Self::Start => MessagePlacement::Priority,
            Self::Get { .. } => MessagePlacement::Normal,
        }
    }
}

#[test]
fn req_091_req_120_external_call_blocking_returns_one_typed_response() {
    let (handle, runtime) = spawn_task::<CallMessage, _, _, 4>(CallMessage::Start, |handle| {
        assert!(matches!(handle.queue().recv().unwrap(), CallMessage::Start));
        match handle.queue().recv().unwrap() {
            CallMessage::Get { session_id, reply } => {
                reply.send(Response::new(session_id, 42)).unwrap();
            }
            CallMessage::Start => panic!("unexpected second start message"),
        }
    })
    .unwrap();

    let response = handle
        .call_blocking(|session_id, reply| CallMessage::Get { session_id, reply })
        .unwrap();
    assert_eq!(response.value, 42);
    assert_eq!(response.session_id.origin, handle.endpoint());

    runtime.join().unwrap();
}

enum CtxRuntimeMessage {
    Normal(u8),
    CallResponse {
        session_id: SessionId,
        value: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    },
    CallRelease {
        session_id: SessionId,
    },
    StreamPull {
        session_id: SessionId,
        credit: u32,
    },
    StreamCancel {
        session_id: SessionId,
    },
    StreamEvent {
        session_id: SessionId,
        event: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    },
}

impl TaskMessage for CtxRuntimeMessage {
    fn placement(&self) -> MessagePlacement {
        match self {
            Self::Normal(_) => MessagePlacement::Normal,
            Self::CallResponse { .. }
            | Self::CallRelease { .. }
            | Self::StreamPull { .. }
            | Self::StreamCancel { .. }
            | Self::StreamEvent { .. } => MessagePlacement::Priority,
        }
    }
}

impl CallResponseMessage for CtxRuntimeMessage {
    fn call_response_with_late_reply_policy(
        session_id: SessionId,
        value: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self {
        Self::CallResponse {
            session_id,
            value,
            late_reply_policy,
        }
    }

    fn into_call_response(self) -> Result<QueuedCallResponse, Self> {
        match self {
            Self::CallResponse {
                session_id,
                value,
                late_reply_policy,
            } => Ok(QueuedCallResponse::with_late_reply_policy(
                session_id,
                value,
                late_reply_policy,
            )),
            other => Err(other),
        }
    }
}

impl CallReleaseMessage for CtxRuntimeMessage {
    fn call_release(session_id: SessionId) -> Self {
        Self::CallRelease { session_id }
    }

    fn into_call_release(self) -> Result<QueuedCallRelease, Self> {
        match self {
            Self::CallRelease { session_id } => Ok(QueuedCallRelease::new(session_id)),
            other => Err(other),
        }
    }
}

impl StreamPullMessage for CtxRuntimeMessage {
    fn stream_pull(session_id: SessionId, credit: u32) -> Self {
        Self::StreamPull { session_id, credit }
    }

    fn into_stream_pull(self) -> Result<StreamPull, Self> {
        match self {
            Self::StreamPull { session_id, credit } => Ok(StreamPull::new(session_id, credit)),
            other => Err(other),
        }
    }
}

impl StreamCancelMessage for CtxRuntimeMessage {
    fn stream_cancel(session_id: SessionId) -> Self {
        Self::StreamCancel { session_id }
    }

    fn into_stream_cancel(self) -> Result<StreamCancel, Self> {
        match self {
            Self::StreamCancel { session_id } => Ok(StreamCancel::new(session_id)),
            other => Err(other),
        }
    }
}

impl StreamEventMessage for CtxRuntimeMessage {
    fn stream_event_with_late_reply_policy(
        session_id: SessionId,
        event: Box<dyn Any + Send>,
        late_reply_policy: LateReplyPolicy,
    ) -> Self {
        Self::StreamEvent {
            session_id,
            event,
            late_reply_policy,
        }
    }

    fn into_stream_event(self) -> Result<QueuedStreamEvent, Self> {
        match self {
            Self::StreamEvent {
                session_id,
                event,
                late_reply_policy,
            } => Ok(QueuedStreamEvent::with_late_reply_policy(
                session_id,
                event,
                late_reply_policy,
            )),
            other => Err(other),
        }
    }
}

#[derive(Default)]
struct AllocateAcrossPending {
    first: Option<SessionId>,
}

impl CtxFuture<TaskContext<CtxRuntimeMessage, 4>> for AllocateAcrossPending {
    type Output = (SessionId, SessionId);

    fn resume(
        &mut self,
        ctx: &mut TaskContext<CtxRuntimeMessage, 4>,
        (): (),
    ) -> CtxPoll<Self::Output> {
        match self.first {
            Some(first) => CtxPoll::Ready((first, ctx.next_session_id())),
            None => {
                self.first = Some(ctx.next_session_id());
                CtxPoll::Pending
            }
        }
    }
}

#[derive(Default)]
struct PendingTwice {
    pending_count: u8,
}

impl CtxFuture<TaskContext<CtxRuntimeMessage, 4>> for PendingTwice {
    type Output = ();

    fn resume(
        &mut self,
        _ctx: &mut TaskContext<CtxRuntimeMessage, 4>,
        (): (),
    ) -> CtxPoll<Self::Output> {
        if self.pending_count < 2 {
            self.pending_count += 1;
            CtxPoll::Pending
        } else {
            CtxPoll::Ready(())
        }
    }
}

#[test]
fn req_064_block_on_ctx_task_returns_context_between_pending_resumes() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(70));
    let mut ctx = TaskContext::new(handle.clone());
    let mut deferred = VecDeque::new();

    handle.send_message(CtxRuntimeMessage::Normal(9)).unwrap();

    let (first, second) = block_on_ctx_task(
        AllocateAcrossPending::default(),
        &queue,
        &mut ctx,
        &mut deferred,
    );

    assert_eq!(first, SessionId::new(EndpointId(70), 0));
    assert_eq!(second, SessionId::new(EndpointId(70), 1));
    assert_eq!(ctx.next_session_id(), SessionId::new(EndpointId(70), 2));
    assert!(matches!(
        deferred.pop_front(),
        Some(CtxRuntimeMessage::Normal(9))
    ));
}

#[test]
fn req_106_req_107_block_on_ctx_task_routes_stream_cancel() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(78));
    let mut ctx = TaskContext::new(handle.clone());
    let mut deferred = VecDeque::new();
    let session_id = SessionId::new(EndpointId(200), 1);

    handle
        .send_message(CtxRuntimeMessage::stream_pull(session_id, 3))
        .unwrap();
    handle
        .send_message(CtxRuntimeMessage::stream_cancel(session_id))
        .unwrap();

    block_on_ctx_task(PendingTwice::default(), &queue, &mut ctx, &mut deferred);

    assert_eq!(ctx.stream_credit(session_id), 0);
    assert_eq!(stream_credit(session_id), 0);
    assert!(deferred.is_empty());
}

#[test]
fn req_064_block_on_ctx_task_routes_call_response_to_ctx_future_waiter() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(71));
    let mut ctx = TaskContext::new(handle.clone());
    let mut deferred = VecDeque::new();
    let (session_id, _reply, call) = ctx.begin_call::<u32>();

    handle
        .send_message(CtxRuntimeMessage::call_response(
            session_id,
            Box::new(42_u32),
        ))
        .unwrap();

    let value = block_on_ctx_task(call, &queue, &mut ctx, &mut deferred).unwrap();

    assert_eq!(value, 42);
    assert!(deferred.is_empty());
}

#[test]
fn req_063_req_092_out_of_order_call_responses_match_session_ids() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(79));
    let mut ctx = TaskContext::new(handle.clone());
    let mut deferred = VecDeque::new();
    let (first_session, _first_reply, first_call) = ctx.begin_call::<u32>();
    let (second_session, _second_reply, second_call) = ctx.begin_call::<u32>();

    handle
        .send_message(CtxRuntimeMessage::call_response(
            second_session,
            Box::new(2_u32),
        ))
        .unwrap();
    handle
        .send_message(CtxRuntimeMessage::call_response(
            first_session,
            Box::new(1_u32),
        ))
        .unwrap();

    let second = block_on_ctx_task(second_call, &queue, &mut ctx, &mut deferred).unwrap();
    let first = block_on_ctx_task(first_call, &queue, &mut ctx, &mut deferred).unwrap();

    assert_eq!(second, 2);
    assert_eq!(first, 1);
    assert!(deferred.is_empty());
}

#[test]
fn req_109_block_on_ctx_task_defers_ordinary_messages_instead_of_discarding() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(80));
    let mut ctx = TaskContext::new(handle.clone());
    let mut deferred = VecDeque::new();

    handle.send_message(CtxRuntimeMessage::Normal(7)).unwrap();

    let _ = block_on_ctx_task(
        AllocateAcrossPending::default(),
        &queue,
        &mut ctx,
        &mut deferred,
    );

    assert!(matches!(
        deferred.pop_front(),
        Some(CtxRuntimeMessage::Normal(7))
    ));
    assert!(deferred.is_empty());
}

#[test]
fn req_094_late_call_response_default_handler_ignores() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(72));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(99), 3);

    assert_eq!(
        ctx.deliver_call_response(QueuedCallResponse::new(session_id, Box::new(17_u32)))
            .unwrap(),
        LateReplyAction::Ignore
    );
    assert!(!ctx.is_stopped());
}

#[test]
fn req_094_late_call_response_handler_receives_borrowed_reply() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(73));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(100), 4);
    let mut observed = None;

    assert_eq!(
        ctx.deliver_call_response_with_late_reply_handler(
            QueuedCallResponse::new(session_id, Box::new(23_u32)),
            |reply| {
                assert_eq!(reply.session_id(), session_id);
                assert_eq!(reply.kind(), LateReplyKind::CallResponse);
                observed = reply.downcast_ref::<u32>().copied();
                LateReplyAction::Ignore
            },
        )
        .unwrap(),
        LateReplyAction::Ignore
    );
    assert_eq!(observed, Some(23));
    assert!(!ctx.is_stopped());
}

#[test]
fn req_094_late_call_response_with_ignore_policy_bypasses_handler() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(74));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(101), 5);
    let mut called = false;

    assert_eq!(
        ctx.deliver_call_response_with_late_reply_handler(
            QueuedCallResponse::with_late_reply_policy(
                session_id,
                Box::new(29_u32),
                LateReplyPolicy::Ignore,
            ),
            |_| {
                called = true;
                LateReplyAction::Terminate
            },
        )
        .unwrap(),
        LateReplyAction::Ignore
    );
    assert!(!called);
    assert!(!ctx.is_stopped());
}

#[test]
fn req_094_late_call_response_handler_can_terminate_task() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(75));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(102), 6);

    assert_eq!(
        ctx.deliver_call_response_with_late_reply_handler(
            QueuedCallResponse::new(session_id, Box::new(31_u32)),
            |_| LateReplyAction::Terminate,
        )
        .unwrap(),
        LateReplyAction::Terminate
    );
    assert!(ctx.is_stopped());
}

#[test]
fn req_108_late_stream_event_handler_receives_borrowed_reply() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(76));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(103), 7);
    let mut observed_end = false;

    assert_eq!(
        ctx.deliver_stream_event_with_late_reply_handler(
            QueuedStreamEvent::new(
                session_id,
                Box::new(StreamEvent::<u32, String>::end(session_id)),
            ),
            |reply| {
                assert_eq!(reply.session_id(), session_id);
                assert_eq!(reply.kind(), LateReplyKind::StreamEvent);
                observed_end = matches!(
                    reply.downcast_ref(),
                    Some(StreamEvent::<u32, String>::End { .. })
                );
                LateReplyAction::Ignore
            },
        )
        .unwrap(),
        LateReplyAction::Ignore
    );
    assert!(observed_end);
    assert!(!ctx.is_stopped());
}

#[test]
fn req_108_late_stream_event_with_ignore_policy_bypasses_handler() {
    let queue = Arc::new(TaskQueue::<CtxRuntimeMessage, 4>::new());
    let handle = TaskHandle::with_endpoint(queue.clone(), EndpointId(77));
    let ctx = TaskContext::new(handle);
    let session_id = SessionId::new(EndpointId(104), 8);
    let mut called = false;

    assert_eq!(
        ctx.deliver_stream_event_with_late_reply_handler(
            QueuedStreamEvent::with_late_reply_policy(
                session_id,
                Box::new(StreamEvent::<u32, String>::end(session_id)),
                LateReplyPolicy::Ignore,
            ),
            |_| {
                called = true;
                LateReplyAction::Terminate
            },
        )
        .unwrap(),
        LateReplyAction::Ignore
    );
    assert!(!called);
    assert!(!ctx.is_stopped());
}

#[derive(Default)]
struct CancelRecorder {
    cancelled: Mutex<Vec<SessionId>>,
}

impl StreamControl for CancelRecorder {
    fn try_cancel(&self, session_id: SessionId) -> Result<(), SendError> {
        self.cancelled.lock().unwrap().push(session_id);
        Ok(())
    }
}

#[test]
fn req_100_req_102_req_103_stream_batches_are_hidden_by_local_buffer() {
    let control = Arc::new(CancelRecorder::default());
    let session_id = SessionId::new(EndpointId(2), 0);
    let mut stream = MessageStream::<u8, &'static str>::new(session_id, control);

    assert_eq!(
        stream
            .next_from_event(StreamEvent::batch(session_id, [1, 2, 3]))
            .unwrap(),
        Some(1)
    );
    assert_eq!(stream.next_buffered().unwrap(), Some(2));
    assert_eq!(stream.next_buffered().unwrap(), Some(3));
    assert_eq!(stream.next_buffered().unwrap(), None);
}

#[test]
fn req_104_stream_end_marks_stream_finished() {
    let control = Arc::new(CancelRecorder::default());
    let session_id = SessionId::new(EndpointId(2), 1);
    let mut stream = MessageStream::<u8, &'static str>::new(session_id, control);

    assert_eq!(
        stream
            .next_from_event(StreamEvent::end(session_id))
            .unwrap(),
        None
    );
    assert!(stream.is_finished());
}

#[test]
fn req_105_stream_error_returns_typed_error() {
    let control = Arc::new(CancelRecorder::default());
    let session_id = SessionId::new(EndpointId(2), 2);
    let mut stream = MessageStream::<u8, &'static str>::new(session_id, control);

    assert_eq!(
        stream.next_from_event(StreamEvent::error(session_id, "bad")),
        Err("bad")
    );
    assert!(stream.is_finished());
}

#[test]
fn req_106_stream_drop_sends_asynchronous_cancel() {
    let control = Arc::new(CancelRecorder::default());
    let session_id = SessionId::new(EndpointId(2), 3);
    {
        let _stream = MessageStream::<u8, &'static str>::new(session_id, control.clone());
    }

    assert_eq!(control.cancelled.lock().unwrap().as_slice(), &[session_id]);
}

#[test]
fn req_106_completed_stream_drop_does_not_cancel() {
    let control = Arc::new(CancelRecorder::default());
    let session_id = SessionId::new(EndpointId(2), 4);
    {
        let mut stream = MessageStream::<u8, &'static str>::new(session_id, control.clone());
        assert_eq!(
            stream
                .next_from_event(StreamEvent::end(session_id))
                .unwrap(),
            None
        );
    }

    assert!(control.cancelled.lock().unwrap().is_empty());
}

#[test]
fn req_082_stream_cancel_has_session_id() {
    let session_id = SessionId::new(EndpointId(3), 9);
    let cancel = StreamCancel::new(session_id);
    assert_eq!(cancel.session_id(), session_id);
}

#[test]
fn req_102_req_111_stream_sink_batches_and_sends_end() {
    let session_id = SessionId::new(EndpointId(4), 0);
    let events = Arc::new(Mutex::new(Vec::<StreamEvent<u8, &'static str>>::new()));
    let captured_events = events.clone();

    let mut sink = StreamSink::new(session_id, 2, move |event| {
        captured_events.lock().unwrap().push(event);
        Ok(())
    });

    sink.push(1).unwrap();
    assert!(events.lock().unwrap().is_empty());

    sink.push(2).unwrap();
    sink.push(3).unwrap();
    sink.finish().unwrap();

    assert_eq!(
        events.lock().unwrap().as_slice(),
        &[
            StreamEvent::batch(session_id, vec![1, 2]),
            StreamEvent::batch(session_id, vec![3]),
            StreamEvent::end(session_id),
        ]
    );
}

#[test]
fn req_105_req_111_stream_sink_flushes_before_error() {
    let session_id = SessionId::new(EndpointId(4), 1);
    let events = Arc::new(Mutex::new(Vec::<StreamEvent<u8, &'static str>>::new()));
    let captured_events = events.clone();

    let mut sink = StreamSink::new(session_id, 4, move |event| {
        captured_events.lock().unwrap().push(event);
        Ok(())
    });

    sink.push(9).unwrap();
    sink.fail("failed").unwrap();

    assert_eq!(
        events.lock().unwrap().as_slice(),
        &[
            StreamEvent::batch(session_id, vec![9]),
            StreamEvent::error(session_id, "failed"),
        ]
    );
}
