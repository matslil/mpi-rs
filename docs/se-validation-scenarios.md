# Validation Scenarios

This document defines initial validation scenarios for `mpi-rs`.

Validation answers: did we build the right thing for the intended users?

## Scenario status values

Use these values:

- `proposed`: scenario drafted but not yet approved;
- `approved`: scenario approved for validation;
- `passed`: evidence shows the scenario works;
- `partial`: scenario partly works or has limitations;
- `failed`: scenario does not work;
- `blocked`: scenario cannot yet be evaluated;
- `deferred`: scenario intentionally postponed.

## VAL-001: Declare a simple task

Status: approved

Stakeholder needs: SN-010, SN-011, SN-012

A Rust developer declares a task with a queue size, task state, start handler, and one event handler.

Expected outcome:

- the declaration is compact and Rust-like;
- message handlers use implementation-neutral `fn` declarations rather than
  exposing the runtime lowering strategy;
- handlers access task user state through scoped context operations rather than
  a `self` receiver;
- the task has a generated message enum;
- the task has a generated context;
- the task has a generated handle;
- the event can be sent through a generated method.

Evidence type: executable example or API walkthrough

Candidate example: `examples/ping_pong.rs`

## VAL-002: Start task predictably

Status: approved

Stakeholder needs: SN-041

A developer spawns a task and relies on the start handler to initialize state before any other application message is handled.

Expected outcome:

- task creation enqueues the start message;
- the start message is priority;
- the start message is received first;
- later handlers can rely on initialized state.

Evidence type: integration test and example

Candidate example: `examples/ping_pong.rs`

## VAL-003: Send an asynchronous event

Status: approved

Stakeholder needs: SN-012, SN-013, SN-014

A developer sends an event to a task through a generated task-handle method.

Expected outcome:

- the caller does not manually construct a message enum;
- queue-full is returned as an explicit error;
- the receiving task dispatches the event to the correct handler.

Evidence type: executable example or integration test

Candidate example: `examples/ping_pong.rs`

## VAL-004: Perform a typed synchronous call

Status: approved

Stakeholder needs: SN-013, SN-015, SN-023, SN-042

A task handler calls another task and awaits a typed reply.

Expected outcome:

- caller code looks like `server.get(ctx, key).await?`;
- the caller does not manually allocate or match session IDs;
- the response type is checked;
- the caller task continues processing other messages while waiting;
- the correct suspended handler resumes when the response arrives.
- a full caller queue does not cause the call response to be silently dropped.

Evidence type: integration test and example

Candidate example: `examples/synchronous_call.rs`

## VAL-005: Handle concurrent same-type calls

Status: approved

Stakeholder needs: SN-042

Two suspended handlers in the same caller task send the same call type to another task and receive replies out of order.

Expected outcome:

- each suspended handler resumes only for its own session;
- replies are matched by message kind and `SessionId`;
- no handler receives the other handler's reply.

Evidence type: integration test

## VAL-006: Reject undeclared receive at compile time

Status: approved

Stakeholder needs: SN-011, SN-023

A task attempts to await a response or stream event it has not declared it can receive.

Expected outcome:

- the code fails to compile;
- the error points toward missing receive declaration or trait bound;
- no runtime fallback is needed for this error.

Evidence type: compile-fail test

## VAL-007: Consume a stream with Rust-like API

Status: approved

Stakeholder needs: SN-010, SN-013, SN-016

A developer starts a streaming call and consumes it using a loop over `next(ctx).await`.

Expected outcome:

- caller code looks like ordinary Rust async control flow;
- stream batches are hidden from the caller;
- items are returned one at a time;
- end returns no further items;
- error returns a typed error.

Evidence type: executable example and integration test

Candidate example: `examples/streaming_query.rs`

## VAL-008: Cancel stream by dropping handle

Status: approved

Stakeholder needs: SN-017, SN-043

A consumer starts a stream and drops the stream object before it completes.

Expected outcome:

- the stream object attempts asynchronous cancellation;
- drop does not await acknowledgement;
- late stream replies invoke the task late-reply handler when the stream declaration does not opt out;
- late stream replies are unobserved by `mpi-rs` when the stream declaration uses `late_reply = "ignore"`;
- ordinary application messages are not silently discarded.

Evidence type: integration test

## VAL-009: Avoid flooding consumer queue

Status: approved

Stakeholder needs: SN-016, SN-043

