use mpi::task;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Worker {
    log: Option<Arc<Mutex<Vec<&'static str>>>>,
}

#[task(queue_size = 8)]
impl Worker {
    #[start]
    fn start(&mut self, _ctx: &mut WorkerContext, log: Arc<Mutex<Vec<&'static str>>>) {
        log.lock().unwrap().push("start");
        self.log = Some(log);
    }

    #[event]
    fn work(&mut self, _ctx: &mut WorkerContext) {
        self.log.as_ref().unwrap().lock().unwrap().push("work");
    }

    #[event(priority)]
    fn shutdown(&mut self, ctx: &mut WorkerContext) {
        self.log.as_ref().unwrap().lock().unwrap().push("shutdown");
        ctx.stop();
    }
}

fn main() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (worker, runtime) = Worker::spawn(Worker::default(), log.clone()).unwrap();

    worker.work_blocking().unwrap();
    worker.shutdown_blocking().unwrap();
    runtime.join().unwrap();

    assert_eq!(log.lock().unwrap().as_slice(), &["start", "shutdown"]);
}
