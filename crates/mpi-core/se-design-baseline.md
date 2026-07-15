# mpi-core Design Baseline

This document defines the lightweight systems-engineering baseline for the `mpi-core` crate.

The `mpi-core` crate owns the message-passing runtime model, runtime types, task handles, queues, sessions, calls, streams, diagnostics support, gated core transaction identifiers, service instance runtime support, and public runtime interfaces. The `mpi` crate is a facade that re-exports this runtime surface and aggregates optional workspace crate features. Macro syntax and code generation responsibilities live in `crates/mpi-macros/se-design-baseline.md`; OS and framework event bridges live in `crates/mpi-os-events/se-design-baseline.md`; storage-backed transaction logging lives in `crates/mpi-transaction/se-design-baseline.md`; default crash-recovery log storage service behavior lives in `crates/persistent-log-storage-service/se-design-baseline.md`.

## Purpose

`mpi-core` provides the runtime and generated-code support for a message-passing model while the `mpi` facade keeps the user-facing API idiomatic for Rust.

Core concepts:

- a task is an operating-system thread with an associated bounded message queue;
- a task declares, at compile time, the messages it can receive;
- `Message` is the umbrella term for all received items;
- an event is an asynchronous message with no reply;
- a call is a synchronous exchange with exactly one typed response;
- a stream is a generator-style exchange with zero or more typed stream events followed by end, error, or cancellation;
- a service is a long-lived message-based capability exposed through a service
  instance that owns one running task and its protocol bindings;
- `SessionId` identifies logical interactions for calls and streams;
- `TransactionId` identifies an ACID transaction root that may contain several
  message interactions;
- `TransactionPath` identifies a root or nested transaction instance within a
  transaction hierarchy;
- normal and priority messages are placed according to the receiver's declaration;
- the start message is forced to priority and must be the first application message received by a new task;
- handlers suspend while waiting for replies or stream events instead of blocking the task thread.

### Lifecycle terminology

- **Task termination** is the irreversible end of a task runtime, classified as
  completed, stopped, or panicked.
- **Task supervision** is the infrastructure facility through which one task
  subscribes to another endpoint's termination.
- **Task monitor** is the cancellable task-scoped handle representing one
  supervision subscription.
- **Panic isolation** is the conversion of an unwinding task panic into panicked
  task termination without automatically propagating that panic to unrelated
  task threads or the application entry thread.

## Stakeholder Needs

The following original stakeholder need IDs remain part of this crate baseline:

- SN-010: Rust developers need an idiomatic task API rather than an API that feels like a foreign actor or signal system.
- SN-011: Rust developers need tasks to declare at compile time which messages they can receive.
- SN-012: Rust developers need generated task handles with send methods instead of manually constructing and enqueueing low-level message enums.
- SN-013: Rust developers need asynchronous events, typed synchronous calls, and generator-style streams to be expressed with clear Rust APIs.
- SN-014: Rust developers need queue-full errors to be explicit and recoverable.
- SN-015: Rust developers need task-internal synchronous waits to avoid blocking the task OS thread.
- SN-016: Rust developers need stream consumption to avoid one queued message per item by supporting batching behind a simple `next(ctx).await` API.
- SN-017: Rust developers need cancellation of dropped streams to be automatic and non-blocking.
- SN-020: Contributors need stable terminology for message, event, call, stream, task, placement, priority, response, and session.
- SN-021: Contributors need clear architecture and interface boundaries between runtime primitives, task macros, queues, receive logic, sessions, streams, and signal support.
- SN-022: Contributors need a practical implementation order that allows incremental development and review.
- SN-023: Contributors need compile-time receive checks to prevent handlers from waiting for undeclared response or stream event messages.
- SN-040: Runtime users need predictable queue ordering for normal and priority messages.
- SN-041: Runtime users need the start message to be the first application message received by a newly spawned task.
- SN-042: Runtime users need calls and streams to match replies by logical interaction so concurrent handlers do not receive each other's replies.
- SN-043: Runtime users need cancellation and late stream replies to be handled safely without hiding ordinary protocol flaws.
- SN-045: Maintainers and operators need diagnostics for sessions, queues, timeouts, deadlocks, unknown-session replies, and stream lifecycle issues.
- SN-046: Rust developers need ACID-style transaction scopes that can coordinate several typed message interactions while keeping invalid transaction participation visible at compile time.
- SN-047: Rust developers need hierarchical transactions so nested transactional work is represented explicitly rather than relying on ad hoc handler-local rollback code.
- SN-048: Maintainers and operators need transaction decisions to be recoverable after crashes once a transaction has crossed the durable commit decision point.
- SN-049: Transactional messaging needs a persistent log storage interface so crash recovery does not depend on ad hoc application-specific file writes.
- SN-050: Rust developers need repository-provided long-lived task capabilities
  to expose protocol bindings through an owning service instance whose lifetime
  reports stopped-task errors after the service task has stopped.
- SN-051: Rust developers need service shutdown to be synchronized by
  capability closure and runtime joining without external application
  messaging.
- SN-052: Task authors need to supervise another task and receive a reliable
  notification when that task terminates.
- SN-053: Application authors need an unwinding panic in one task to terminate
  that task and its active sessions without automatically terminating unrelated
  tasks or the application process.

## Scope

`mpi-core` is responsible for:

- bounded task queues;
- normal and priority queue ordering;
- task endpoints and task handles;
- task context runtime support;
- message placement traits and runtime message wrappers;
- `SessionId`, `EndpointId`, response matching, late replies, and diagnostics snapshots;
- task-internal calls and streams;
- task-scope-only application messaging;
- sender/session message headers and type-erased proxy envelopes;
- compile-time receive-check traits consumed by generated code;
- runtime transaction identifiers and transaction paths;
- compile-time surface for generated transactional message APIs.
- runtime support for service instance lifetime and service stop
  synchronization.
- endpoint lifecycle state, task supervision subscriptions, and panic isolation.

`mpi` is not responsible for:

- proc-macro parsing or code generation;
- facade-level feature aggregation or optional crate re-export naming;
- native OS or framework event capture;
- providing the default file-backed persistent log storage implementation;
- providing the storage-backed transaction decision log adapter;
- defining crate-specific service protocols or service task state;
- the standalone `ctx-future` implementation.

## Requirements

The original workspace-level `REQ-*` IDs remain stable after migration. The
`MPI-REQ-*` IDs below are local grouping aliases; they do not replace the
original IDs used by tests, reports, and traceability.

### Migrated stable requirements

