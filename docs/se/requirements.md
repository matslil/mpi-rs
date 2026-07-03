# Requirements

This document defines the initial lightweight systems-engineering requirements for `mpi-rs`.

The requirements are derived from `docs/message-model.md` and are intended to be testable, inspectable, and useful for AI-assisted implementation.

Status values are defined in `docs/agents/process.md`.

## Process requirements

### REQ-001: Authoritative source handling

The repository shall treat `docs/message-model.md` as the current conceptual source of truth until the human maintainer approves a replacement or update.

Rationale: The user explicitly identified `docs/message-model.md` as the only current repository source to use.

Verification: inspection

Status: approved

### REQ-002: Agent role separation

The repository shall define separate instructions for system engineering, implementation, testing, review, validation, traceability, and release activities.

Rationale: The maintainer wants separate AI workflows to reduce self-confirming errors and support human-in-the-loop review.

Verification: inspection

Status: approved

### REQ-003: Human approval

The process shall require human maintainer approval before merge to `main`.

Rationale: The maintainer intends to keep a human in the loop for actual merge decisions.

Verification: inspection

Status: approved

## Task model requirements

### REQ-010: Task abstraction

A task shall be modeled as an operating-system thread with an associated bounded message queue.

Source: SN-010, SN-040

Verification: test

Status: approved

### REQ-011: Task-owned queue

Every task shall own its message queue.

Source: SN-040

Verification: inspection

Status: approved

### REQ-012: Static queue capacity

Each task shall have one static queue capacity configured at compile time.

Source: SN-014, SN-040

Verification: test

Status: approved

### REQ-013: Shared queue capacity

The task queue capacity shall be shared between normal and priority messages.

Source: SN-014, SN-040

Verification: test

Status: approved

### REQ-014: Queue-full error

A generated send method shall return an explicit queue-full error when the target queue is full.

Source: SN-014

Verification: test

Status: approved

### REQ-015: Generated task context

Each task shall have a generated task context type passed to handlers.

Source: SN-012, SN-015

Verification: inspection

Status: approved

## Message declaration requirements

### REQ-020: Message umbrella term

The term `Message` shall be the umbrella term for every item that can be received by a task.

Source: SN-020

Verification: inspection

Status: approved

### REQ-021: User-visible message categories

The user-visible message categories shall include events, calls, and streams.

Source: SN-013, SN-020

Verification: inspection

Status: approved

### REQ-022: Event definition

An event shall be an asynchronous message that does not expect a reply.

Source: SN-013, SN-020

Verification: inspection

Status: approved

### REQ-023: Call definition

A call shall be a synchronous message exchange with exactly one response.

Source: SN-013, SN-042

Verification: test

Status: approved

### REQ-024: Stream definition

A stream shall be a synchronous or generator-style message exchange with zero or more stream reply events followed by end, error, or cancellation.

Source: SN-013, SN-016, SN-043

Verification: test

Status: approved

### REQ-025: Compile-time receive declaration

A task shall declare at compile time which messages it can receive.

Source: SN-011

Verification: inspection

Status: approved

### REQ-026: Generated message enum

Each task shall have one generated message enum representing the messages it can receive.

Source: SN-011, SN-012

Verification: inspection

Status: approved

### REQ-027: Generated send methods

Sending a user-visible message shall be done by calling a generated method on the receiving task handle.

Source: SN-012

Verification: demonstration

Status: approved

### REQ-028: Direct enqueue by send method

A generated send method shall construct the message and enqueue it directly into the receiving task queue.

Source: SN-012, SN-014

Verification: inspection

Status: approved

## Queue placement requirements

### REQ-030: Receiver-declared placement

Message placement shall be declared by the receiving task, not chosen by the sender.

Source: SN-040

Verification: inspection

Status: approved

### REQ-031: Placement classes

The queue placement classes shall include `Normal` and `Priority`.

Source: SN-040

Verification: inspection

Status: approved

### REQ-032: Normal FIFO ordering

Normal messages shall be inserted at the tail of the normal queue and received in FIFO order relative to other normal messages.

Source: SN-040

Verification: test

Status: approved

### REQ-033: Priority FIFO ordering

Priority messages shall be inserted at the tail of the priority queue and received in FIFO order relative to other priority messages.

Source: SN-040

Verification: test

Status: approved

### REQ-034: Priority-before-normal receive order

