# Generated Task API Inspection

## Summary

This report records inspection evidence for the macro-generated task surface in
`crates/mpi-macros/src/lib.rs`.

The inspected macro generates the task message enum, task handle, task context,
message placement implementation, protocol-message conversions, stream control
adapter, spawn helper, and dispatch arms from one `#[task(queue_size = N)] impl`
block.

## Requirements Inspected

- REQ-026: generated message enum
- REQ-027: generated send methods
- REQ-028: direct enqueue by send method
- REQ-030 through REQ-035: receiver-declared placement
- REQ-040 through REQ-043 and REQ-053: generated start message priority
- REQ-050 through REQ-052: task macro and dispatch
- REQ-090 through REQ-097: call response and late-reply plumbing
- REQ-100 through REQ-108: stream event, cancellation, and late stream plumbing
- REQ-120 through REQ-121: explicit external blocking APIs

## Inspected Implementation

The macro parses `#[task(queue_size = ...)]` on an impl block, so queue capacity
and handler declarations are available to one procedural macro expansion.

Generated message enum evidence:

- the macro computes `<Task>Message` and emits `enum #message_ident`;
- each `#[start]`, `#[event]`, `#[call]`, and `#[stream]` handler contributes a
  message variant;
- internal protocol variants are generated for call responses, call release,
  stream pull, stream cancellation, and stream events.

Generated placement evidence:

- the generated `TaskMessage` impl computes placement from the receiver-side
  handler declaration;
- start messages are always emitted as `MessagePlacement::Priority`;
- priority events use `MessagePlacement::Priority`;
- ordinary events, calls, and streams use `MessagePlacement::Normal`;
- generated protocol messages use priority placement.

Generated handle evidence:

- each event handler receives a context-aware send method and an explicit
  `_blocking` external method;
- each call handler receives a context-aware method returning `SuspendedCall<T>`
  and an explicit `_blocking` method returning `Result<T, CallError>`;
- each stream handler receives a context-aware method returning
  `SuspendedMessageStream<T, E>` and an explicit `_blocking` method returning
  `BlockingMessageStream<T, E>`;
- context-aware call and stream methods require the caller context's generated
  message enum to implement `CanReceive<T>` for the expected response or stream
  event type;
- generated methods enqueue the generated message enum directly through
  `TaskHandle::send_message` or `TaskHandle::call_blocking`.

Generated context and dispatch evidence:

- the macro emits `<Task>Context` and implements `TaskScope` for it;
- the generated context exposes `self_handle`, `next_session_id`, `stop`, and
  `is_stopped`;
- the generated spawn helper enqueues the start variant through `spawn_task`;
- generated dispatch arms call the matching handler for each user-declared
  message variant;
- dispatch routes generated protocol variants to task-local call, stream, and
  late-reply handling.
- `#[task(..., receives(...))]` emits `CanReceive<T>` impls for declared raw
  response and stream event types and for protocol-qualified reply/event wrapper
  types.
- Protocol-generated reply and stream event receive types are unique wrappers,
  so receive declarations can distinguish protocol message identity even when
  two protocols use the same Rust payload, item, or error types.

## Verification Evidence

The current automated evidence is:

- `crates/mpi/tests/task_macro.rs`
- `crates/mpi/tests/scope_compile_fail.rs`
- `crates/mpi/tests/runtime_baseline.rs`
- `crates/mpi/examples/ping_pong.rs`
- `crates/mpi/examples/synchronous_call.rs`
- `crates/mpi/examples/streaming_query.rs`
- `crates/mpi/examples/priority_shutdown.rs`

## Remaining Gaps

The macro inspection does not close these known gaps:

- direct awaited assignment, pre-await side effects followed by awaited
  assignment, awaited-let reply projection, and two pending call futures now
  lower into the native `CtxFuture` dispatch hook, and mapped REQ-062 tests
  cover request progress for the suspended call-wait shapes;
- handler declarations use ordinary `fn` syntax and are adapted into
  `CtxFuture` at the task runtime boundary when no native lowering is available;
  native lowering of stream-next loops and arbitrary handler bodies directly
  into `CtxFuture` continuations remains pending.

These gaps should remain visible in traceability and verification reports until
supporting implementation and tests exist.
