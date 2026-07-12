use std::sync::Arc;
use std::time::Duration;

use mpi::{EndpointId, MessagePlacement, SessionId, TaskHandle, TaskMessage, TaskQueue};
use timeout_service::{Time, TimeoutRequest, start_timeout_service};

enum ExampleMessage {
    Timeout(SessionId),
}

impl TaskMessage for ExampleMessage {
    fn placement(&self) -> MessagePlacement {
        MessagePlacement::Normal
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let target = TaskHandle::<ExampleMessage, 8>::new(Arc::new(TaskQueue::new()));
    let session_id = SessionId::new(EndpointId(1), 42);
    let delivery_target = target.clone();

    service.protocol().request_blocking(TimeoutRequest::new(
        session_id,
        Time::now() + Duration::from_millis(10),
        move || delivery_target.send_message(ExampleMessage::Timeout(session_id)),
    ))?;

    let ExampleMessage::Timeout(delivered) = target.recv_message()?;
    assert_eq!(delivered, session_id);
    println!("timeout delivered for {delivered}");
    Ok(())
}