- REQ-010: A task shall be modeled as an operating-system thread with an associated bounded message queue.
- REQ-011: Every task shall own its message queue.
- REQ-012: Each task shall have one static queue capacity configured at compile time.
- REQ-013: The task queue capacity shall be shared between normal and priority messages.
- REQ-014: A generated send method shall return an explicit queue-full error when the target queue is full.
- REQ-014A: The receiver-side runtime shall be able to register a sending task for a future queue-capacity reservation when a task-internal send cannot enqueue because receiver capacity is unavailable.
- REQ-014B: A receiving task endpoint shall register each sending task at most once in its queue-capacity wait list and consider registered senders in FIFO order.
- REQ-014C: A sender with a receiver-owned reservation shall consume that reservation before using unreserved queue capacity on its next enqueue to that receiver.
- REQ-014D: Queue-capacity wakeups shall be framework-only messages generated by `mpi-rs`, not user-declared protocol or task messages.
- REQ-014E: If a queue-capacity wakeup cannot be delivered, the receiver shall release that sender's reservation and may continue with the next registered sender.
- REQ-015: Each task shall have a generated task context type passed to handlers.
- REQ-020: `Message` shall be the umbrella term for every item that can be received by a task.
- REQ-021: User-visible message categories shall include events, calls, and streams.
- REQ-022: An event shall be an asynchronous message that does not expect a reply.
- REQ-023: A call shall be a synchronous message exchange with exactly one response.
- REQ-024: A stream shall be a synchronous or generator-style message exchange with zero or more stream reply events followed by end, error, or cancellation.
- REQ-025: A task shall declare at compile time which messages it can receive.
- REQ-030: Message placement shall be declared by the receiving task, not chosen by the sender.
- REQ-031: Queue placement classes shall include `Normal` and `Priority`.
- REQ-032: Normal messages shall be inserted at the tail of the normal queue and received in FIFO order relative to other normal messages.
- REQ-033: Priority messages shall be inserted at the tail of the priority queue and received in FIFO order relative to other priority messages.
- REQ-034: Receive shall return a priority message before any normal message whenever the priority queue is non-empty.
- REQ-035: For calls and streams, request placement and response or stream-event placement shall be independently declared by the receiving task for each received message.
- REQ-036: Each task queue shall reserve configurable capacity for priority messages so normal messages cannot consume all queue slots; the default shall be one slot.
- REQ-037: The task declaration interface shall allow priority-reserved queue capacity to be configured per task.
- REQ-040: Task creation shall create and enqueue a start message for the new task.
- REQ-041: The generated start message shall be priority even if the user does not explicitly declare it as priority.
- REQ-042: The first application message received by a newly spawned task shall be its start message.
- REQ-043: The task model shall not require a separate out-of-band task initialization path for normal task startup.
- REQ-060: Handlers shall be expressible as async functions executed by a task-local runtime.
- REQ-061: Awaiting a specific reply or stream event shall suspend the handler continuation instead of blocking the task OS thread.
- REQ-062: While one handler is suspended waiting for a reply or stream event, the task shall continue receiving and handling other messages.
- REQ-063: The receive loop shall match suspended waiters by message kind and by `SessionId` for protocol messages.
- REQ-064: Task-local suspended handlers shall use a context-returning suspension primitive that does not retain mutable borrows of task state or task context while suspended.
- REQ-070: A task-internal synchronous call shall compile only when the caller task declares that it can receive the corresponding response message.
- REQ-071: A task-internal streaming call shall compile only when the caller task declares that it can receive the corresponding stream event message.
- REQ-072: The implementation shall provide or generate a mechanism equivalent to a `CanReceive<T>` trait for compile-time receive checks.
- REQ-080: `mpi-rs` shall use a universal `SessionId` for logical interactions that expect future messages.
- REQ-081: `SessionId` shall identify a logical exchange rather than a single physical queued message.
- REQ-082: `SessionId` shall be used for synchronous calls, streaming calls, cancellation, reply matching, late reply handling, tracing, and debugging.
- REQ-083: `SessionId` should contain an origin endpoint identifier and a sequence number.
- REQ-084: A task context shall be able to allocate task-local session sequence values without global synchronization.
- REQ-090: A synchronous call response shall use a typed response wrapper containing the session ID and reply value.
- REQ-091: A synchronous call shall complete with exactly one response, error, or defined failure outcome.
- REQ-092: Concurrent same-type calls in the same caller task shall resume only for their own `SessionId`, including out-of-order responses.
- REQ-093: A call handler may return the reply payload, with macro/runtime plumbing converting it into the typed response message.
- REQ-094: Late one-shot responses shall invoke the receiving task's late-reply callback unless `late_reply = "ignore"` applies.
- REQ-095: A call or stream declaration may declare `late_reply = "ignore"` to make unknown-session replies intentionally unobservable to `mpi-rs`.
- REQ-096: A task late-reply callback shall receive a borrowed reference to the unexpected reply and return either ignore or terminate.
- REQ-097: If no late-reply callback is declared, the default late-reply callback shall take no action and return ignore.
- REQ-098: Task-internal call responses that cannot enqueue because the caller queue is full shall suspend through the queue-capacity reservation mechanism rather than being dropped.
- REQ-100: A stream shall represent producer output using typed `Batch`, `End`, and `Error` stream events.
- REQ-101: The stream consumer API shall expose a Rust-like `next(ctx).await` operation that returns one item at a time.
- REQ-102: Streams shall support batching so one stream event may carry multiple items.
- REQ-103: The stream consumer shall drain a local buffer before awaiting another stream event.
- REQ-104: Receiving an end event for a session shall complete the stream and return no further items.
- REQ-105: Receiving an error event for a session shall complete the stream with that error.
- REQ-106: Dropping an unfinished stream object shall attempt to send an asynchronous cancellation message for the stream session.
- REQ-107: Stream cancellation messages should usually be priority in the producer task.
- REQ-108: Late stream replies shall invoke the receiving task's late-reply callback unless `late_reply = "ignore"` applies.
- REQ-109: Ordinary application messages shall not be silently ignored under the late reply policy.
- REQ-110: A streaming handler shall remain within the current task model and shall not create a new task merely to produce stream items.
- REQ-111: The runtime shall send stream end on streaming-handler success and stream error on streaming-handler error.
- REQ-112: The stream design shall prevent a producer from flooding the consumer queue, with credit-based flow control as the recommended baseline.
- REQ-113: A stream producer may suspend when backpressured or waiting for stream control messages while the task continues handling other messages.
- REQ-114: A future `futures_core::Stream` implementation may be added only if it preserves safe access to task-local receive state.
- REQ-115: Producer-side `yield_item()` and `yield_batch()` shall suspend under insufficient credit until credit, cancellation, or another terminal stream-control condition.
- REQ-116: Stream item, end, and error replies that cannot enqueue because the consumer queue is full shall suspend through the queue-capacity reservation mechanism.
- REQ-120: Obsolete; external application messaging is not supported.
- REQ-121: Application messaging APIs shall require task scope so handlers do not block the task thread and external code cannot send or receive messages.
- REQ-122: External code may construct tasks, inject creation-time dependencies, and join runtimes, but shall not send or receive application or protocol messages.
- REQ-123: Generated messaging APIs shall require generated task context access.
- REQ-127: Ordinary protocol messages shall carry infrastructure-owned sender and `SessionId` metadata plus their declared payload.
- REQ-128: Proxy forwarding shall use an outer message whose payload is a complete type-erased inner message, causing replies to traverse the proxy.
- REQ-124: Task instances participating in `mpi-rs` task-to-task message passing shall be declared in a topology known to generated code at compile time.
- REQ-125: Repeated task instances of the same task type shall use a compile-time-known array size or equivalent const generic value.
- REQ-126: The core task model shall not require runtime-discovered task instances for task-internal call, stream, reply, or suspension routing.
- REQ-140: The implementation shall include or preserve a roadmap for diagnostics, timeouts, tracing, and deadlock/debug support.
- REQ-180: A service start function shall return a service instance that owns
  one running service task and exposes that task's protocol bindings.