A producer emits many stream items while the consumer processes them more slowly.

Expected outcome:

- batching reduces per-item message overhead;
- flow control prevents unbounded flooding of the consumer queue;
- stream handlers using `yield_item()` or `yield_batch()` suspend under no-credit backpressure instead of manually polling or retrying flow-control errors;
- full consumer queues cause stream reply production to suspend through runtime backpressure rather than exposing ordinary queue-full retry loops;
- the public API remains `next(ctx).await`.

Evidence type: integration test or analysis plus demonstration

## VAL-010: Use priority shutdown

Status: approved

Stakeholder needs: SN-040

A developer declares a shutdown event as priority and sends it while normal work is queued.

Expected outcome:

- shutdown placement is defined by the receiver's declaration;
- the task's default priority-reserved queue capacity leaves room for an urgent priority shutdown when normal messages fill the queue;
- the shutdown event is handled before queued normal messages;
- FIFO ordering among priority messages is preserved.

Evidence type: executable example and test

Candidate example: `examples/priority_shutdown.rs`

## VAL-011: Use external blocking API explicitly

Status: approved

Stakeholder needs: SN-010, SN-015

External code outside any task calls into a task and waits for a reply.

Expected outcome:

- the API is explicitly blocking, for example `get_blocking`;
- task-internal code has a separate context-aware API;
- developers can see which API may block a thread.

Evidence type: example or API walkthrough

## VAL-012: Forward OS events safely

Status: approved

Stakeholder needs: SN-044, SN-046

A developer forwards operating-system or framework events into a task as normal
Rust messages or typed interactions. The first implemented case forwards POSIX
signals into a task as normal Rust messages.

Expected outcome:

- the signal handler performs only async-signal-safe operations;
- a bridge task or thread constructs the normal message;
- the receiving task handles the signal as a normal event.
- Unix signal bridge support is available by default from `mpi-os-events` on Unix targets and can be removed from that crate's public API by disabling the default `unix-signals` feature;
- bridge documentation distinguishes asynchronous OS events that map to event messages from synchronous OS events that need calls or another explicit reply path;
- Linux, Windows, and macOS support use native OS event adapters;
- Android and iOS support use framework adapters, beginning with Tauri while preserving a path to other frameworks.

Evidence type: inspection and demonstration

Candidate example: `crates/mpi-os-events/examples/unix_signal_bridge.rs`

## VAL-013: Diagnose sessions and queues

Status: deferred

Stakeholder needs: SN-045

A developer investigates a timeout, late response, cancelled stream, or queue-full condition.

Expected outcome:

- diagnostics expose enough information to identify the affected task, session, and queue condition;
- unknown-session replies are available to the task late-reply handler unless their call or stream declaration uses `late_reply = "ignore"`;
- diagnostics do not require users to understand all internal macro expansion details.

Evidence type: demonstration or documentation review

## VAL-014: AI workflow implements from requirements

Status: approved

Stakeholder needs: SN-001, SN-002, SN-030, SN-032

The human maintainer updates a requirement and asks AI workflows to implement, test, review, validate, and update traceability.

Expected outcome:

- the implementation agent identifies affected requirements;
- the test agent adds or updates tests;
- the review agent checks against requirements and architecture;
- the validation agent evaluates affected scenarios;
- the traceability agent links the change;
- human approval remains required.

Evidence type: process demonstration through pull request evidence

## VAL-015: Share protocol across separate binaries

Status: proposed

Stakeholder needs: SN-018, SN-023, SN-024

Two separately built Rust applications depend on the same exported protocol
declaration and use a namespace-qualified protocol message for a typed call or
stream.

Expected outcome:

- both applications refer to the same namespace-qualified protocol message;
- request, reply, stream item, and stream error types are determined by the protocol declaration;
- a caller task cannot await an undeclared protocol reply or stream event;
- receive declarations match the protocol-qualified reply or stream event and its declared Rust type;
- generated protocol response identities follow the derived Rust naming convention, such as `get::Reply` for a call reply and `list_directories::Item`, `list_directories::Finish`, and `list_directories::Error` for stream replies;
- generated send, call, or stream methods are derived from the protocol or from a protocol-instance binding to the task that implements it;
- two protocols may reuse the same short message name without conflict;
- an incompatible message contract change is represented by a new protocol name.

Evidence type: compile-fail test, API walkthrough, or multi-crate example
