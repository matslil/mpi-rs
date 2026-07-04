use std::sync::{Arc, Mutex};

use mpi::{
    EndpointId, HasSessionId, MessagePlacement, MessageStream, Response, SendError, SessionId,
    SessionIdAllocator, StreamCancel, StreamControl, StreamEvent, StreamSink, SyncReplySender,
    TaskContext, TaskHandle, TaskMessage, TaskQueue, spawn_task,
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
