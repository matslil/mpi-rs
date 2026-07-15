use mpi::task;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct TimedTask {
    completed: Arc<Mutex<Vec<u8>>>,
}

#[task(queue_size = 8)]
impl TimedTask {
    #[event]
    fn delay(ctx: &mut TimedTaskContext, id: u8, deadline: Instant) {
        ctx.sleep_until(deadline).await;
        let done = ctx.with_state(|state| {
            let mut completed = state.completed.lock().unwrap();
            completed.push(id);
            completed.len() == 2
        });
        if done {
            ctx.stop();
        }
    }
}

#[test]
fn mpi_req_146_147_148_149_timed_handlers_complete_in_deadline_order() {
    let completed = Arc::new(Mutex::new(Vec::new()));
    let (task, runtime) = TimedTask::spawn(TimedTask {
        completed: Arc::clone(&completed),
    })
    .unwrap();
    let now = Instant::now();

    task.delay_blocking(1, now + Duration::from_millis(40))
        .unwrap();
    task.delay_blocking(2, now + Duration::from_millis(10))
        .unwrap();

    runtime.join().unwrap();
    assert_eq!(*completed.lock().unwrap(), vec![2, 1]);
}
