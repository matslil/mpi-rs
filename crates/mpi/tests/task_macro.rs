use mpi::task;

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

    #[event(priority)]
    async fn add(&mut self, _ctx: &mut CounterContext, amount: u32) {
        self.value += amount;
    }

    #[event(priority)]
    async fn add_from_handler(&mut self, ctx: &mut CounterContext, amount: u32) {
        let self_handle = ctx.self_handle();
        self_handle.add(ctx, amount).unwrap();
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

#[derive(Default)]
struct Producer;

#[task(queue_size = 8)]
impl Producer {
    #[start]
    async fn start(&mut self, _ctx: &mut ProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2)]
    async fn numbers(
        &mut self,
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
        count: u32,
    ) -> Result<(), String> {
        for value in 0..count {
            out.push(value).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[stream(item = u32, error = String, batch_size = 2)]
    async fn fail_after_one(
        &mut self,
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        out.push(9).map_err(|error| error.to_string())?;
        Err("failed".to_owned())
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ProducerContext) {
        ctx.stop();
    }
}

#[test]
fn req_051_req_052_macro_generates_task_handle_dispatch_and_call_plumbing() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 10).unwrap();

    counter.add_blocking(5).unwrap();
    counter.add_blocking(7).unwrap();

    assert_eq!(counter.get_blocking().unwrap(), 22);

    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_053_macro_forces_start_message_to_priority() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 3).unwrap();

    // If the generated start message were normal, this priority stop could run
    // first and prevent the start handler from initializing the value.
    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_123_generated_event_send_requires_task_scope_for_non_blocking_api() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 1).unwrap();

    counter.add_from_handler_blocking(4).unwrap();
    assert_eq!(counter.get_blocking().unwrap(), 5);

    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_101_req_102_req_103_generated_stream_hides_batches() {
    let (producer, runtime) = Producer::spawn(Producer).unwrap();
    let mut stream = producer.numbers_blocking(3).unwrap();

    assert_eq!(stream.next_blocking().unwrap(), Some(0));
    assert_eq!(stream.next_blocking().unwrap(), Some(1));
    assert_eq!(stream.next_blocking().unwrap(), Some(2));
    assert_eq!(stream.next_blocking().unwrap(), None);
    assert!(stream.is_finished());

    producer.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_105_req_111_generated_stream_error_is_reported_after_buffered_items() {
    let (producer, runtime) = Producer::spawn(Producer).unwrap();
    let mut stream = producer.fail_after_one_blocking().unwrap();

    assert_eq!(stream.next_blocking().unwrap(), Some(9));
    assert_eq!(stream.next_blocking(), Err("failed".to_owned()));
    assert!(stream.is_finished());

    producer.stop_blocking().unwrap();
    runtime.join().unwrap();
}
