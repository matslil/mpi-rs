use mpi::{protocol, task};
use std::sync::mpsc;

#[derive(Clone, Debug, Eq, PartialEq)]
struct AddRequest {
    amount: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GetRequest;

#[derive(Clone, Debug, Eq, PartialEq)]
struct GetReply {
    value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NumbersRequest {
    count: u32,
}

type NumbersError = String;

protocol! {
    pub protocol CounterProtocolV1 {
        event add(AddRequest);
        call get(GetRequest) -> GetReply;
    }
}

protocol! {
    pub protocol ProducerProtocolV1 {
        stream numbers(NumbersRequest) -> u32, NumbersError;
    }
}

#[derive(Default)]
struct Counter {
    value: u32,
}

#[derive(Default)]
struct NoStartTask {
    value: u32,
}

#[task(queue_size = 4)]
impl NoStartTask {
    #[event]
    fn set(ctx: &mut NoStartTaskContext, value: u32) {
        ctx.with_state(|state| {
            state.value = value;
        });
    }

    #[call]
    fn get(ctx: &mut NoStartTaskContext) -> u32 {
        ctx.with_state(|state| state.value)
    }

    #[event(priority)]
    fn stop(ctx: &mut NoStartTaskContext) {
        ctx.stop();
    }
}

#[task(queue_size = 8)]
impl Counter {
    #[start]
    fn start(ctx: &mut CounterContext, initial: u32) {
        ctx.with_state(|state| {
            state.value = initial;
        });
    }

    #[event(priority)]
    fn add(ctx: &mut CounterContext, amount: u32) {
        ctx.with_state(|state| {
            state.value += amount;
        });
    }

    #[event(priority)]
    fn add_from_handler(ctx: &mut CounterContext, amount: u32) {
        let self_handle = ctx.self_handle();
        self_handle.add(ctx, amount).unwrap();
    }

    #[call(late_reply = "ignore")]
    fn get(ctx: &mut CounterContext) -> u32 {
        ctx.with_state(|state| state.value)
    }

