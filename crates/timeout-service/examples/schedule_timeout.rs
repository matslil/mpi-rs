use std::sync::mpsc;
use std::time::Duration;

use mpi::{EndpointId, SendError, SessionId};
use timeout_service::{Time, TimeoutRequest, start_timeout_service};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let (tx, rx) = mpsc::channel();
    let session_id = SessionId::new(EndpointId(1), 42);

    service.protocol().request_blocking(TimeoutRequest::new(
        session_id,
        Time::now() + Duration::from_millis(10),
        move || tx.send(session_id).map_err(|_| SendError::TaskStopped),
    ))?;

    let delivered = rx.recv_timeout(Duration::from_secs(1))?;
    assert_eq!(delivered, session_id);
    println!("timeout delivered for {delivered}");
    Ok(())
}
