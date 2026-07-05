use mpi::SendError;

#[test]
fn stream_limited_send_error_is_distinct() {
    assert_ne!(SendError::StreamFlowLimited, SendError::QueueFull);
    assert_ne!(SendError::StreamFlowLimited, SendError::TaskStopped);
}
