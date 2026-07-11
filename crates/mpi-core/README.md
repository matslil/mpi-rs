# mpi-core

`mpi-core` owns the runtime building blocks for the `mpi-rs` task model:
bounded queues, task handles, sessions, synchronous calls, streams, task
contexts, diagnostics snapshots, and gated transaction identifier types.

Most users should import these items through the `mpi` facade. Use `mpi-core`
directly when building workspace internals or optional crates that need runtime
APIs without depending on the facade.

## What It Solves

The crate provides typed message-passing primitives with explicit queue
backpressure. A task owns a bounded queue. Senders enqueue messages through a
`TaskHandle`, and calls/streams use `SessionId` values to match replies to the
logical interaction that requested them.

## How It Interacts With Other Crates

- Depends on `ctx-future` for context-returning resumable computation support.
- Re-exports `mpi-macros` so generated task and protocol code can target stable
  `::mpi` runtime paths.
- Is re-exported by the `mpi` facade.
- Provides transaction IDs and paths when `enable-mpi-transaction` is enabled.
- Is consumed by `timeout-service`, `persistent-log-storage-service`, and
  `mpi-transaction`.

## Usage

The easiest path is through generated task examples in the facade crate:

- [../mpi/examples/ping_pong.rs](../mpi/examples/ping_pong.rs)
- [../mpi/examples/synchronous_call.rs](../mpi/examples/synchronous_call.rs)
- [../mpi/examples/streaming_query.rs](../mpi/examples/streaming_query.rs)
- [../mpi/examples/priority_shutdown.rs](../mpi/examples/priority_shutdown.rs)

When writing low-level code, use `TaskQueue`, `TaskHandle`, `MessagePlacement`,
`TaskMessage`, `SessionId`, and the call/stream helper types directly.

## Best Practices

Declare message placement on the receiving message type. Senders should not
choose priority at each send site.

Treat `SendError::QueueFull` as recoverable backpressure unless the application
policy says otherwise. Treat `SendError::TaskStopped` as lifecycle information:
the receiving task no longer accepts messages.

Use `SessionId` for every logical call or stream interaction. Do not match
responses only by message type when concurrent calls may be in flight.

Use the facade examples as the preferred public API shape. Low-level direct
runtime usage is useful for services and tests, but generated tasks are the
intended ergonomic user surface.
