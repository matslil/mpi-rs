# Architecture

This document describes the authoritative `mpi-rs` architecture baseline.

It should be used to guide implementation, testing, review, and validation.

## Architecture goals

ARCH-001: `mpi-rs` shall provide message passing while exposing an idiomatic Rust API.

ARCH-002: The architecture shall separate runtime primitives from proc-macro generated task plumbing.

ARCH-003: The architecture shall support incremental implementation of queues, task handles, start messages, dispatch, async handlers, sessions, calls, streams, flow control, Unix signal bridging, and diagnostics.

ARCH-004: The architecture shall keep streams within the current task model rather than creating separate tasks merely to produce stream items.

## System context

`mpi-rs` is a Rust library used by applications that want task-oriented message passing.

Conceptually:

```text
Rust application
  |
  | declares tasks, handlers, events, calls, streams
  v
mpi-rs macros
  |
  | generate task handles, message enums, contexts, dispatch, protocol plumbing
  v
mpi-rs runtime
  |
  | manages queues, receive loop, sessions, streams, cancellation, diagnostics
  v
OS threads and synchronization primitives
```

## Major components

| ID | Component | Responsibility |
|---|---|---|
| CMP-001 | Task | Owns state, queue, context, and dispatch loop on one OS thread. |
| CMP-002 | TaskHandle | Public send surface used by other code to enqueue messages to a task. |
| CMP-003 | TaskQueue | Bounded queue with separate internal FIFO queues for normal and priority messages. |
| CMP-004 | TaskContext | Generated handler context containing self handle, session allocation, task-local receive state, and control operations. |
| CMP-005 | TaskMessage | Trait implemented by generated task message enums to expose receiver-declared placement. |
| CMP-006 | Dispatch loop | Receives messages, resumes matching waiters, or dispatches messages to handlers. |
| CMP-007 | Task-local runtime | Runs async handlers and suspended continuations without blocking the task OS thread. |
| CMP-008 | Session subsystem | Allocates and matches `SessionId` values for calls and streams. |
| CMP-009 | Call subsystem | Sends typed call requests and typed `Response<T>` messages. |
| CMP-010 | Stream subsystem | Sends stream requests, stream events, stream cancellation, and stream flow-control messages. |
| CMP-011 | Compile-time receive check subsystem | Ensures a caller task can only wait for messages it declares it can receive. |
| CMP-012 | Macro crate | Generates task message enums, contexts, handles, send methods, dispatch plumbing, and protocol integration. |
| CMP-013 | Unix signal bridge | Converts POSIX signal notifications into normal Rust messages outside signal-handler context. |
| CMP-014 | Diagnostics subsystem | Supports tracing, timeouts, deadlock/debug support, session debugging, and queue diagnostics. |

## Suggested crate structure

The initial conceptual crate structure is:

```text
crates/
  mpi/
    src/
      lib.rs
      task.rs
      queue.rs
      message.rs
      runtime.rs
      receive.rs
      session.rs
      stream.rs
      error.rs
      signal.rs
  mpi-macros/
    src/
      lib.rs
      task_macro.rs
      message_macro.rs
examples/
  ping_pong.rs
  synchronous_call.rs
  streaming_query.rs
  priority_shutdown.rs
```

The implementation may evolve, but deviations should be reported in implementation and review evidence.

## Task architecture

A task consists of:

- a task state object;
- a task handle used by other code to send messages;
- a bounded message queue;
- a generated task context passed to handlers;
- a dispatch loop running on one OS thread.

Architecture rules:

ARCH-010: Each task owns exactly one logical message queue.

ARCH-011: Each task has a generated message enum representing received messages.

ARCH-012: Each task has a generated context type passed to handlers.

ARCH-013: Each task has a generated handle type exposing send methods.

ARCH-014: Task initialization is performed through the start message; there is no separate normal out-of-band initialization path.

## Queue architecture

The queue uses two internal FIFO queues:

- normal queue;
- priority queue.

Architecture rules:

ARCH-020: Normal messages are inserted at the tail of the normal queue.

ARCH-021: Priority messages are inserted at the tail of the priority queue.

ARCH-022: Receive first tries the head of the priority queue, then the head of the normal queue.

ARCH-023: Total queue capacity is shared between normal and priority queues.

ARCH-024: Message placement is determined by the receiving task's declaration.

ARCH-025: The sender cannot override placement.

## Start message architecture

Task creation sequence:

```text
create task queue
enqueue Start message as priority
spawn OS thread running task loop
first receive returns Start message
Start handler initializes task state
```

