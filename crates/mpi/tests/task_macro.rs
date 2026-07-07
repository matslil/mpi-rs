use mpi::{protocol, task};

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
        event Add(AddRequest);
        call Get(GetRequest) -> GetReply;
    }
}

protocol! {
    pub protocol ProducerProtocolV1 {
        stream Numbers(NumbersRequest) -> u32 error NumbersError;
    }
}

#[derive(Default)]
struct Counter {
    value: u32,
}

#[task(queue_size = 8)]
impl Counter {
    #[start]
    fn start(&mut self, _ctx: &mut CounterContext, initial: u32) {
        self.value = initial;
    }

    #[event(priority)]
    fn add(&mut self, _ctx: &mut CounterContext, amount: u32) {
        self.value += amount;
    }

    #[event(priority)]
    fn add_from_handler(&mut self, ctx: &mut CounterContext, amount: u32) {
        let self_handle = ctx.self_handle();
        self_handle.add(ctx, amount).unwrap();
    }

    #[call(reply = u32, late_reply = "ignore")]
    fn get(&mut self, _ctx: &mut CounterContext) -> u32 {
        self.value
    }

    #[event(priority)]
    fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct ProtocolCounter {
    value: u32,
}

#[task(queue_size = 8)]
impl ProtocolCounter {
    #[start]
    fn start(&mut self, _ctx: &mut ProtocolCounterContext, initial: u32) {
        self.value = initial;
    }

    #[event(protocol = CounterProtocolV1::Add)]
    fn add(&mut self, _ctx: &mut ProtocolCounterContext, request: AddRequest) {
        self.value += request.amount;
    }

    #[call(protocol = CounterProtocolV1::Get, reply = GetReply)]
    fn get(&mut self, _ctx: &mut ProtocolCounterContext, _request: GetRequest) -> GetReply {
        GetReply { value: self.value }
    }

    #[event(priority)]
    fn stop(&mut self, ctx: &mut ProtocolCounterContext) {
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
        CounterProtocolV1::Get::Reply,
        ProducerProtocolV1::Numbers::Event
    )
)]
impl Client {
    #[start]
    fn start(&mut self, _ctx: &mut ClientContext) {}

    #[event]
    fn ask_counter(&mut self, ctx: &mut ClientContext, counter: CounterHandle) {
        self.observed = counter.get(ctx).await.unwrap();
    }

    #[event]
    fn ask_counter_twice(&mut self, ctx: &mut ClientContext, counter: CounterHandle) {
        let first = counter.get(ctx);
        let second = counter.get(ctx);
        self.observed = first.await.unwrap() + second.await.unwrap();
    }

    #[event]
    fn ask_counter_then_mark(&mut self, ctx: &mut ClientContext, counter: CounterHandle) {
        let self_handle = ctx.self_handle();
        self_handle.mark(ctx, 1).unwrap();
        self.observed = counter.get(ctx).await.unwrap();
    }

    #[event]
    fn sum_numbers(&mut self, ctx: &mut ClientContext, producer: ProducerHandle) {
        let mut stream = producer.numbers(ctx, 4).unwrap();
        let mut sum = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            sum += value;
        }
        self.observed = sum;
    }

    #[event]
    fn ask_protocol_counter(
        &mut self,
        ctx: &mut ClientContext,
        counter: CounterProtocolV1::Binding<ProtocolCounterHandle>,
    ) {
        let reply = counter.get(ctx, GetRequest).await.unwrap();
        self.observed = reply.value;
    }

    #[event]
    fn sum_protocol_numbers(
        &mut self,
        ctx: &mut ClientContext,
        producer: ProducerProtocolV1::Binding<ProtocolProducerHandle>,
    ) {
        let mut stream = producer.numbers(ctx, NumbersRequest { count: 4 }).unwrap();
        let mut sum = 0;
        while let Some(value) = stream.next(ctx).await.unwrap() {
            sum += value;
        }
        self.observed = sum;
    }

    #[event]
    fn drop_reported_stream(&mut self, ctx: &mut ClientContext, producer: ProducerHandle) {
        let stream = producer.fail_after_one(ctx).unwrap();
        drop(stream);
    }

    #[event]
    fn simulate_late_reply_handler(&mut self, ctx: &mut ClientContext) {
        let value = 5_u32;
        let reply = mpi::LateReplyRef::new(
            mpi::SessionId::new(mpi::EndpointId(1), 1),
            mpi::LateReplyKind::CallResponse,
            &value,
        );
        let _ = self.unexpected_reply(ctx, reply);
    }

    #[event]
    fn record_diagnostic_endpoint(&mut self, ctx: &mut ClientContext) {
        self.observed = ctx.diagnostics_snapshot().endpoint.0 as u32;
    }

    #[late_reply]
    fn unexpected_reply(
        &mut self,
        _ctx: &mut ClientContext,
        reply: mpi::LateReplyRef<'_>,
    ) -> mpi::LateReplyAction {
        if reply.kind() == mpi::LateReplyKind::CallResponse
            && reply.downcast_ref::<u32>() == Some(&5)
        {
            self.observed += 100;
        }
        mpi::LateReplyAction::Ignore
    }

    #[event(priority)]
    fn mark(&mut self, _ctx: &mut ClientContext, amount: u32) {
        self.observed += amount;
    }

    #[call(reply = u32)]
    fn observed(&mut self, _ctx: &mut ClientContext) -> u32 {
        self.observed
    }

    #[event(priority)]
    fn stop(&mut self, ctx: &mut ClientContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Producer;

#[task(queue_size = 8)]
impl Producer {
    #[start]
    fn start(&mut self, _ctx: &mut ProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2, late_reply = "ignore")]
    fn numbers(
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
    fn fail_after_one(
        &mut self,
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        out.push(9).map_err(|error| error.to_string())?;
        Err("failed".to_owned())
    }

    #[event(priority)]
    fn stop(&mut self, ctx: &mut ProducerContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct ProtocolProducer;

#[task(queue_size = 8)]
impl ProtocolProducer {
    #[start]
    fn start(&mut self, _ctx: &mut ProtocolProducerContext) {}

    #[stream(
        protocol = ProducerProtocolV1::Numbers,
        item = u32,
        error = String,
        batch_size = 2
    )]
    fn numbers(
        &mut self,
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
    fn stop(&mut self, ctx: &mut ProtocolProducerContext) {
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
    assert_eq!(client.observed_blocking().unwrap(), 41);

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
    assert_eq!(client.observed_blocking().unwrap(), 42);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
}

#[test]
fn req_063_req_092_queued_call_response_wakes_waiter_before_deferred_messages() {
    let (counter, counter_runtime) = Counter::spawn(Counter::default(), 31).unwrap();
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client
        .ask_counter_then_mark_blocking(counter.clone())
        .unwrap();
    assert_eq!(client.observed_blocking().unwrap(), 32);

    client.stop_blocking().unwrap();
    counter.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    counter_runtime.join().unwrap();
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
fn req_094_generated_late_reply_handler_can_inspect_reply() {
    let (client, client_runtime) = Client::spawn(Client::default()).unwrap();

    client.simulate_late_reply_handler_blocking().unwrap();
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
    assert_eq!(client.observed_blocking().unwrap(), 6);

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
    assert_eq!(client.observed_blocking().unwrap(), 33);

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
    assert_eq!(client.observed_blocking().unwrap(), 6);

    client.stop_blocking().unwrap();
    producer.stop_blocking().unwrap();
    client_runtime.join().unwrap();
    producer_runtime.join().unwrap();
}
