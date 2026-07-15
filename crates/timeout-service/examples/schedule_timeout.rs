use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpi::{EndpointId, SessionId, task};
use timeout_service::{
    Time, TimeoutOccurred, TimeoutRequest, TimeoutServiceInstance, start_timeout_service,
};

struct Receiver {
    occurred: Arc<Mutex<Option<SessionId>>>,
}

#[task(queue_size = 8)]
impl Receiver {
    #[start]
    fn start(ctx: &mut ReceiverContext, service: TimeoutServiceInstance<8>, session_id: SessionId) {
        let request = TimeoutRequest::new(ctx, session_id, Time::now() + Duration::from_millis(10));
        service.protocol().request(ctx, request).unwrap();
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
    let session_id = SessionId::new(EndpointId(1), 42);
    let (_receiver, runtime) = Receiver::spawn(
        Receiver {
            occurred: Arc::clone(&observed),
        },
        service.clone(),
        session_id,
    )?;

    runtime.join()?;
    assert_eq!(*observed.lock().unwrap(), Some(session_id));
    println!("timeout occurred for {session_id}");
    Ok(())
}
