use mpi_objects::{
    MpiMessage, MpiResponse, ReplySink, mpi_trampoline, mpi_trampoline_stream, spawn_object,
};

#[derive(Debug, Default)]
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
            CounterMessage::Get => {
                sink.return_value(object.value);
            }
            CounterMessage::Countdown(from) => {
                for value in (0..=from).rev() {
                    sink.yield_item(value);
                }
                sink.finish();
            }
        }
    }
}

#[derive(Clone)]
struct CounterHandle(mpi_objects::MpiHandle<CounterMessage>);

mpi_trampoline!(impl CounterHandle {
    /// Synchronously increment the counter.
    fn increment(&self, delta: i32) -> i32 => CounterMessage::Increment(delta);
    /// Synchronously fetch the current value.
    fn get(&self) -> i32 => CounterMessage::Get;
});

mpi_trampoline_stream!(impl CounterHandle {
    /// Stream a countdown from the provided start value to zero.
    fn countdown(&self, from: i32) -> mpi_objects::ResponseStream<i32, i32> => CounterMessage::Countdown(from);
});

fn main() {
    let handle = CounterHandle(spawn_object::<Counter, CounterMessage>(Counter::default()));

    let next = handle.increment(3).expect("object should respond");
    println!("counter is now {}", next);

    let mut stream = handle.countdown(5).expect("object should respond");
    while let Some(MpiResponse::Yield(value)) = stream.next() {
        println!("yielded {value}");
    }
}
