use std::time::Duration;

use mpi::{StreamEvent, task};
use timeout_service::{
    Time, TimeoutError, TimeoutOccurred, TimeoutServiceInstance, start_timeout_service,
};

#[derive(Default)]
struct Client;

#[task(queue_size = 8, receives(StreamEvent<TimeoutOccurred, TimeoutError>))]
impl Client {
    #[start]
    fn start(ctx: &mut ClientContext, service: TimeoutServiceInstance) {
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
    let service = start_timeout_service();
    let (_client, runtime) = Client::spawn(Client, service).unwrap();
    runtime.join().unwrap();
    println!("timeout stream was canceled by dropping it");
}
