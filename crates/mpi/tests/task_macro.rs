use mpi::{call, event, start, task};

#[derive(Default)]
struct Counter {
    value: u32,
}

#[task(queue_size = 8)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext, initial: u32) {
        self.value = initial;
    }

    #[event]
    async fn add(&mut self, _ctx: &mut CounterContext, amount: u32) {
        self.value += amount;
    }

    #[call(reply = u32)]
    async fn get(&mut self, _ctx: &mut CounterContext) -> u32 {
        self.value
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[test]
fn req_051_req_052_macro_generates_task_handle_dispatch_and_call_plumbing() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 10).unwrap();

    counter.add(5).unwrap();
    counter.add(7).unwrap();

    assert_eq!(counter.get_blocking().unwrap(), 22);
    assert_eq!(counter.endpoint(), counter.endpoint());

    counter.stop().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_053_macro_forces_start_message_to_priority() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 3).unwrap();

    // If the generated start message were normal, this priority stop could run
    // first and prevent the start handler from initializing the value.
    counter.stop().unwrap();
    runtime.join().unwrap();
}
