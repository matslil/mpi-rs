use mpi::task;

#[derive(Default)]
struct PingPong {
    value: u32,
}

#[task(queue_size = 8)]
impl PingPong {
    fn new(initial: u32) -> Self {
        Self { value: initial }
    }

    #[event]
    fn ping(ctx: &mut PingPongContext, amount: u32) {
        ctx.with_state(|state| {
            state.value += amount;
        });
    }

    #[call]
    fn value(ctx: &mut PingPongContext) -> u32 {
        ctx.with_state(|state| state.value)
    }

    #[event(priority)]
    fn stop(ctx: &mut PingPongContext) {
        ctx.stop();
    }
}

fn main() {
    let (task, runtime) = PingPong::spawn(PingPong::new(2)).unwrap();

    task.ping_blocking(3).unwrap();
    task.ping_blocking(5).unwrap();

    assert_eq!(task.value_blocking().unwrap(), 10);

    task.stop_blocking().unwrap();
    runtime.join().unwrap();
}