- REQ-181: Dropping the final clone of a service instance shall close its
  service capability and join the service runtime without sending an
  application or protocol message from external scope.
- REQ-182: Service shutdown synchronization shall use capability closure and
  runtime join rather than an externally initiated stop call.
- REQ-183: Protocol bindings cloned from a service instance may outlive the
  service instance object, but sends and calls through those bindings shall fail
  with task-stopped behavior after the service task has stopped.
- REQ-184: Direct access to service task state or direct function calls into the
  service task shall be unavailable unless an affected crate-local baseline
  documents an explicit exception.
- REQ-185: Functions invoked by runtime or event machinery outside ordinary
  message dispatch shall be called callbacks rather than message handlers.

### MPI-REQ-010: Task abstraction

A task shall be modeled as an operating-system thread with an associated bounded message queue.

Verification: test

Status: approved

### MPI-REQ-011: Task-owned queue

Every task shall own exactly one logical message queue.

Verification: inspection

Status: approved

### MPI-REQ-012: Static queue capacity

Each task shall have one static queue capacity configured at compile time.

Verification: test

Status: approved

### MPI-REQ-013: Shared queue capacity

Task queue capacity shall be shared between normal and priority messages.

Verification: test

Status: approved

### MPI-REQ-014: Queue-full error

Sending shall return an explicit queue-full error when the target queue is full and no runtime suspension path applies.

Verification: test

Status: approved

### MPI-REQ-015: Receiver-owned send reservations

When a task-internal send cannot enqueue because the receiving task queue has no available capacity, the receiver-side runtime shall be able to register the sending task for a future queue-capacity reservation.

Verification: test

Status: proposed

### MPI-REQ-016: Fair sender registration

A receiving task endpoint shall register each sending task at most once in its queue-capacity wait list and shall consider registered senders in FIFO order.

Verification: test

Status: proposed

### MPI-REQ-017: Reservation consumption

When a sender with a receiver-owned reservation next enqueues to that receiver, the receiver shall consume that sender's reservation before using unreserved queue capacity.

Verification: test

Status: proposed

### MPI-REQ-018: Framework queue-space wakeup

Queue-capacity wakeups used to resume senders waiting for reserved queue capacity shall be framework-only messages generated by `mpi`.

Verification: inspection

Status: proposed

### MPI-REQ-019: Failed wakeup releases reservation

If a receiving task endpoint cannot deliver a queue-capacity wakeup to a registered sender, the receiver shall release that sender's reservation and may continue with the next registered sender.

Verification: test

Status: proposed

### MPI-REQ-020: Message model

The runtime message model shall include events, calls, streams, responses, stream events, stream cancellation, start messages, and internal runtime/control messages when needed.

Verification: inspection

Status: approved

### MPI-REQ-021: Receiver-declared placement

Message placement shall be declared by the receiving task's message contract, not chosen by the sender at each send site.

Verification: inspection

Status: approved

### MPI-REQ-022: Placement classes

Queue placement classes shall include `Normal` and `Priority`.

Verification: inspection

Status: approved

### MPI-REQ-023: Queue ordering

Normal messages shall be FIFO relative to other normal messages, priority messages shall be FIFO relative to other priority messages, and receive shall return a priority message before any normal message whenever the priority queue is non-empty.

Verification: test

Status: approved

### MPI-REQ-024: Priority-reserved capacity

Each task queue shall reserve configurable capacity for priority messages so normal messages cannot consume all queue slots. The default priority-reserved capacity shall be one slot.

Verification: test

Status: proposed

### MPI-REQ-030: Start message creation

Task creation shall create and enqueue a start message for the new task.

Verification: test

Status: approved

### MPI-REQ-031: Start message forced priority

The start message shall be priority even if generated code or user declarations do not explicitly declare it as priority.

Verification: test

Status: approved

### MPI-REQ-032: Start message first

The first application message received by a newly spawned task shall be its start message.

Verification: test

Status: approved

### MPI-REQ-040: Suspended receive

When a handler awaits a specific reply or stream event, the handler continuation shall suspend instead of blocking the task OS thread.

Verification: test

Status: approved

### MPI-REQ-041: Continued task processing during wait

While one handler is suspended waiting for a reply or stream event, the task shall continue receiving and handling other messages.

Verification: test

Status: approved

### MPI-REQ-042: Waiter matching

The receive loop shall match suspended waiters by message kind and `SessionId` for protocol messages.

Verification: test

Status: approved

### MPI-REQ-043: Context-returning suspension

Task-local suspended handlers shall use a context-returning suspension primitive that does not retain mutable borrows of task state or task context while suspended.

Verification: test

Status: approved

### MPI-REQ-050: Compile-time receive checks

The runtime shall provide or support a mechanism equivalent to `CanReceive<T>` so task-internal calls and streams compile only when the caller can receive the corresponding response or stream event.

Verification: inspection and compile-fail test

Status: approved

### MPI-REQ-060: Universal SessionId

`mpi` shall use a universal `SessionId` for logical interactions that expect future messages.

Verification: inspection

Status: approved

### MPI-REQ-061: SessionId scope

