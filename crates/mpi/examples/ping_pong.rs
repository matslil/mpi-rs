use mpi::task;

#[derive(Default)]
struct PingPong {
    value: u32,
}

#[task(queue_size = 8)]
impl PingPong {
    #[start]
    fn start(&mut self, _ctx: &mut PingPongContext, initial: u32) {
        self.value = initial;
    }

    #[event]
    fn ping(&mut self, _ctx: &mut PingPongContext, amount: u32) {
        self.value += amount;
    }

    #[call(reply = u32)]
    fn value(&mut self, _ctx: &mut PingPongContext) -> u32 {
        self.value
    }

    #[event(priority)]
    fn stop(&mut self, ctx: &mut PingPongContext) {
        ctx.stop();
    }
}

fn main() {
    let (task, runtime) = PingPong::spawn(PingPong::default(), 2).unwrap();

    task.ping_blocking(3).unwrap();
    task.ping_blocking(5).unwrap();

    assert_eq!(task.value_blocking().unwrap(), 10);

    task.stop_blocking().unwrap();
    runtime.join().unwrap();
}
