# mpi-macros

`mpi-macros` provides the procedural macros that turn ordinary Rust task and
protocol declarations into runtime-facing message enums, handles, contexts, and
protocol bindings.

Most users get these macros through `mpi` rather than depending on
`mpi-macros` directly.

## What It Solves

Hand-writing a task message enum, placement implementation, handle methods,
blocking call plumbing, stream control, and dispatch loop is repetitive and
easy to get subtly wrong. The macros generate that boilerplate from a compact
Rust declaration.

## How It Interacts With Other Crates

- Generated code targets `::mpi` runtime paths.
- `mpi-core` re-exports the macros.
- The `mpi` facade re-exports `mpi-core`, so application code can use
  `#[mpi::task]`, `#[mpi::event]`, `#[mpi::call]`, and `mpi::protocol!`.
- Runtime semantics such as queue ordering, sessions, and stream buffering live
  in `mpi-core`.

## Usage

Read the runnable facade examples:

- [../mpi/examples/ping_pong.rs](../mpi/examples/ping_pong.rs): minimal
  `#[task]`, `#[start]`, and `#[event]`.
- [../mpi/examples/synchronous_call.rs](../mpi/examples/synchronous_call.rs):
  `#[call]` and blocking external use.
- [../mpi/examples/streaming_query.rs](../mpi/examples/streaming_query.rs):
  `#[stream]` and stream sinks.

The conceptual shape is:

```rust
struct Server {
    value: usize,
}

#[mpi::task(queue_size = 16)]
impl Server {
    #[mpi::start]
    fn start(_ctx: &mut ServerContext) {}

    #[mpi::event]
    fn set(ctx: &mut ServerContext, value: usize) {
        ctx.with_state(|state| state.value = value);
    }

    #[mpi::call]
    fn get(ctx: &mut ServerContext) -> usize {
        ctx.with_state(|state| state.value)
    }
}
```

## Best Practices

Access task state through `ctx.with_state(...)`. Do not add a `self` receiver
to handlers.

Use generated blocking methods only from outside task handler scope. Inside a
handler, use context-aware generated methods so waits suspend the handler
instead of blocking the task thread.

Put reusable message contracts in `protocol!` declarations when multiple tasks
or crates should share the same typed interaction shape.

Let macro diagnostics guide unsupported shapes. If a handler name collides with
a generated handle method, rename the handler rather than relying on name
shadowing.
