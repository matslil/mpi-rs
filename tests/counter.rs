use mpi_objects::{MpiMessage, MpiResponse, ReplySink, spawn_object};

#[derive(Default)]
struct Counter {
    value: i32,
}

enum CounterMessage {
    Increment(i32),
    Get,
    Countdown(i32),
}

impl MpiMessage for CounterMessage {
    type Target = Counter;
    type Yield = i32;
    type Return = i32;

    fn dispatch(self, object: &mut Self::Target, sink: &mut ReplySink<Self::Yield, Self::Return>) {
        match self {
            CounterMessage::Increment(delta) => {
                object.value += delta;
                sink.return_value(object.value);
            }
            CounterMessage::Get => sink.return_value(object.value),
            CounterMessage::Countdown(from) => {
                for value in (0..=from).rev() {
                    sink.yield_item(value);
                }
                sink.finish();
            }
        }
    }
}

#[test]
fn synchronous_calls_return_values() {
    let handle = spawn_object::<Counter, CounterMessage>(Counter::default());

    assert_eq!(Some(3), handle.call(CounterMessage::Increment(3)).unwrap());
    assert_eq!(Some(3), handle.call(CounterMessage::Get).unwrap());
}

#[test]
fn streaming_calls_emit_all_yields() {
    let handle = spawn_object::<Counter, CounterMessage>(Counter::default());

    let mut stream = handle
        .call_stream(CounterMessage::Countdown(3))
        .expect("stream should start");

    let mut observed = Vec::new();
    while let Some(response) = stream.next() {
        match response {
            MpiResponse::Yield(value) => observed.push(value),
            MpiResponse::Finished => break,
            other => panic!("unexpected response: {:?}", other),
        }
    }

    assert_eq!(vec![3, 2, 1, 0], observed);
}
