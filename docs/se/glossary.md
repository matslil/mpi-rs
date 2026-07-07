# Glossary

This glossary defines the baseline terminology for `mpi-rs`.

Use these terms consistently in requirements, architecture, interfaces, tests, code comments, and agent reports.

## Core terms

### Message

The umbrella term for every item that can be received by a task.

Messages include start messages, events, synchronous call requests, synchronous call responses, stream start requests, stream pull control messages, stream reply events, stream cancel control messages, and internal runtime/control messages when needed.

### Event

An asynchronous message that does not expect a reply.

### Call

A synchronous message exchange with exactly one response.

### Stream

A synchronous or generator-style message exchange with zero or more stream reply events followed by end, error, or cancellation.

### Task

An operating-system thread with an associated bounded message queue, task state, generated context, generated handle, and dispatch loop.

### Task handle

A generated handle used by other code to send messages to a task.

### Task context

A generated context passed to handlers. It provides access to task-local runtime behavior such as self handle, session allocation, suspended receive, and control operations.

### Task message enum

The generated enum representing all messages a specific task can receive.

### Handler

A function associated with a received message. Handlers process start messages, events, calls, streams, or internal protocol messages, and may suspend through task-internal call or stream APIs.

### Dispatch loop

The task loop that receives messages, checks suspended waiters, resumes matching continuations, or dispatches messages to handlers.

### CtxFuture

A future-like resumable computation that receives mutable context during resume and does not retain that context borrow after suspension.

### Context-returning suspension

A suspension model where a continuation returns control and mutable context access to the scheduler whenever it cannot make progress.

## Queue terms

### Queue placement

The class of queue into which a message is inserted for a receiver.

### Normal

The default queue placement. Normal messages are placed in the normal FIFO queue.

### Priority

A queue placement for messages that should be received before normal messages. Priority messages are still FIFO relative to other priority messages.

### Receiver-declared placement

The rule that message placement is declared by the receiving task's message contract, not chosen by the sender at each send site.

### Queue-full error

An explicit send failure returned when a generated send method attempts to enqueue a message into a full target queue.

## Startup terms

### Start message

The special first application message enqueued during task creation. It is forced to priority and must be the first application message received by the new task.

### Start handler

The handler for the start message. It initializes task state used by later handlers.

## Protocol terms

### Protocol

A named, exported message contract that defines protocol messages and their
explicit payload, reply, stream item, or stream error types.

### Protocol namespace

The Rust module or crate path that qualifies a protocol declaration.

### Protocol message identity

The namespace-qualified identity of a protocol message, formed from protocol
namespace, protocol name, and message name.

### Protocol-instance binding

A generated or declared binding from a protocol message contract to the concrete
task, endpoint, or handle that implements that protocol instance.

### Protocol-derived send surface

Generated send, call, or stream methods whose message identity and Rust types
come from a protocol declaration, optionally through a protocol-instance
binding.

### Append-only protocol

A published protocol that may add new message declarations but does not change
or remove existing protocol message declarations or their associated types.

### Breaking protocol change

An incompatible change to a protocol message declaration or its associated
types. A breaking protocol change requires a new protocol name.

### SessionId

A logical interaction identifier for exchanges that can produce future messages.

`SessionId` is used for synchronous calls, streaming calls, cancellation, reply matching, late reply handling, tracing, and debugging.

### EndpointId

An identifier for the task or external endpoint that originated a session.

### Sequence

A task-local or endpoint-local sequence number used with an origin endpoint to form a `SessionId`.

### Response<T>

A typed response wrapper for synchronous calls. It contains the session ID and reply value.

### CanReceive<T>

A conceptual generated trait or equivalent mechanism used to check at compile time that a task can receive a given response or stream event type.

### HasSessionId

A conceptual trait or equivalent mechanism used by protocol messages that carry a session ID.

## Stream terms

### StreamEvent<T, E>

A typed stream event that carries one of:

- a batch of values;
- end of stream;
- stream error.

### Batch

A stream event variant carrying multiple stream items in one queued message.

### End

A stream event variant indicating normal completion of a stream.

### Error

A stream event variant indicating stream failure with a typed error.

### MessageStream<T, E>

The conceptual consumer-side stream object. It owns a session ID, stream control handle, local buffer, and completion state.

### StreamSink<T, E>

The conceptual producer-side stream object used by a streaming handler to emit items.

### StreamPull

A hidden stream control message that grants producer credit.

### StreamCancel

A hidden stream control message that cancels a stream by session ID.

### Credit-based flow control

A flow-control scheme where the consumer grants item credits and the producer may send up to the granted amount.

### Late reply

A call response or stream reply that arrives after the receiving task no longer has an active waiter or stream object for the reply's `SessionId`.

### Late reply policy

The declaration-derived runtime policy for an unknown-session reply. The default policy is `report`; `late_reply = "ignore"` explicitly makes the unknown-session reply unobservable to `mpi-rs`.

### Late reply handler

An optional task handler that receives a borrowed `LateReplyRef` for a reported late reply and returns a `LateReplyAction`.

### LateReplyRef

A read-only borrowed view of a late reply. It exposes the reply `SessionId`, late-reply kind, and downcast access to the reply payload for the duration of the late-reply handler call.

### LateReplyAction

The result of a late-reply handler. `Ignore` continues the task after the late reply is dropped, and `Terminate` stops the task.

### Late stream event

A stream event that arrives after the stream has been cancelled, dropped, or otherwise no longer has an active consumer session.

## API terms

### Task-internal API

An API used from inside a task handler. It uses task context and must not block the task OS thread while waiting for replies or stream events.

### External blocking API

An API used from outside a task. It may block the calling thread and should be explicitly named as blocking or otherwise clearly distinct.

## Signal terms

### POSIX signal handler

The low-level function or handler invoked by the operating system for a Unix signal. It must perform only async-signal-safe operations.

### Signal bridge

A task or thread that observes signal-handler-safe state or notifications and sends normal Rust messages outside signal-handler context.

## Process terms

### Requirement

A testable or inspectable obligation stated with `shall`.

### Verification

Evidence that the implementation satisfies the approved requirements.

### Validation

Evidence that the implementation satisfies stakeholder needs and intended usage scenarios.

### Traceability

Links between stakeholder needs, requirements, architecture, interfaces, implementation, tests, verification evidence, and validation scenarios.

### Human system engineer

The human maintainer who defines and approves requirements, architecture, interfaces, and merge decisions.

### Agent

An AI workflow with a defined role, inputs, outputs, permissions, and completion criteria.