`SessionId` shall identify a logical exchange rather than a single physical queued message.

Verification: inspection

Status: approved

### MPI-REQ-062: Task-local sequence allocation

A task context shall be able to allocate task-local session sequence values without global synchronization.

Verification: test

Status: approved

### MPI-REQ-070: Typed call response

A synchronous call response shall use a typed response wrapper containing the session ID and reply value.

Verification: inspection

Status: approved

### MPI-REQ-071: Exactly one call response

A synchronous call shall complete with exactly one response, error, or defined failure outcome.

Verification: test

Status: approved

### MPI-REQ-072: Out-of-order response matching

If two suspended handlers in the same caller task send the same call type and responses arrive out of order, each handler shall resume only for its own `SessionId`.

Verification: test

Status: approved

### MPI-REQ-073: Late reply handling

Late one-shot responses and late stream replies shall be passed to the receiving task's late-reply callback by default and ignored only when the interaction declares `late_reply = "ignore"`.

Verification: test

Status: approved

### MPI-REQ-074: Call response backpressure

When a task-internal call response cannot be enqueued to the caller because the caller's queue has no available capacity, the callee-side handler continuation shall suspend through the queue-capacity reservation mechanism.

Verification: test

Status: proposed

### MPI-REQ-080: Stream events

A stream shall represent producer output using typed batch, end, and error stream events.

Verification: inspection

Status: approved

### MPI-REQ-081: Rust-like stream consumer API

The stream consumer API shall expose a Rust-like `next(ctx).await` operation that returns one item at a time.

Verification: demonstration

Status: approved

### MPI-REQ-082: Stream batching and buffering

Streams shall support batching, and the consumer shall drain a local buffer before awaiting another stream event.

Verification: test

Status: approved

### MPI-REQ-083: Stream termination

The stream consumer shall complete on end, complete with error on stream error, and attempt asynchronous cancellation when an unfinished stream object is dropped.

Verification: test

Status: approved

### MPI-REQ-084: Producer within task model

A streaming handler shall remain within the current task model and shall not create a new task merely to produce stream items.

Verification: inspection

Status: approved

### MPI-REQ-085: Credit-based flow control

The stream design shall prevent a producer from flooding the consumer queue, with credit-based flow control as the recommended baseline.

Verification: test

Status: approved

### MPI-REQ-086: Producer suspension under flow control

Producer-side stream yield operations shall suspend when insufficient stream credit is available and resume after credit, cancellation, or another defined terminal stream-control condition.

Verification: test

Status: approved

### MPI-REQ-087: Stream reply backpressure

When a stream handler cannot enqueue a stream item, stream end, or stream error reply to the consumer because the consumer queue has no available capacity, the stream handler continuation shall suspend through the queue-capacity reservation mechanism.

Verification: test

Status: proposed

### MPI-REQ-090: External blocking API (obsolete)

External callers shall not send or receive application or protocol messages.
They may construct tasks, inject creation-time dependencies, and join runtimes.

Verification: demonstration

Status: obsolete

### MPI-REQ-091: No accidental internal blocking

Messaging APIs shall require task scope so handlers do not block the task thread
and external construction scope cannot send or receive messages.

Verification: inspection

Status: approved

### MPI-REQ-100: Diagnostics roadmap

The implementation shall include or preserve a roadmap for diagnostics, timeouts, tracing, and deadlock/debug support.

Verification: inspection

Status: approved

### MPI-REQ-110: Transaction identity

A transaction instance shall be identified by a transaction path containing a
root `TransactionId` and zero or more child path segments. `TransactionId`
shall identify a multi-message transaction scope and shall be distinct from
`SessionId`.

Verification: inspection

Status: proposed

### MPI-REQ-111: Transactional session matching

A transactional call or stream shall carry the transaction path for transaction
membership while continuing to use `SessionId` for individual reply matching.

Verification: test

Status: proposed

### MPI-REQ-112: Transaction outcome categories

Transactional runtime APIs shall distinguish business rejection from
infrastructure failure. Business rejection includes domain outcomes such as
insufficient funds or unavailable inventory. Infrastructure failure includes
task stop, cancellation, timeout, protocol violation, and unrecoverable send or
receive failure.

Verification: inspection and test

Status: proposed

### MPI-REQ-113: Transactional queue backpressure

Task-internal transactional sends that cannot enqueue because the target queue
has no available capacity shall suspend through the queue-capacity reservation
mechanism until capacity, cancellation, task stop, or the transaction deadline.

Verification: test

Status: proposed

### MPI-REQ-114: Transaction deadline

A transaction deadline shall apply to queue-capacity waits, transactional
operation replies, prepare votes, commit delivery, and abort delivery.

Verification: test

Status: proposed

### MPI-REQ-115: Timeout before decision

Before a durable commit decision exists, timeout or business rejection shall
drive the transaction toward abort.

Verification: test

Status: proposed

### MPI-REQ-116: Durable commit decision

After a durable commit decision exists, timeout while delivering commit
messages shall not roll the transaction back. Recovery shall continue
rollforward until every prepared participant has received the commit decision
or a defined unrecoverable hazard is reported.

Verification: analysis and test

Status: proposed

### MPI-REQ-117: Durable abort decision

After a durable abort decision exists, timeout while delivering abort messages
shall not change the abort decision. Recovery shall continue abort delivery
until every affected participant has received the abort decision or a defined
unrecoverable hazard is reported.

Verification: analysis and test

Status: proposed

### MPI-REQ-118: Durable participant prepare

A transaction participant shall durably prepare the transaction work it owns
before voting prepared.

Verification: analysis and test

Status: proposed

### MPI-REQ-119: Durable coordinator decision

A transaction coordinator shall durably record a commit decision before sending
commit messages to participants, and shall durably record an abort decision
before sending abort messages when an abort decision is required.

Verification: analysis and test

Status: proposed

### MPI-REQ-120: Transaction recovery

Recovery shall retry commit for any transaction with a durable commit decision
that has incomplete participant commit delivery, and shall retry abort for any
transaction with a durable abort decision that has incomplete participant abort
delivery.

Verification: analysis and test

Status: proposed

### MPI-REQ-121: Hierarchical transaction commit

A child transaction may commit only into its parent transaction state until the
root transaction reaches a durable commit decision.

Verification: test

Status: proposed

### MPI-REQ-122: Hierarchical transaction abort

Aborting a parent transaction shall abort every active or prepared descendant
transaction.

Verification: test

Status: proposed

### MPI-REQ-123: Hierarchical transaction prepare

Preparing a parent transaction shall require every active child transaction in
its subtree to be prepared or otherwise resolved according to a defined abort
path.

