use std::time::Duration;

use mpi::task;
use timeout_service::{
    Time, TimeoutRequest, TimeoutServiceInstance, TimeoutServiceProtocolV1, start_timeout_service,
};

#[derive(Default)]
struct Client {
    canceled: Option<mpi::SessionId>,
}

#[task(queue_size = 8, receives(TimeoutServiceProtocolV1::request::Reply))]
impl Client {
    #[start]
    fn start(ctx: &mut ClientContext, service: TimeoutServiceInstance<8>) {
        let timeout = service.protocol().request(
            ctx,
            TimeoutRequest::new(Time::now() + Duration::from_millis(200)),
        );
        let session_id = timeout.session_id().expect("timeout request was accepted");

        // Dropping the suspended call sends a best-effort cancellation keyed by
        // the request's generated SessionId.
        drop(timeout);
        ctx.with_state(|state| {
            state.canceled = Some(session_id);
        });
    }

    #[call]
    fn canceled(ctx: &mut ClientContext) -> mpi::SessionId {
        ctx.with_state(|state| state.canceled.expect("timeout cancellation was recorded"))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let (client, runtime) = Client::spawn(Client::default(), service)?;
    let session_id = client.canceled_blocking()?;

    client.stop_blocking()?;
    runtime.join()?;
    println!("timeout for {session_id} was canceled before delivery");
    Ok(())
}
