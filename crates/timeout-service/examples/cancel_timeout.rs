use std::time::Duration;

use mpi::{StreamEvent, task};
use timeout_service::{Time, TimeoutError, TimeoutOccurred, TimeoutTaskServiceInstance};

struct Client {
    service: TimeoutTaskServiceInstance,
}

#[task(queue_size = 8, receives(StreamEvent<TimeoutOccurred, TimeoutError>))]
impl Client {
    fn new(service: TimeoutTaskServiceInstance) -> Self {
        Self { service }
    }

    #[start]
    fn start(ctx: &mut ClientContext) {
        let service = ctx.with_state(|state| state.service.clone());
        let timeout = service
            .timeout(ctx, Time::now() + Duration::from_secs(1))
            .unwrap();
        drop(timeout);

        let mut verification = service
            .timeout(ctx, Time::now() + Duration::from_millis(10))
            .unwrap();
        assert_eq!(verification.next(ctx).await.unwrap(), Some(TimeoutOccurred));
        ctx.stop();
    }
}

fn main() {
    let service = TimeoutTaskServiceInstance::start();
    let (_client, runtime) = Client::spawn(Client::new(service)).unwrap();
    runtime.join().unwrap();
    println!("timeout stream was canceled by dropping it");
}