Architecture rules:

ARCH-030: Start messages are forced to priority.

ARCH-031: No other priority message can be enqueued before the start message during task creation.

ARCH-032: FIFO ordering within priority messages guarantees the start message is received first.

## Handler execution and selective receive architecture

Handlers are intended to be async functions executed by a task-local runtime.

When a handler awaits a reply or stream event:

```text
handler sends request
handler awaits reply/event
handler continuation is suspended
task returns to receive loop
task handles other messages
matching reply/event arrives
suspended handler resumes
```

Architecture rules:

ARCH-040: Task-internal waits suspend handler continuations.

ARCH-041: Task-internal waits do not block the task OS thread.

ARCH-042: The receive loop checks suspended waiters before normal handler dispatch.

ARCH-043: Waiter matching uses message kind and `SessionId` for protocol messages.

## Session architecture

`SessionId` identifies a logical interaction that can produce future messages.

Architecture rules:

ARCH-050: `SessionId` is shared by calls, streams, cancellation, matching, late-event handling, tracing, and debugging.

ARCH-051: `SessionId` is a logical interaction identifier, not a physical message identifier.

ARCH-052: The preferred structure is `origin: EndpointId` plus `sequence: u64`.

ARCH-053: A task context allocates session IDs using task-local sequence state.

ARCH-054: Protocol messages that belong to a session implement or support a `HasSessionId` mechanism.

## Call architecture

A synchronous call is a request message followed by exactly one response message.

Architecture rules:

ARCH-060: Call requests carry a `SessionId` and reply address.

ARCH-061: Responses are represented using a typed `Response<T>` wrapper containing `session_id` and `value`.

ARCH-062: A call handler can return the reply payload; runtime or macro plumbing converts it into `Response<T>`.

ARCH-063: Concurrent calls with the same request and response type are disambiguated by `SessionId`.

ARCH-064: Late one-shot responses are surfaced to fallback reply handling or task policy rather than silently discarded by default.

## Stream architecture

A stream is a request message followed by zero or more stream events and then end, error, or cancellation.

Architecture rules:

ARCH-070: Streams use the same `SessionId` model as calls.

ARCH-071: Stream events include `Batch`, `End`, and `Error` variants.

ARCH-072: The public stream API returns one item at a time while internally supporting batches.

ARCH-073: Dropping an unfinished stream object attempts asynchronous cancellation.

ARCH-074: Stream cancellation uses the same `SessionId` as the stream.

ARCH-075: Stream producers remain within the current task model.

ARCH-076: Credit-based flow control is the recommended mechanism to avoid flooding the consumer queue.

ARCH-077: A stream producer may suspend when backpressured or waiting for stream control messages while the task continues handling other messages.

ARCH-078: A future `futures_core::Stream` implementation may be added only if it preserves safe access to task-local receive state.

## External caller architecture

External callers do not have task queues.

Architecture rules:

ARCH-080: External blocking APIs may use one-shot channels internally.

ARCH-081: External blocking APIs must be explicit, for example `get_blocking`.

ARCH-082: Task-internal APIs must remain distinct from external blocking APIs.

## Unix signal architecture

POSIX signal handlers cannot safely allocate normal Rust messages.

Architecture rules:

ARCH-090: The signal handler part performs only async-signal-safe operations.

ARCH-091: A signal bridge task or thread observes signal state or notification.

ARCH-092: The signal bridge constructs and sends normal Rust messages outside signal-handler context.

## Diagnostics architecture

Diagnostics are not first-phase implementation, but architecture should preserve room for:

- queue diagnostics;
- session tracing;
- stream lifecycle tracing;
- timeout reporting;
- deadlock/debug support;
- late response and late stream event reporting.

## Implementation phases

Recommended order:

1. Bounded task queues with separate FIFO queues for normal and priority messages.
2. Task handles and generated event send methods.
3. Start message and task spawn API, with start forced to priority and guaranteed first.
4. Generated message enums, task context types, and dispatch loops.
5. Async handlers run by a task-local executor.
6. Selective receive by message kind.
7. `SessionId`, `Response<T>`, and synchronous calls.
8. Compile-time `CanReceive<T>` checks for responses.
9. `StreamEvent<T, E>`, `MessageStream<T, E>`, and stream cancellation.
10. Stream batching and credit-based flow control.
11. Safe Unix signal bridge.
12. Diagnostics, timeouts, tracing, and deadlock/debug support.
