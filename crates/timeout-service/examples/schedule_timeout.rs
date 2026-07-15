use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpi::{StreamEvent, task};
use timeout_service::{Time, TimeoutError, TimeoutOccurred, TimeoutTaskServiceInstance};

struct Client {
    occurred: Arc<Mutex<bool>>,
    service: TimeoutTaskServiceInstance,
}

#[task(queue_size = 8, receives(StreamEvent<TimeoutOccurred, TimeoutError>))]
impl Client {
    fn new(occurred: Arc<Mutex<bool>>, service: TimeoutTaskServiceInstance) -> Self {
        Self { occurred, service }
    }

    #[start]
    fn start(ctx: &mut ClientContext) {
        let service = ctx.with_state(|state| state.service.clone());
        let mut timeout = service
            .timeout(ctx, Time::now() + Duration::from_millis(10))
            .unwrap();
        assert_eq!(timeout.next(ctx).await.unwrap(), Some(TimeoutOccurred));
        assert_eq!(timeout.next(ctx).await.unwrap(), None);
        ctx.with_state(|state| *state.occurred.lock().unwrap() = true);
        ctx.stop();
    }
}

fn main() {
    let service = TimeoutTaskServiceInstance::start();
    let occurred = Arc::new(Mutex::new(false));
    let (_client, runtime) = Client::spawn(Client::new(Arc::clone(&occurred), service)).unwrap();
    runtime.join().unwrap();
    assert!(*occurred.lock().unwrap());
    println!("timeout occurred");
}