Verification: test

Status: proposed

### MPI-REQ-124: Side-effect enforcement boundary

The runtime shall enforce transaction side-effect restrictions only for
generated `mpi` send APIs and shall not claim to prevent arbitrary Rust side
effects outside generated `mpi` APIs.

Verification: inspection

Status: proposed

### MPI-REQ-125: Transaction log storage interface

Transactional messaging shall write transaction recovery records through a
persistent log storage protocol rather than through ad hoc runtime file writes.

Verification: inspection

Status: proposed

### MPI-REQ-126: Transaction decision commit

When transactional messaging writes a transaction commit or abort decision log
entry, it shall call the persistent log storage commit operation for that entry
index before treating the decision as durable.

Verification: test and inspection

Status: proposed

### MPI-REQ-127: Default persistent log storage crate

The default local file-backed persistent log storage service used by
transactional messaging shall be provided by the separate
`persistent-log-storage-service` workspace crate.

Verification: inspection

Status: proposed

### MPI-REQ-130: Service instance start

A service start function shall return a service instance that owns one running
service task and exposes the protocol binding or bindings used to communicate
with that task.

Verification: inspection

Status: proposed

### MPI-REQ-131: Service instance drop stop

When the final clone of a service instance is dropped, the service instance
shall close its service capability and join the service runtime without sending
an application or protocol message from external scope.

Verification: test and inspection

Status: proposed

### MPI-REQ-132: Service stop synchronization

Service shutdown synchronization shall use capability closure and runtime join
rather than an externally initiated stop call. Failure to join is fatal unless
the service implementation prevents that failure.

Verification: test and inspection

Status: proposed

### MPI-REQ-133: Service protocol binding stopped behavior

Protocol bindings cloned from a service instance may outlive the service
instance object, but they shall not keep the service task alive. After the final
service instance clone stops the task, sends and calls through remaining
protocol bindings shall return task-stopped errors.

Verification: compile-fail test and inspection

Status: proposed

### MPI-REQ-134: Service encapsulation

Direct access to service task state or direct function calls into the service
task shall be unavailable unless an affected crate-local baseline documents an
explicit exception.

Verification: inspection

Status: proposed

### MPI-REQ-135: Callback terminology

Functions invoked by runtime or event machinery outside ordinary message
dispatch shall be called callbacks rather than message handlers.

Verification: inspection

Status: proposed

### MPI-REQ-136: Endpoint termination state

Each task endpoint shall make one irreversible transition from running to a
terminal state that records whether the task completed, stopped, or panicked.

Verification: test

Status: proposed

### MPI-REQ-137: Task panic isolation

When Rust panic strategy is `unwind`, an unwinding panic from task startup,
message dispatch, or a message handler shall be caught at the task runtime
boundary and shall terminate that task without automatically unwinding another
task or the application entry thread.

Verification: test

Status: proposed

### MPI-REQ-138: Panic isolation limitation

The task runtime shall document that `panic = "abort"`, process abort, fatal
signals, undefined behavior, and other process-fatal failures cannot be
isolated by the task runtime.

Verification: inspection

Status: proposed

### MPI-REQ-139: Active session termination

When a task endpoint terminates, every active call or stream waiting on that
endpoint shall complete with a typed target-terminated outcome carrying the
recorded task termination reason.

Verification: test

Status: proposed

### MPI-REQ-140: Task supervision subscription

A task shall be able to subscribe from task scope to another endpoint's
termination. The subscription shall produce exactly one task-terminated event,
or complete immediately when the target is already terminated.

Verification: test

Status: proposed

### MPI-REQ-141: Race-free supervision registration

Registering supervision concurrently with target termination shall either
register before termination notification or observe the already recorded
termination; it shall not miss termination.

Verification: test

Status: proposed

### MPI-REQ-142: Supervision cancellation

Dropping a task supervision handle shall request cancellation, and termination
of the subscriber task shall automatically cancel all supervision
subscriptions owned by that subscriber.

Verification: test

Status: proposed

### MPI-REQ-143: Join panic reporting

Joining a task that terminated by panic shall return a typed join error rather
than resume the task panic in the joining thread. Applications and supervisors
decide whether that failure should terminate or restart other tasks.

Verification: test

Status: proposed

### MPI-REQ-144: Sanitized panic termination data

Panicked task termination may retain a diagnostic string extracted from a
string panic payload, but shall not expose or route the raw type-erased panic
payload as an application message.

Verification: test and inspection

Status: proposed

### MPI-REQ-145: Panic hook behavior

Task panic isolation shall not claim to suppress Rust's configured panic hook,
which runs before `catch_unwind` observes the panic. The runtime shall not
replace the process-global panic hook merely to hide isolated task panics.

Verification: inspection

Status: proposed

## Architecture

The original architecture IDs ARCH-001 through ARCH-004, ARCH-010 through
ARCH-082, and CMP-001 through CMP-011 and CMP-014 remain stable for `mpi`
runtime behavior. The local `MPI-ARCH-*` and `MPI-CMP-*` IDs below are grouping
aliases and do not replace those stable anchors.

Stable architecture ID anchors:

- ARCH-001, ARCH-002, ARCH-003, ARCH-004
- ARCH-010, ARCH-011, ARCH-012, ARCH-013, ARCH-014, ARCH-015
- ARCH-020, ARCH-021, ARCH-022, ARCH-023, ARCH-024, ARCH-025, ARCH-026, ARCH-027, ARCH-028, ARCH-029, ARCH-029A, ARCH-029B
- ARCH-030, ARCH-031, ARCH-032
- ARCH-040, ARCH-041, ARCH-042, ARCH-043, ARCH-044
- ARCH-050, ARCH-051, ARCH-052, ARCH-053, ARCH-054
- ARCH-060, ARCH-061, ARCH-062, ARCH-063, ARCH-064, ARCH-065, ARCH-066, ARCH-067, ARCH-068
- ARCH-070, ARCH-071, ARCH-072, ARCH-073, ARCH-074, ARCH-075, ARCH-076, ARCH-077, ARCH-077A, ARCH-078, ARCH-079, ARCH-079A
- ARCH-080, ARCH-081, ARCH-082
- CMP-001, CMP-002, CMP-002A, CMP-003, CMP-004, CMP-005, CMP-006, CMP-007, CMP-008, CMP-009, CMP-010, CMP-011, CMP-014

