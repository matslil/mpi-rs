use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpi::{EndpointId, SessionId, task};
use timeout_service::{
    Time, TimeoutCancel, TimeoutOccurred, TimeoutRequest, TimeoutServiceInstance,
    start_timeout_service,
};

struct Receiver {
    occurred: Arc<Mutex<Option<SessionId>>>,
}

#[task(queue_size = 8)]
impl Receiver {
    #[start]
    fn start(
        ctx: &mut ReceiverContext,
        service: TimeoutServiceInstance<8>,
        canceled: SessionId,
        verification: SessionId,
    ) {
        let canceled_request =
            TimeoutRequest::new(ctx, canceled, Time::now() + Duration::from_millis(20));
        service.protocol().request(ctx, canceled_request).unwrap();
        service
            .protocol()
            .cancel(ctx, TimeoutCancel::new(canceled))
            .unwrap();

        let verification_request =
            TimeoutRequest::new(ctx, verification, Time::now() + Duration::from_millis(40));
        service
            .protocol()
            .request(ctx, verification_request)
            .unwrap();
    }

    #[event(receive)]
    fn timeout_occurred(ctx: &mut ReceiverContext, occurred: TimeoutOccurred) {
        ctx.with_state(|state| {
            *state.occurred.lock().unwrap() = Some(occurred.session_id());
        });
        ctx.stop();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let observed = Arc::new(Mutex::new(None));
    let canceled = SessionId::new(EndpointId(1), 7);
    let verification = SessionId::new(EndpointId(1), 8);
    let (_receiver, runtime) = Receiver::spawn(
        Receiver {
            occurred: Arc::clone(&observed),
        },
        service.clone(),
        canceled,
        verification,
    )?;

    runtime.join()?;
    assert_eq!(*observed.lock().unwrap(), Some(verification));
    println!("timeout for {canceled} was canceled before it occurred");
    Ok(())
}