The receive operation shall return a priority message before any normal message whenever the priority queue is non-empty.

Source: SN-040

Verification: test

Status: approved

### REQ-035: Independent request and reply placement

For calls and streams, request placement and response or stream-event placement shall be independently declared by the receiving task for each received message.

Source: SN-040, SN-042

Verification: inspection

Status: approved

## Start message requirements

### REQ-040: Start message creation

Task creation shall create and enqueue a start message for the new task.

Source: SN-041

Verification: test

Status: approved

### REQ-041: Start message forced priority

The generated start message shall be priority even if the user does not explicitly declare it as priority.

Source: SN-041

Verification: test

Status: approved

### REQ-042: Start message first

The first application message received by a newly spawned task shall be its start message.

Source: SN-041

Verification: test

Status: approved

### REQ-043: No separate initialization path

The task model shall not require a separate out-of-band task initialization path for normal task startup.

Source: SN-041

Verification: inspection

Status: approved

## Macro and generated code requirements

### REQ-050: Task macro name

The macro used for task declarations shall be named `task`.

Source: SN-010, SN-020

Verification: inspection

Status: approved

### REQ-051: Task macro generated artifacts

The `#[task]` macro shall generate or support the task message enum, task context type, task handle, handle send methods, queue placement implementation, dispatch plumbing, and call or stream plumbing.

Source: SN-012, SN-021

Verification: inspection

Status: approved

### REQ-052: Handler dispatch

The generated dispatch logic shall route each message enum variant to the corresponding handler.

Source: SN-012, SN-021

Verification: test

Status: approved

### REQ-053: Start handler priority handling

The macro shall either reject an explicit normal placement for a start handler or ignore it and force priority.

Source: SN-041

Verification: test

Status: approved

## Handler and receive requirements

### REQ-060: Async handlers

Handlers shall be expressible as async functions executed by a task-local runtime.

Source: SN-015

Verification: demonstration

Status: approved

### REQ-061: Suspended receive

When a handler awaits a specific reply or stream event, the handler continuation shall suspend instead of blocking the task OS thread.

Source: SN-015

Verification: test

Status: approved

### REQ-062: Continued task processing during wait

While one handler is suspended waiting for a reply or stream event, the task shall continue receiving and handling other messages.

Source: SN-015

Verification: test

Status: approved

### REQ-063: Waiter matching

The receive loop shall match suspended waiters by message kind and by `SessionId` for protocol messages.

Source: SN-042

Verification: test

Status: approved

## Compile-time receive check requirements

### REQ-070: Response receive declaration

A task-internal synchronous call shall compile only when the caller task declares that it can receive the corresponding response message.

Source: SN-023

Verification: test

Status: approved

### REQ-071: Stream event receive declaration

A task-internal streaming call shall compile only when the caller task declares that it can receive the corresponding stream event message.

Source: SN-023

Verification: test

Status: approved

### REQ-072: Generated receive trait

The implementation shall provide or generate a mechanism equivalent to a `CanReceive<T>` trait for compile-time receive checks.

Source: SN-023

Verification: inspection

Status: approved

## Session requirements

### REQ-080: Universal SessionId

`mpi-rs` shall use a universal `SessionId` for logical interactions that expect future messages.

Source: SN-042

Verification: inspection

Status: approved

### REQ-081: SessionId scope

`SessionId` shall identify a logical exchange rather than a single physical queued message.

Source: SN-042

Verification: inspection

Status: approved

### REQ-082: SessionId uses

`SessionId` shall be used for synchronous calls, streaming calls, cancellation, reply matching, late stream event handling, tracing, and debugging.

Source: SN-042, SN-043, SN-045

Verification: inspection

Status: approved

### REQ-083: SessionId structure

`SessionId` should contain an origin endpoint identifier and a sequence number.

Source: SN-042

Verification: inspection

Status: approved

### REQ-084: Task-local sequence

A task context shall be able to allocate task-local session sequence values without global synchronization.

Source: SN-042

Verification: test

Status: approved

## Synchronous call requirements

### REQ-090: Typed response wrapper

A synchronous call response shall use a typed response wrapper containing the session ID and reply value.

Source: SN-013, SN-042

Verification: inspection

Status: approved

### REQ-091: Exactly one call response

A synchronous call shall complete with exactly one response, error, or defined failure outcome.

Source: SN-013, SN-042

Verification: test

Status: approved

### REQ-092: Out-of-order response matching