| ID | Component | Responsibility |
|---|---|---|
| MPI-CMP-001 | Task | Owns state, queue, context, and dispatch loop on one OS thread. |
| MPI-CMP-002 | TaskHandle | Public send surface used by other code to enqueue messages to a task. |
| MPI-CMP-003 | TaskEndpoint | Shared runtime endpoint behind task handles and in-flight sessions. |
| MPI-CMP-004 | TaskQueue | Bounded queue with normal and priority FIFO queues. |
| MPI-CMP-005 | TaskContext | Handler context containing self handle, session allocation, receive state, and control operations. |
| MPI-CMP-006 | TaskMessage | Trait implemented by generated message enums to expose receiver-declared placement. |
| MPI-CMP-007 | Dispatch loop | Receives messages, resumes matching waiters, or dispatches messages to handlers. |
| MPI-CMP-008 | Session subsystem | Allocates and matches `SessionId` values for calls and streams. |
| MPI-CMP-009 | Call subsystem | Sends typed call requests and typed `Response<T>` messages. |
| MPI-CMP-010 | Stream subsystem | Sends stream requests, stream events, cancellation, and flow-control messages. |
| MPI-CMP-011 | Compile-time receive check subsystem | Ensures a caller task can wait only for messages it declares it can receive. |
| MPI-CMP-012 | Diagnostics subsystem | Supports snapshots and future tracing, timeouts, and deadlock/debug support. |
| MPI-CMP-013 | Transaction core types | Allocates transaction identifiers and tracks transaction paths shared by generated transactional APIs and storage-backed transaction support. |
| MPI-CMP-014 | Transaction log adapter | Lives in the separate `mpi-transaction` crate and persists prepared participant state and coordinator commit or abort decisions through the persistent log storage protocol. |
| MPI-CMP-015 | Service instance | Owns one running service task, exposes its protocol bindings, and synchronizes stop when the final clone is dropped. |
| MPI-CMP-016 | Endpoint lifecycle | Records the endpoint's irreversible running-to-terminated transition and termination reason. |
| MPI-CMP-017 | Task monitor | Represents one cancellable task-scoped subscription to an endpoint's termination. |

Architecture rules:

- MPI-ARCH-010: Each task owns exactly one logical message queue.
- MPI-ARCH-011: Each task has a generated message enum representing received messages.
- MPI-ARCH-012: Task initialization is performed through the start message.
- MPI-ARCH-020: Normal messages are inserted at the tail of the normal queue.
- MPI-ARCH-021: Priority messages are inserted at the tail of the priority queue.
- MPI-ARCH-022: Receive first tries the head of the priority queue, then the head of the normal queue.
- MPI-ARCH-023: Total queue capacity is shared between normal and priority queues.
- MPI-ARCH-024: Message placement is determined by the receiving task's declaration.
- MPI-ARCH-025: Receiver-owned send reservations count against total queue capacity but do not affect message ordering until consumed.
- MPI-ARCH-030: Start messages are forced to priority and no other application message can be received before start.
- MPI-ARCH-040: Task-internal waits suspend handler continuations and do not block the task OS thread.
- MPI-ARCH-041: The receive loop checks suspended waiters before normal handler dispatch.
- MPI-ARCH-042: Suspended handler continuations shall not retain mutable borrows of task state or task context while suspended.
- MPI-ARCH-050: `SessionId` is shared by calls, streams, cancellation, matching, late-event handling, tracing, and debugging.
- MPI-ARCH-060: Protocol messages carry infrastructure-owned sender and
  `SessionId` headers. Protocol declarations define request/reply relationships;
  replies return to the request sender using the request session.
- MPI-ARCH-070: Streams use the same `SessionId` model as calls and expose batch, end, and error stream events.
- MPI-ARCH-080: External application messaging APIs are unavailable; task
  construction and runtime joining remain external operations.
- MPI-ARCH-081: Proxy forwarding wraps a complete type-erased message as the
  payload of a new outer message, so downstream replies return to the proxy.
- MPI-ARCH-090: `TransactionId` identifies the root transaction instance; `TransactionPath` identifies the root or a child transaction instance within a hierarchy.
- MPI-ARCH-091: `SessionId` remains the matching key for each individual transactional call or stream, while `TransactionPath` identifies the enclosing transaction instance.
- MPI-ARCH-092: A transaction participant stages or prepares transaction work before the coordinator records a durable commit decision; final persistent effects occur only during rollforward after that decision.
- MPI-ARCH-093: Transaction coordination follows two-phase commit at the root: participants prepare, the coordinator records a durable commit or abort decision, and recovery continues the recorded decision after restart.
- MPI-ARCH-094: Child transaction commit merges the child into its parent transaction state but does not make final effects globally committed before the root commit decision.
- MPI-ARCH-095: Parent abort propagates to active and prepared descendants.
- MPI-ARCH-096: Transaction deadlines bound waits for queue capacity, operation replies, prepare votes, commit delivery, and abort delivery.
- MPI-ARCH-097: Business rejection is a valid transaction outcome that drives abort before a durable commit decision; it is distinct from infrastructure failure.
- MPI-ARCH-098: Transaction recovery records are written through a persistent log storage protocol with message-based store, commit, discard, and read operations.
- MPI-ARCH-099: The default local implementation of that protocol is provided by the `persistent-log-storage-service` crate; storage-backed transaction logging is provided by the `mpi-transaction` crate to avoid dependency cycles.
- MPI-ARCH-100: A service instance owns service lifetime. Protocol bindings may
  be cloned independently, but they do not keep the service task alive after the
  final service instance clone is dropped.
- MPI-ARCH-101: Service shutdown closes the owned capability and joins the
  runtime without external application messaging.
- MPI-ARCH-102: Service task state is encapsulated behind message-based
  protocol bindings unless a crate-local baseline documents an explicit
  exception.
- MPI-ARCH-103: Task startup and dispatch execute inside one unwind-catching
  runtime boundary. Catching a panic records sanitized endpoint termination
  before the task thread returns; it does not suppress the configured panic
  hook.
- MPI-ARCH-104: Endpoint termination atomically closes message acceptance,
  completes active call and stream sessions, and notifies task monitors.
- MPI-ARCH-105: Supervision registration and endpoint termination synchronize
  through the same lifecycle state so registration cannot miss termination.
- MPI-ARCH-106: Supervision is an infrastructure lifecycle subscription, not an
  application protocol and not a general-purpose connection object.
- MPI-ARCH-107: A task endpoint owns cleanup registrations for subscriptions
  created by that task, allowing subscriber termination to cancel them.

## Transaction architecture

Transactional messages coordinate several typed message interactions under one
ACID transaction root. Each transactional call or stream carries a
`TransactionPath` and still carries its own `SessionId`; the transaction path
controls transaction membership and recovery, while the session ID controls
reply matching for the individual interaction.

