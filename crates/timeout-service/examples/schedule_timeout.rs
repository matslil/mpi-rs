use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpi::{StreamEvent, task};
use timeout_service::{
    Time, TimeoutError, TimeoutOccurred, TimeoutServiceInstance, start_timeout_service,
};

struct Client(Arc<Mutex<bool>>);

#[task(queue_size = 8, receives(StreamEvent<TimeoutOccurred, TimeoutError>))]
impl Client {
    #[start]
    fn start(ctx: &mut ClientContext, service: TimeoutServiceInstance) {
        let mut timeout = service
            .timeout(ctx, Time::now() + Duration::from_millis(10))
            .unwrap();
        assert_eq!(timeout.next(ctx).await.unwrap(), Some(TimeoutOccurred));
        assert_eq!(timeout.next(ctx).await.unwrap(), None);
        ctx.with_state(|state| *state.0.lock().unwrap() = true);
        ctx.stop();
    }
}

fn main() {
    let service = start_timeout_service();
    let occurred = Arc::new(Mutex::new(false));
    let (_client, runtime) = Client::spawn(Client(Arc::clone(&occurred)), service).unwrap();
    runtime.join().unwrap();
    assert!(*occurred.lock().unwrap());
    println!("timeout occurred");
}
