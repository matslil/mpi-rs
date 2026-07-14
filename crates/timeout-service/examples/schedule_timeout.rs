use std::time::Duration;

use mpi::{EndpointId, SessionId, task};
use timeout_service::{Time, TimeoutRequest, start_timeout_service};

#[derive(Default)]
struct Receiver {
    occurred: Option<SessionId>,
}

#[task(queue_size = 8)]
impl Receiver {
    #[event]
    fn timeout_occurred(ctx: &mut ReceiverContext, session_id: SessionId) {
        ctx.with_state(|state| {
            state.occurred = Some(session_id);
        });
    }

    #[call]
    fn occurred(ctx: &mut ReceiverContext) -> Option<SessionId> {
        ctx.with_state(|state| state.occurred)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    let (receiver, runtime) = Receiver::spawn(Receiver::default())?;
    let session_id = SessionId::new(EndpointId(1), 42);
    let delivery_target = receiver.clone();

    service.protocol().request_blocking(TimeoutRequest::new(
        session_id,
        Time::now() + Duration::from_millis(10),
        move |occurred| delivery_target.timeout_occurred_blocking(occurred),
    ))?;

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(receiver.occurred_blocking()?, Some(session_id));
    receiver.stop_blocking()?;
    runtime.join()?;
    println!("timeout occurred for {session_id}");
    Ok(())
}