Transactions use a two-phase commit model. During the open and prepare phases,
participants may validate requests and stage transaction work, but final
persistent effects are not committed. A business rejection, such as
insufficient funds, is a normal outcome that drives the transaction toward
abort before the coordinator records a durable commit decision.

Once every required participant has prepared, the coordinator may append a
commit decision to persistent log storage and commit through that log index.
That durable record is the rollforward boundary. After it exists, rollback is no
longer allowed; commit delivery may time out or be interrupted by restart, but
recovery continues rollforward from the durable commit decision. Abort decisions
follow the same durability rule: once an abort decision is durable, recovery
continues abort delivery rather than changing the outcome.

Hierarchical transactions represent nested transactional work explicitly. A
child transaction commits into its parent transaction state, not into globally
committed state. The root transaction's durable commit decision is the first
point where the whole prepared tree may roll forward into final persistent
effects. Aborting a parent aborts all active or prepared descendants.

Queue-full conditions in task-internal transactional sends use the existing
queue-capacity reservation model. A transactional send waits without blocking
the task OS thread until capacity becomes available, the transaction is
cancelled, the target stops, or the transaction deadline expires.

`mpi` can enforce side-effect restrictions only for generated `mpi` APIs.
Transaction handlers may still perform arbitrary Rust side effects if user code
calls external APIs directly; those effects are outside the runtime's
enforcement boundary and must be handled by application design and review.

## Interface

The original interface IDs INT-001 through INT-005, INT-020 through INT-027B,
INT-030 through INT-045, INT-050 through INT-081, INT-090 through INT-100, and
INT-093 through INT-099 remain stable for `mpi` runtime behavior and generated
runtime-facing APIs.

Stable interface ID anchors:

- INT-001, INT-002, INT-003, INT-004, INT-005
- INT-011, INT-011A, INT-016, INT-017, INT-019, INT-019A
- INT-020, INT-021, INT-022, INT-023, INT-024, INT-025, INT-026, INT-027, INT-027A, INT-027B
- INT-030, INT-031, INT-032, INT-033
- INT-040, INT-041, INT-042, INT-042A, INT-043, INT-044, INT-045
- INT-050, INT-051, INT-052
- INT-060, INT-061, INT-062, INT-063, INT-064, INT-065, INT-066, INT-067, INT-068
- INT-070, INT-071, INT-072, INT-073, INT-074, INT-075, INT-075A, INT-075B, INT-076, INT-077, INT-078, INT-080, INT-081
- INT-090, INT-091, INT-092, INT-093, INT-094, INT-095, INT-096, INT-097, INT-098, INT-099
- INT-100, INT-101

Core runtime interfaces are conceptually:

```rust
pub trait TaskMessage {
    fn placement(&self) -> MessagePlacement;
}

pub enum MessagePlacement {
    Normal,
    Priority,
}

pub struct SessionId {
    origin: EndpointId,
    sequence: u64,
}

pub struct TransactionId {
    origin: EndpointId,
    sequence: u64,
}

pub struct TransactionPath {
    root: TransactionId,
    path: Vec<u64>,
}

pub struct Transaction<TxKind> {
    path: TransactionPath,
    // runtime-owned coordinator or participant state
}

pub struct Response<T> {
    pub session_id: SessionId,
    pub value: T,
}

pub trait CanReceive<T>: TaskMessage {
    fn wrap(value: T) -> Self;
}

pub enum TaskTermination {
    Completed,
    Stopped,
    Panicked { message: Option<String> },
}

pub struct TaskMonitor {
    // target endpoint, subscriber endpoint, and supervision session
}
```

Interface rules:

- MPI-INT-001: Every generated task message enum shall implement the task message placement interface.
- MPI-INT-002: Placement shall be computed from the receiving task's message declaration.
- MPI-INT-003: Queue-full behavior shall be represented as an explicit error.
- MPI-INT-004: Generated messaging methods shall require a task context.
- MPI-INT-005: Generated handles and protocol bindings shall not expose external
  blocking send or receive methods.
- MPI-INT-006: `SessionId` shall be available to runtime protocol messages for calls, streams, cancellation, matching, tracing, and debugging.
- MPI-INT-007: Stream batching, stream end, stream error, stream cancellation, and stream flow-control details should be hidden from ordinary consumer code.
- MPI-INT-008: Public API errors should be typed and documented.
- MPI-INT-009: `TransactionId` and `TransactionPath` shall be available to runtime protocol messages that participate in transactions.
- MPI-INT-010: User-facing transactional APIs should pass a typed transaction handle rather than requiring ordinary users to manually construct or route transaction identifiers.
- MPI-INT-011: Transaction errors shall distinguish business rejection, timeout before decision, commit in progress, abort in progress, and unrecoverable transaction hazard.
- MPI-INT-012: Transaction recovery APIs shall expose durable transactions that require continued commit or abort delivery after restart.
- MPI-INT-013: Transaction log integration shall use the message-based persistent log storage protocol with store, commit, discard, and read calls.
- MPI-INT-014: Ordinary transaction users should not need to directly manipulate persistent log storage records for normal transaction execution.
- MPI-INT-015: Service start APIs shall return a service instance, not detached
  protocol bindings.
- MPI-INT-016: Service instances may be cloneable, but only the final clone
  drop shall stop the service task.
- MPI-INT-017: Protocol bindings exposed by a service instance may be cloned
  independently, but those bindings shall report stopped-task errors once the
  service instance has stopped the task.
- MPI-INT-018: Service shutdown shall be exposed as capability closure and
  runtime joining, not as an externally callable protocol message.
- MPI-INT-019: Starting task supervision shall require task scope and return a
  cancellable `TaskMonitor` associated with a supervision `SessionId`.
- MPI-INT-020: Waiting on a task monitor shall yield `TaskTermination`; dropping
  an unfinished monitor shall request cancellation without blocking.
- MPI-INT-021: Call and stream APIs shall expose target termination as a typed
  infrastructure error or terminal outcome, not as an application payload.
- MPI-INT-022: `TaskRuntime::join` shall report panicked termination as a typed
  error and shall not resume the caught panic payload.

Conceptual transaction API:

```rust
let mut tx = ctx.begin_transaction::<BankingV1::TransferTx>()
    .deadline(deadline)
    .await?;

accounts.reserve_debit(&mut tx, debit).await?;
accounts.reserve_credit(&mut tx, credit).await?;

let mut ledger_tx = tx.begin_ledger_tx(ctx).await?;
ledger.stage_entry(&mut ledger_tx, entry).await?;
ledger_tx.commit_into_parent(ctx).await?;

tx.commit(ctx).await?;
```