    #[event(priority)]
    fn stop(ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[test]
fn macro_req_033_task_without_start_uses_empty_start_handler() {
    let (task, runtime) = NoStartTask::spawn(NoStartTask::default()).unwrap();

    task.set_blocking(7).unwrap();
    assert_eq!(task.get_blocking().unwrap(), 7);
    task.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[derive(Default)]
struct ProtocolCounter {
    value: u32,
}

#[task(queue_size = 8)]
impl ProtocolCounter {
    #[start]
    fn start(ctx: &mut ProtocolCounterContext, initial: u32) {
        ctx.with_state(|state| {
            state.value = initial;
        });
    }

    #[event(protocol = CounterProtocolV1::add)]
    fn add(ctx: &mut ProtocolCounterContext, request: AddRequest) {
        ctx.with_state(|state| {
            state.value += request.amount;
        });
    }

    #[call(protocol = CounterProtocolV1::get)]
    fn get(ctx: &mut ProtocolCounterContext, _request: GetRequest) -> GetReply {
        ctx.with_state(|state| GetReply { value: state.value })
    }

    #[event(priority)]
    fn stop(ctx: &mut ProtocolCounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client {
    observed: u32,
}

#[task(
    queue_size = 8,
    receives(
        mpi::Response<u32>,
        mpi::StreamEvent<u32, String>,
        CounterProtocolV1::get::Reply,
        ProducerProtocolV1::numbers::Item
    )
)]
impl Client {
    #[start]
    fn start(_ctx: &mut ClientContext) {}

    #[event]
    fn ask_counter(ctx: &mut ClientContext, counter: CounterHandle) {
        let observed = counter.get(ctx).await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_counter_twice(ctx: &mut ClientContext, counter: CounterHandle) {
        let first = counter.get(ctx);
        let second = counter.get(ctx);
        let observed = first.await.unwrap() + second.await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_counter_then_mark(ctx: &mut ClientContext, counter: CounterHandle) {
        let self_handle = ctx.self_handle();
        self_handle.mark(ctx, 1).unwrap();
        let observed = counter.get(ctx).await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_counter_then_mark_deferred(ctx: &mut ClientContext, counter: CounterHandle) {
        let self_handle = ctx.self_handle();
        self_handle.mark(ctx, 1).unwrap();
        let reply = counter.get(ctx);
        let observed = reply.await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_delayed_counter(ctx: &mut ClientContext, counter: DelayedCounterHandle) {
        let observed = counter.delayed(ctx).await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_delayed_counter_then_mark(ctx: &mut ClientContext, counter: DelayedCounterHandle) {
        let self_handle = ctx.self_handle();
        self_handle.mark(ctx, 1).unwrap();
        let observed = counter.delayed(ctx).await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn ask_two_delayed_counters(
        ctx: &mut ClientContext,
        first_counter: DelayedCounterHandle,
        second_counter: DelayedCounterHandle,
    ) {
        let first = first_counter.delayed(ctx);
        let second = second_counter.delayed(ctx);
        let observed = first.await.unwrap() + second.await.unwrap();
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[event]
    fn sum_numbers(ctx: &mut ClientContext, producer: ProducerHandle) {
        let mut stream = producer.numbers(ctx, 4).unwrap();
        let mut sum = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            sum += value;
        }
        ctx.with_state(|state| {
            state.observed = sum;
        });
    }

    #[event]
    fn sum_delayed_numbers(ctx: &mut ClientContext, producer: DelayedProducerHandle) {
        let mut stream = producer.delayed_numbers(ctx).unwrap();
        let mut sum = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            sum += value;
        }
        ctx.with_state(|state| {
            state.observed = sum;
        });
    }

    #[event]
    fn ask_protocol_counter(
        ctx: &mut ClientContext,
        counter: CounterProtocolV1::Binding<ProtocolCounterHandle>,
    ) {
        let reply = counter.get(ctx, GetRequest).await.unwrap();
        ctx.with_state(|state| {
            state.observed = reply.value;
        });
    }

    #[event]
    fn sum_protocol_numbers(
        ctx: &mut ClientContext,
        producer: ProducerProtocolV1::Binding<ProtocolProducerHandle>,
    ) {
        let mut stream = producer.numbers(ctx, NumbersRequest { count: 4 }).unwrap();
        let mut sum = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            sum += value;
        }
        ctx.with_state(|state| {
            state.observed = sum;
        });
    }

    #[event]
    fn drop_reported_stream(ctx: &mut ClientContext, producer: ProducerHandle) {
        let stream = producer.fail_after_one(ctx).unwrap();
        drop(stream);
    }

    #[event]
    fn simulate_late_reply_callback(ctx: &mut ClientContext) {
        let value = 5_u32;
        let reply = mpi::LateReplyRef::new(
            mpi::SessionId::new(mpi::EndpointId(1), 1),
            mpi::LateReplyKind::CallResponse,
            &value,
        );
        let _ = Self::unexpected_reply(ctx, reply);
    }

    #[event]
    fn record_diagnostic_endpoint(ctx: &mut ClientContext) {
        let observed = ctx.diagnostics_snapshot().endpoint.0 as u32;
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[late_reply]
    fn unexpected_reply(
        ctx: &mut ClientContext,
        reply: mpi::LateReplyRef<'_>,
    ) -> mpi::LateReplyAction {
        if reply.kind() == mpi::LateReplyKind::CallResponse
            && reply.downcast_ref::<u32>() == Some(&5)
        {
            ctx.with_state(|state| {
                state.observed += 100;
            });
        }
        mpi::LateReplyAction::Ignore
    }

    #[event(priority)]
    fn mark(ctx: &mut ClientContext, amount: u32) {
        ctx.with_state(|state| {
            state.observed += amount;
        });
    }

    #[call]
    fn observed(ctx: &mut ClientContext) -> u32 {
        ctx.with_state(|state| state.observed)
    }

    #[event(priority)]
    fn stop(ctx: &mut ClientContext) {
        ctx.stop();
    }
}

struct DelayedCounter {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

#[task(queue_size = 8)]
impl DelayedCounter {
    #[start]
    fn start(_ctx: &mut DelayedCounterContext) {}

    #[call]
    fn delayed(ctx: &mut DelayedCounterContext) -> u32 {
        ctx.with_state(|state| {
            state.started.send(()).unwrap();
            state.release.recv().unwrap();
        });
        10
    }

    #[event(priority)]
    fn stop(ctx: &mut DelayedCounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Producer;

#[derive(Default)]
struct BackpressureProducer;

#[derive(Default)]
struct BackpressureClient {
    observed: u32,
}

#[derive(Default)]
struct ScopedState {
    value: u32,
}

#[task(queue_size = 8)]
impl ScopedState {
    #[start]
    fn start(ctx: &mut ScopedStateContext, initial: u32) {
        ctx.with_state(|state| {
            state.value = initial;
        });
    }

    #[event]
    fn add(ctx: &mut ScopedStateContext, amount: u32) {
        ctx.with_state(|state| {
            state.value += amount;
        });
    }

    #[call]
    fn value(ctx: &mut ScopedStateContext) -> u32 {
        ctx.with_state(|state| state.value)
    }

    #[event(priority)]
    fn stop(ctx: &mut ScopedStateContext) {
        ctx.stop();
    }
}

#[task(queue_size = 8)]
impl Producer {
    #[start]
    fn start(_ctx: &mut ProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2, late_reply = "ignore")]
    fn numbers(
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
    fn fail_after_one(
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        out.push(9).map_err(|error| error.to_string())?;
        Err("failed".to_owned())
    }

    #[event(priority)]
    fn stop(ctx: &mut ProducerContext) {
        ctx.stop();
    }
}

#[task(queue_size = 4)]
impl BackpressureProducer {
    #[start]
    fn start(_ctx: &mut BackpressureProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 1, late_reply = "ignore")]
    fn burst(
        _ctx: &mut BackpressureProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
        count: u32,
    ) -> Result<(), String> {
        for value in 0..count {
            out.push(value).unwrap();
        }
        Ok(())
    }

    #[event(priority)]
    fn stop(ctx: &mut BackpressureProducerContext) {
        ctx.stop();
    }
}

#[task(queue_size = 2, receives(mpi::StreamEvent<u32, String>))]
impl BackpressureClient {
    #[start]
    fn start(_ctx: &mut BackpressureClientContext) {}

    #[event(priority)]
    fn collect_after_delay(
        ctx: &mut BackpressureClientContext,
        producer: BackpressureProducerHandle,
    ) {
        let mut stream = producer.burst(ctx, 3).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(25));

        let mut observed = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            observed += value;
        }
        ctx.with_state(|state| {
            state.observed = observed;
        });
    }

    #[call]
    fn observed(ctx: &mut BackpressureClientContext) -> u32 {
        ctx.with_state(|state| state.observed)
    }

    #[event(priority)]
    fn stop(ctx: &mut BackpressureClientContext) {
        ctx.stop();
    }
}

struct DelayedProducer {
    started: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

#[task(queue_size = 8)]
impl DelayedProducer {
    #[start]
    fn start(_ctx: &mut DelayedProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2, late_reply = "ignore")]
    fn delayed_numbers(
        ctx: &mut DelayedProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        ctx.with_state(|state| {
            state.started.send(()).unwrap();
            state.release.recv().unwrap();
        });
        out.push(7).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[event(priority)]
    fn stop(ctx: &mut DelayedProducerContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct ProtocolProducer;

#[task(queue_size = 8)]
impl ProtocolProducer {
    #[start]
    fn start(_ctx: &mut ProtocolProducerContext) {}

    #[stream(
        protocol = ProducerProtocolV1::numbers,
        item = u32,
        error = String,
        batch_size = 2
    )]
    fn numbers(
        _ctx: &mut ProtocolProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
        request: NumbersRequest,
    ) -> Result<(), String> {
        for value in 0..request.count {
            out.push(value).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[event(priority)]
    fn stop(ctx: &mut ProtocolProducerContext) {
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
fn req_064_generated_context_with_state_scopes_user_state_access() {
    let (task, runtime) = ScopedState::spawn(ScopedState::default(), 7).unwrap();

    task.add_blocking(5).unwrap();
    assert_eq!(task.value_blocking().unwrap(), 12);

    task.stop_blocking().unwrap();
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
fn req_027_generated_event_send_requires_task_scope_for_non_blocking_api() {
    let (counter, runtime) = Counter::spawn(Counter::default(), 1).unwrap();

    counter.add_from_handler_blocking(4).unwrap();
    assert_eq!(counter.get_blocking().unwrap(), 5);

    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[test]
fn req_120_req_121_generated_call_has_context_aware_handler_api() {
    let (counter, counter_runtime) = Counter::spawn(Counter::default(), 41).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.ask_counter_blocking(counter.clone()).unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 41 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 41);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_061_req_063_call_futures_do_not_borrow_task_context_while_suspended() {
    let (counter, counter_runtime) = Counter::spawn(Counter::default(), 21).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.ask_counter_twice_blocking(counter.clone()).unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 42 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 42);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_062_generated_deferred_future_dispatches_ordinary_message_while_suspended() {
    let (counter, counter_runtime) = Counter::spawn(Counter::default(), 31).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_counter_then_mark_deferred_blocking(counter.clone())
        .unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 31 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 31);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_062_generated_task_receives_call_request_while_handler_is_suspended() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (delayed_counter, delayed_counter_runtime) = DelayedCounter::spawn(DelayedCounter {
        started: started_tx,
        release: release_rx,
    })
    .unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_delayed_counter_blocking(delayed_counter.clone())
        .unwrap();
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();

    let (observed_tx, observed_rx) = mpsc::channel();
    let observed_client = client.clone();
    let observed_thread = std::thread::spawn(move || {
        observed_tx
            .send(observed_client.observed_blocking().unwrap())
            .unwrap();
    });

    let observed_before_release = observed_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("generated dispatch deferred the call request until the suspended handler resumed");
    release_tx.send(()).unwrap();

    observed_thread.join().unwrap();
    assert_eq!(observed_before_release, 0);
    assert_eq!(client.observed_blocking().unwrap(), 10);

    client.stop_blocking().unwrap();
    delayed_counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    delayed_counter_runtime.join().unwrap();
}

#[test]
fn req_062_generated_pre_await_handler_dispatches_ordinary_message_while_suspended() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (delayed_counter, delayed_counter_runtime) = DelayedCounter::spawn(DelayedCounter {
        started: started_tx,
        release: release_rx,
    })
    .unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_delayed_counter_then_mark_blocking(delayed_counter.clone())
        .unwrap();
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();

    assert_eq!(client.observed_blocking().unwrap(), 1);

    release_tx.send(()).unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 10 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 10);

    client.stop_blocking().unwrap();
    delayed_counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    delayed_counter_runtime.join().unwrap();
}

#[test]
fn req_062_generated_two_await_handler_dispatches_ordinary_message_while_suspended() {
    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (first_release_tx, first_release_rx) = mpsc::channel();
    let (first_counter, first_runtime) = DelayedCounter::spawn(DelayedCounter {
        started: first_started_tx,
        release: first_release_rx,
    })
    .unwrap();
    let (second_started_tx, second_started_rx) = mpsc::channel();
    let (second_release_tx, second_release_rx) = mpsc::channel();
    let (second_counter, second_runtime) = DelayedCounter::spawn(DelayedCounter {
        started: second_started_tx,
        release: second_release_rx,
    })
    .unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_two_delayed_counters_blocking(first_counter.clone(), second_counter.clone())
        .unwrap();
    first_started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    second_started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();

    let (observed_tx, observed_rx) = mpsc::channel();
    let observed_client = client.clone();
    let observed_thread = std::thread::spawn(move || {
        observed_tx
            .send(observed_client.observed_blocking().unwrap())
            .unwrap();
    });
    assert!(
        observed_rx
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_ok()
    );

    first_release_tx.send(()).unwrap();
    second_release_tx.send(()).unwrap();
    observed_thread.join().unwrap();

    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 20 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 20);

    client.stop_blocking().unwrap();
    first_counter.stop_blocking().unwrap();
    second_counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    first_runtime.join().unwrap();
    second_runtime.join().unwrap();
}

#[test]
fn req_062_generated_stream_next_handler_dispatches_ordinary_message_while_suspended() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (producer, producer_runtime) = DelayedProducer::spawn(DelayedProducer {
        started: started_tx,
        release: release_rx,
    })
    .unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .sum_delayed_numbers_blocking(producer.clone())
        .unwrap();
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();

    assert_eq!(client.observed_blocking().unwrap(), 0);

    release_tx.send(()).unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 7 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 7);

    client.stop_blocking().unwrap();
    producer.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    producer_runtime.join().unwrap();
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
fn req_106_req_107_generated_stream_cancel_clears_producer_credit() {
    let (producer, runtime) = Producer::spawn(Producer).unwrap();
    let stream = producer.numbers_blocking(1).unwrap();
    let session_id = stream.session_id();

    drop(stream);
    producer.stop_blocking().unwrap();
    runtime.join().unwrap();

    assert_eq!(mpi::stream_credit(session_id), 0);
}

#[test]
fn req_094_generated_late_reply_callback_can_inspect_reply() {
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.simulate_late_reply_callback_blocking().unwrap();
    assert_eq!(client.observed_blocking().unwrap(), 100);

    client.stop_blocking().unwrap();
    client_runtime.join().unwrap();
}

#[test]
fn req_140_generated_context_exposes_diagnostics_snapshot() {
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.record_diagnostic_endpoint_blocking().unwrap();
    assert_ne!(client.observed_blocking().unwrap(), 0);

    client.stop_blocking().unwrap();
    client_runtime.join().unwrap();
}

#[test]
fn req_101_req_103_task_internal_stream_next_await_hides_batches() {
    let (producer, producer_runtime) = Producer::spawn(Producer).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.sum_numbers_blocking(producer.clone()).unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 6 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 6);

    client.stop_blocking().unwrap();
    producer.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    producer_runtime.join().unwrap();
}

#[test]
fn req_014a_req_036_req_116_generated_stream_items_wait_for_reserved_queue_capacity() {
    let (producer, producer_runtime) = BackpressureProducer::spawn(BackpressureProducer).unwrap();
    let (client, client_runtime) =
        BackpressureClient::spawn(BackpressureClient::default()).unwrap();

    client
        .collect_after_delay_blocking(producer.clone())
        .unwrap();
    let mut observed = 0;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        match client.observed_blocking() {
            Ok(value) => {
                observed = value;
                if observed == 3 {
                    break;
                }
            }
            Err(mpi::CallError::Send(mpi::SendError::QueueFull)) => {}
            Err(error) => panic!("unexpected observed call error: {error:?}"),
        }
    }
    assert_eq!(observed, 3);

    client.stop_blocking().unwrap();
    producer.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    producer_runtime.join().unwrap();
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

#[test]
fn req_160_req_163_req_169_protocol_binding_uses_declared_message_types() {
    let (counter, counter_runtime) =
        ProtocolCounter::spawn(ProtocolCounter::default(), 10).unwrap();
    let counter_protocol = CounterProtocolV1::bind(counter.clone());

    counter_protocol
        .add_blocking(AddRequest { amount: 7 })
        .unwrap();
    assert_eq!(
        counter_protocol.get_blocking(GetRequest).unwrap(),
        GetReply { value: 17 }
    );

    counter.stop_blocking().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_166_req_168_protocol_receive_declaration_allows_task_internal_waits() {
    let (counter, counter_runtime) =
        ProtocolCounter::spawn(ProtocolCounter::default(), 33).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_protocol_counter_blocking(CounterProtocolV1::bind(counter.clone()))
        .unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 33 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 33);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_166_req_168_protocol_stream_receive_declaration_allows_task_internal_waits() {
    let (producer, producer_runtime) = ProtocolProducer::spawn(ProtocolProducer).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .sum_protocol_numbers_blocking(ProducerProtocolV1::bind(producer.clone()))
        .unwrap();
    let mut observed = client.observed_blocking().unwrap();
    for _ in 0..100 {
        if observed == 6 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
        observed = client.observed_blocking().unwrap();
    }
    assert_eq!(observed, 6);

    client.stop_blocking().unwrap();
    producer.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    producer_runtime.join().unwrap();
}
