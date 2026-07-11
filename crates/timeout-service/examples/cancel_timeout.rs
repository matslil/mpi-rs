use std::sync::mpsc;
use std::time::Duration;

use mpi::{EndpointId, SendError, SessionId};
use timeout_service::{Time, TimeoutCancel, TimeoutRequest, start_timeout_service};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let (tx, rx) = mpsc::channel();
    let session_id = SessionId::new(EndpointId(1), 7);

    service.protocol().request_blocking(TimeoutRequest::new(
        session_id,
        Time::now() + Duration::from_millis(200),
        move || tx.send(session_id).map_err(|_| SendError::TaskStopped),
    ))?;
    service
        .protocol()
        .cancel_blocking(TimeoutCancel::new(session_id))?;

    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    println!("timeout for {session_id} was canceled before delivery");
    Ok(())
}