## Validation Scenarios

The original validation IDs VAL-001 through VAL-011 and VAL-013 remain stable
for `mpi` runtime behavior. The `MPI-VAL-*` IDs below are grouping aliases.

| ID | Scenario | Status |
|---|---|---|
| MPI-VAL-001 | Declare a simple task with queue size, start handler, and event handler through generated code. | approved |
| MPI-VAL-002 | Start task predictably; start is priority and received first. | approved |
| MPI-VAL-003 | Send an asynchronous event through a generated handle method and receive explicit queue-full errors. | approved |
| MPI-VAL-004 | Perform a typed synchronous call without blocking the task OS thread. | approved |
| MPI-VAL-005 | Handle concurrent same-type calls with out-of-order replies. | approved |
| MPI-VAL-006 | Reject undeclared receive at compile time. | approved |
| MPI-VAL-007 | Consume a stream with `next(ctx).await`. | approved |
| MPI-VAL-008 | Cancel an unfinished stream by dropping the stream object. | approved |
| MPI-VAL-009 | Avoid flooding a consumer queue with stream flow control. | approved |
| MPI-VAL-010 | Use priority shutdown while normal work is queued. | approved |
| MPI-VAL-011 | Main constructs tasks and joins runtimes without sending or receiving messages. | approved |
| MPI-VAL-012 | Diagnose sessions and queues. | deferred |
| MPI-VAL-013 | Coordinate a root transaction across several typed calls, abort on business rejection, and commit only after every participant prepares. | proposed |
| MPI-VAL-014 | Run a hierarchical transaction where a child transaction commits into its parent and becomes globally committed only after the root durable commit decision. | proposed |
| MPI-VAL-015 | Recover a transaction after restart by continuing rollforward from a durable commit decision or continuing abort delivery from a durable abort decision. | proposed |
| MPI-VAL-016 | Reject generated non-transactional side-effecting sends from inside a transactional handler. | proposed |
| MPI-VAL-017 | Recover transaction decisions from the default file-backed persistent log storage crate after restart. | proposed |
| MPI-VAL-018 | Start a service, use its protocol bindings through the service instance, and observe synchronized stop when the final service instance clone is dropped. | proposed |
| MPI-VAL-019 | Supervise a task, observe exactly one termination event, and cancel supervision by dropping the monitor or terminating the subscriber. | proposed |
| MPI-VAL-020 | Panic in one task, observe target-terminated call, stream, monitor, and join outcomes, while unrelated tasks continue running. | proposed |

## Verification

Verification should include:

- queue capacity, queue-full, normal FIFO, priority FIFO, and priority-before-normal tests;
- start enqueue, forced priority, and received-first tests;
- task endpoint lifecycle tests;
- suspended call and stream tests proving continued task progress;
- compile-fail tests for missing receive declarations;
- session allocation and out-of-order response matching tests;
- late reply and late stream reply tests;
- stream batching, end, error, cancellation, and credit tests;
- compile-fail tests showing external code cannot send or receive messages;
- tests for sender/session headers and nested proxy request/reply forwarding;
- diagnostics roadmap inspection.
- transaction coordination tests covering business rejection, queue-capacity
  suspension with deadlines, prepare, durable commit decision, durable abort
  decision, hierarchical commit and abort, and recovery after restart;
- compile-fail or inspection evidence showing generated APIs reject
  non-transactional side-effecting sends from transactional scopes;
- transaction recovery tests showing commit and abort decision records are
  committed through persistent log storage before the runtime treats the
  decision as durable.
- service lifecycle tests showing synchronized stop on final service instance
  drop and stopped-task errors from protocol bindings retained after stop.
- supervision tests covering already-terminated targets, cancellation,
  subscriber termination, and registration/termination races;
- unwind-panic tests showing endpoint closure, typed session termination,
  monitor notification, typed join failure, and continued unrelated task
  execution;
- inspection that panic isolation is not claimed for `panic = "abort"` or other
  process-fatal failures.

## Traceability

| Requirement area | Architecture | Interfaces | Validation |
|---|---|---|---|
| MPI-REQ-010..MPI-REQ-019 | MPI-ARCH-010, MPI-ARCH-020..MPI-ARCH-025 | MPI-INT-001..MPI-INT-004 | MPI-VAL-002, MPI-VAL-003, MPI-VAL-010 |
| MPI-REQ-020..MPI-REQ-024 | MPI-ARCH-020..MPI-ARCH-025 | MPI-INT-001..MPI-INT-003 | MPI-VAL-003, MPI-VAL-010 |
| MPI-REQ-030..MPI-REQ-032 | MPI-ARCH-030 | MPI-INT-001, MPI-INT-002 | MPI-VAL-002 |
| MPI-REQ-040..MPI-REQ-043 | MPI-ARCH-040..MPI-ARCH-042 | MPI-INT-004 | MPI-VAL-004, MPI-VAL-005 |
| MPI-REQ-050 | MPI-CMP-011 | MPI-INT-004 | MPI-VAL-006 |
| MPI-REQ-060..MPI-REQ-074 | MPI-ARCH-050, MPI-ARCH-060 | MPI-INT-006 | MPI-VAL-004, MPI-VAL-005 |
| MPI-REQ-080..MPI-REQ-087 | MPI-ARCH-070 | MPI-INT-007 | MPI-VAL-007, MPI-VAL-008, MPI-VAL-009 |
| MPI-REQ-090..MPI-REQ-091 | MPI-ARCH-080 | MPI-INT-004, MPI-INT-005 | MPI-VAL-011 |
| MPI-REQ-100 | MPI-CMP-012 | MPI-INT-006, MPI-INT-008 | MPI-VAL-012 |
| MPI-REQ-110..MPI-REQ-127 | MPI-CMP-013, MPI-CMP-014, MPI-ARCH-090..MPI-ARCH-099 | MPI-INT-009..MPI-INT-014 | MPI-VAL-013..MPI-VAL-017 |
| MPI-REQ-130..MPI-REQ-135 | MPI-CMP-015, MPI-ARCH-100..MPI-ARCH-102 | MPI-INT-015..MPI-INT-018 | MPI-VAL-018 |
| MPI-REQ-136..MPI-REQ-145 | MPI-CMP-016, MPI-CMP-017, MPI-ARCH-103..MPI-ARCH-107 | MPI-INT-019..MPI-INT-022 | MPI-VAL-019, MPI-VAL-020 |