If two suspended handlers in the same caller task send the same call type and responses arrive out of order, each handler shall resume only for its own `SessionId`.

Source: SN-042

Verification: test

Status: approved

### REQ-093: Callee return conversion

A call handler may be written as a Rust function returning the reply payload, and the macro/runtime shall convert that value into the typed response message.

Source: SN-010, SN-013

Verification: test

Status: approved

## Stream requirements

### REQ-100: Stream event variants

A stream shall represent producer output using typed `Batch`, `End`, and `Error` stream events.

Source: SN-016, SN-043

Verification: inspection

Status: approved

### REQ-101: Rust-like stream consumer API

The stream consumer API shall expose a Rust-like `next(ctx).await` operation that returns one item at a time.

Source: SN-010, SN-016

Verification: demonstration

Status: approved

### REQ-102: Stream batching

Streams shall support batching so one stream event may carry multiple items.

Source: SN-016

Verification: test

Status: approved

### REQ-103: Local stream buffer

The stream consumer shall drain a local buffer before awaiting another stream event.

Source: SN-016

Verification: test

Status: approved

### REQ-104: Stream end handling

When a stream consumer receives an end event for its session, `next(ctx).await` shall complete the stream and return no further items.

Source: SN-016, SN-043

Verification: test

Status: approved

### REQ-105: Stream error handling

When a stream consumer receives an error event for its session, `next(ctx).await` shall complete the stream with that error.

Source: SN-016, SN-043

Verification: test

Status: approved

### REQ-106: Drop cancellation

Dropping an unfinished stream object shall attempt to send an asynchronous cancellation message for the stream session.

Source: SN-017, SN-043

Verification: test

Status: approved

### REQ-107: Cancellation priority guidance

Stream cancellation messages should usually be priority in the producer task.

Source: SN-017, SN-043

Verification: inspection

Status: approved

### REQ-108: Late stream event discard

The receive logic may discard or log-and-discard stream events whose `SessionId` no longer has an active waiter or stream object.

Source: SN-043

Verification: test

Status: approved

### REQ-109: No silent discard of ordinary messages

Ordinary application messages shall not be silently discarded under the late stream event rule.

Source: SN-040, SN-043

Verification: test

Status: approved

### REQ-110: Producer within task model

A streaming handler shall remain within the current task model and shall not create a new task merely to produce stream items.

Source: SN-021, SN-043

Verification: inspection

Status: approved

### REQ-111: Automatic stream completion events

The runtime shall send a stream end event when a streaming handler returns success and a stream error event when it returns an error.

Source: SN-016, SN-043

Verification: test

Status: approved

### REQ-112: Credit-based flow control

The stream design shall prevent a producer from flooding the consumer queue, with credit-based flow control as the recommended baseline.

Source: SN-016, SN-043

Verification: test

Status: approved

## External caller requirements

### REQ-120: Explicit external blocking API

External callers that do not have a task queue may use explicit blocking APIs such as `get_blocking`.

Source: SN-010, SN-015

Verification: demonstration

Status: approved

### REQ-121: No accidental internal blocking

Task-internal APIs shall distinguish themselves from external blocking APIs so handlers do not accidentally block the task thread.

Source: SN-015

Verification: inspection

Status: approved

## Unix signal requirements

### REQ-130: Async-signal-safe signal handling

The POSIX signal handler portion of any Unix signal support shall perform only async-signal-safe operations.

Source: SN-044

Verification: inspection

Status: approved

### REQ-131: Signal bridge

Unix signal forwarding shall use a bridge that observes async-signal-safe state or notification and then sends normal Rust messages outside the signal handler.

Source: SN-044

Verification: inspection

Status: approved

## Diagnostics requirements

### REQ-140: Diagnostics roadmap

The implementation shall include or preserve a roadmap for diagnostics, timeouts, tracing, and deadlock/debug support.

Source: SN-045

Verification: inspection

Status: approved

## Implementation phase requirements

### REQ-150: Incremental implementation order

Implementation should follow the phases in `docs/message-model.md` unless the human maintainer approves a different order.

Source: SN-022

Verification: inspection

Status: approved

### REQ-151: Suggested crate structure

The repository should organize runtime primitives and macros along the conceptual crate structure described in `docs/message-model.md`, unless implementation evidence justifies a different structure.

Source: SN-021, SN-022

Verification: inspection

Status: approved
