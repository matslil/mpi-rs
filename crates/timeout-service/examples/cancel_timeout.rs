use std::sync::Arc;
use std::time::Duration;

use mpi::{EndpointId, MessagePlacement, SessionId, TaskHandle, TaskMessage, TaskQueue};
use timeout_service::{Time, TimeoutCancel, TimeoutRequest, start_timeout_service};

enum ExampleMessage {
    Timeout,
}

impl TaskMessage for ExampleMessage {
    fn placement(&self) -> MessagePlacement {
        MessagePlacement::Normal
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let target = TaskHandle::<ExampleMessage, 8>::new(Arc::new(TaskQueue::new()));
    let session_id = SessionId::new(EndpointId(1), 7);
    let delivery_target = target.clone();

    service.protocol().request_blocking(TimeoutRequest::new(
        session_id,
        Time::now() + Duration::from_millis(200),
        move || delivery_target.send_message(ExampleMessage::Timeout),
    ))?;
    service
        .protocol()
        .cancel_blocking(TimeoutCancel::new(session_id))?;

    std::thread::sleep(Duration::from_millis(300));
    assert!(target.try_recv_message().is_none());
    println!("timeout for {session_id} was canceled before delivery");
    Ok(())
}
