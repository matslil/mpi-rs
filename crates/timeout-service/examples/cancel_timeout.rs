use std::sync::mpsc;
use std::time::Duration;

use mpi::task;
use timeout_service::{
    Time, TimeoutRequest, TimeoutServiceInstance, TimeoutServiceProtocolV1, start_timeout_service,
};

struct Client;

#[task(queue_size = 8, receives(TimeoutServiceProtocolV1::request::Reply))]
impl Client {
    #[start]
    fn start(
        ctx: &mut ClientContext,
        service: TimeoutServiceInstance<8>,
        canceled: mpsc::Sender<mpi::SessionId>,
    ) {
        let timeout = service.protocol().request(
            ctx,
            TimeoutRequest::new(Time::now() + Duration::from_millis(200)),
        );
        let session_id = timeout.session_id().expect("timeout request was accepted");

        // Dropping the suspended call sends a best-effort cancellation keyed by
        // the request's generated SessionId.
        drop(timeout);
        canceled.send(session_id).unwrap();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let (canceled_tx, canceled_rx) = mpsc::channel();
    let (client, runtime) = Client::spawn(Client, service, canceled_tx)?;
    let session_id = canceled_rx.recv()?;

    client.stop_blocking()?;
    runtime.join()?;
    println!("timeout for {session_id} was canceled before delivery");
    Ok(())
}
