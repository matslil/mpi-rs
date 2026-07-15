use mpi::task;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Worker {
    log: Option<Arc<Mutex<Vec<&'static str>>>>,
}

#[task(queue_size = 8)]
impl Worker {
    fn new(log: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self { log: Some(log) }
    }

    #[event]
    fn work(ctx: &mut WorkerContext) {
        ctx.with_state(|state| {
            state.log.as_ref().unwrap().lock().unwrap().push("work");
        });
    }

    #[event(priority)]
    fn shutdown(ctx: &mut WorkerContext) {
        ctx.with_state(|state| {
            state.log.as_ref().unwrap().lock().unwrap().push("shutdown");
        });
        ctx.stop();
    }
}

fn main() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (worker, runtime) = Worker::spawn(Worker::new(log.clone())).unwrap();

    worker.work_blocking().unwrap();
    worker.shutdown_blocking().unwrap();
    runtime.join().unwrap();

    assert_eq!(log.lock().unwrap().as_slice(), &["shutdown"]);
}
