# mpi-macros Design Baseline

This document defines the lightweight systems-engineering baseline for the `mpi-macros` crate.

`mpi-macros` owns proc-macro parsing and generated Rust code for task declarations, protocol declarations, generated handles, contexts, message enums, dispatch, protocol bindings, and compile-time receive declarations. Runtime semantics live in `crates/mpi-core/se-design-baseline.md`; user-facing macro exports are available through the `mpi` facade.

## Purpose

The crate lets users declare `mpi` tasks and protocols with idiomatic Rust syntax while generating the boilerplate needed by the runtime.

## Stakeholder Needs

The following original stakeholder need IDs remain part of this crate baseline:

- SN-011: Rust developers need tasks to declare at compile time which messages they can receive.
- SN-012: Rust developers need generated task handles with send methods instead of manually constructing and enqueueing low-level message enums.
- SN-013: Rust developers need asynchronous events, typed synchronous calls, and generator-style streams to be expressed with clear Rust APIs.
- SN-018: Rust developers need reusable protocol declarations so independently built applications can share typed message contracts across separate binaries.
- SN-021: Contributors need clear architecture and interface boundaries between runtime primitives, task macros, queues, receive logic, sessions, streams, and signal support.
- SN-023: Contributors need compile-time receive checks to prevent handlers from waiting for undeclared response or stream event messages.
- SN-024: Contributors need protocol compatibility rules that distinguish compatible additions from incompatible message contract changes.
- SN-046: Rust developers need ACID-style transaction scopes that can coordinate several typed message interactions while keeping invalid transaction participation visible at compile time.
- SN-047: Rust developers need hierarchical transactions so nested transactional work is represented explicitly rather than relying on ad hoc handler-local rollback code.

## Scope

`mpi-macros` is responsible for:

- `#[task]` task declaration macro behavior;
- task message enum generation;
- task context and handle generation;
- handler dispatch generation;
- start, event, call, and stream handler plumbing;
- late-reply callback plumbing;
- generated service start, stop, and service-instance API support;
- receive declaration generation;
- protocol declaration and protocol-instance binding generation;
- generated compile-time receive checks;
- transaction declaration parsing and generation;
- generated transaction-kind marker types, transaction hierarchy checks, and side-effect send restrictions for generated `mpi` APIs.

`mpi-macros` is not responsible for:

- queue implementations;
- session allocation algorithms;
- stream buffering or flow control algorithms;
- transaction coordination, durable logging, or recovery algorithms;
- service-specific task behavior or service state;
- OS or framework event bridges.

## Requirements

The original workspace-level `REQ-*` IDs remain stable after migration. The
`MACRO-REQ-*` IDs below are local grouping aliases; they do not replace the
original IDs used by tests, reports, and traceability.

### Migrated stable requirements

- REQ-026: Each task shall have one generated message enum representing the messages it can receive.
- REQ-027: Sending a user-visible message shall be done by calling a generated method on the receiving task handle.
- REQ-028: A generated send method shall construct the message and enqueue it directly into the receiving task queue.
- REQ-050: The macro used for task declarations shall be named `task`.
- REQ-051: The `#[task]` macro shall generate or support the task message enum, task context type, task handle, handle send methods, queue placement implementation, dispatch plumbing, and call or stream plumbing.
- REQ-052: Generated dispatch logic shall route each message enum variant to the corresponding handler.
- REQ-053: The macro shall either reject explicit normal placement for a start handler or ignore it and force priority.
- REQ-160: A protocol shall define a named, exported set of message declarations.
- REQ-161: A protocol shall belong to a namespace represented by the Rust module or crate path that exports it.
- REQ-162: A protocol message identity shall include the protocol namespace, protocol name, and message name.
- REQ-162A: Call reply and stream reply message identities shall be derived from the declared protocol interaction name using a fixed naming convention.
- REQ-163: Each protocol message declaration shall explicitly identify all Rust types carried by the protocol message interaction.
- REQ-164: A published protocol shall be compatible only with additions of new message declarations.
- REQ-165: Changing or removing an existing protocol message declaration or associated type shall require a new protocol name.
- REQ-166: Compile-time receive checks shall be based on protocol-declared response and stream event types.
- REQ-167: The baseline shall not require a generated protocol fingerprint for compile-time receive checks.
- REQ-168: A receive declaration shall match a protocol reply or stream event by protocol message identity and by the Rust type declared for that reply or stream event.
- REQ-169: Generated send, call, and stream APIs shall be derived from a protocol declaration or from a derivative that preserves protocol message identities and declared types.
- REQ-170: A protocol derivative may bind a protocol declaration to the concrete task, endpoint, or handle that implements a specific protocol instance.
- REQ-180: A service start function shall return a service instance that owns
  one running service task and exposes that task's protocol bindings.
- REQ-181: Dropping the final clone of a service instance shall close the
  service capability and join its runtime without external application
  messaging.
- REQ-182: Service shutdown synchronization shall use capability closure and
  runtime join rather than an externally initiated stop call.
- REQ-183: Protocol bindings cloned from a service instance may outlive the
  service instance object, but sends and calls through those bindings shall fail
  with task-stopped behavior after the service task has stopped.
- REQ-184: Direct access to service task state or direct function calls into the
  service task shall be unavailable unless an affected crate-local baseline
  documents an explicit exception.
- REQ-186: A service task may omit an explicit start handler; when omitted, the
  macro shall generate an empty no-argument start handler.
- REQ-187: A service task may omit an explicit stop handler; when omitted, the
  macro shall generate an empty no-argument stop handler.
- REQ-188: If a user-declared message handler or task symbol collides with a
  generated task API symbol, the macro shall produce a compile error rather
  than silently suppressing the generated symbol.

### MACRO-REQ-001: Task macro name

The macro used for task declarations shall be named `task`.

Verification: inspection

Status: approved

### MACRO-REQ-002: Generated task artifacts

The `#[task]` macro shall generate or support the task message enum, task context type, task handle, handle send methods, queue placement implementation, dispatch plumbing, spawn helper, and call or stream plumbing.

Verification: inspection

Status: approved

### MACRO-REQ-003: Handler dispatch

Generated dispatch logic shall route each message enum variant to the corresponding handler.

Verification: test

Status: approved

### MACRO-REQ-004: Start handler priority

The macro shall reject explicit normal placement for a start handler or ignore it and force priority.

Verification: test

Status: approved

### MACRO-REQ-005: Ordinary handler syntax

Handler declarations shall use ordinary Rust `fn` syntax without a `self` receiver. The macro owns any async or context-future lowering needed to execute handlers.

Verification: compile-fail test and inspection

Status: approved

### MACRO-REQ-006: Scoped state access

Generated task contexts shall expose scoped state access so user-state mutation cannot directly perform context-dependent task operations while the state borrow is live.

Verification: compile-fail test and inspection

Status: approved

### MACRO-REQ-007: Receive declarations

The macro shall generate compile-time receive support for declared response and stream event messages.

Verification: compile-fail test

Status: approved

### MACRO-REQ-010: Protocol declaration

A protocol macro shall define a named, exported set of message declarations.

Verification: inspection

Status: proposed

### MACRO-REQ-011: Protocol namespace

A protocol shall belong to a namespace represented by the Rust module or crate path that exports it.

Verification: inspection

Status: proposed

### MACRO-REQ-012: Protocol message identity

A protocol message identity shall include the protocol namespace, protocol name, and message name.

Verification: inspection

Status: proposed

### MACRO-REQ-013: Derived response identities

Call reply and stream reply message identities shall be derived from the declared protocol interaction name using fixed idiomatic Rust naming: `get::Reply` for a call reply and `list_directories::Item`, `list_directories::Finish`, and `list_directories::Error` for stream replies.

Verification: inspection

Status: proposed

### MACRO-REQ-014: Explicit protocol message types

Each protocol declaration shall explicitly identify all Rust types carried by the interaction, including event payload, call request, call reply, stream request, stream item, and stream error types as applicable.

Verification: inspection

Status: proposed

### MACRO-REQ-015: Append-only protocol compatibility

A published protocol shall be compatible only with additions of new message declarations. Changing or removing an existing declaration or associated type shall require a new protocol name.

Verification: inspection

Status: proposed

### MACRO-REQ-016: Protocol-based receive checks

Compile-time receive checks shall be based on protocol-declared response and stream event types and identities.

Verification: compile-fail test

Status: proposed

### MACRO-REQ-017: Protocol-derived send surface

Generated send, call, and stream APIs shall be derived from a protocol declaration or from a derivative that preserves protocol message identities and declared types.

Verification: inspection

Status: proposed

### MACRO-REQ-020: Transaction declaration

A protocol macro shall support declaring named transaction kinds that list the
protocol messages allowed to participate in each transaction kind.

Verification: inspection

Status: proposed

### MACRO-REQ-021: Transaction child declaration

A protocol transaction kind shall be able to list the child transaction kinds
that are allowed directly beneath it.

Verification: inspection

Status: proposed

### MACRO-REQ-022: Transaction message membership check

A generated transactional send method shall compile only when the active
transaction kind allows the target protocol message.

Verification: compile-fail test

Status: proposed

### MACRO-REQ-023: Transaction hierarchy check

A generated child transaction creation method shall compile only when the active
parent transaction kind allows the child transaction kind.

Verification: compile-fail test

Status: proposed

### MACRO-REQ-024: Side-effect message declaration

A protocol message declaration may mark the message as side-effecting for
generated send-effect checks.

Verification: inspection

Status: proposed

### MACRO-REQ-025: Transactional side-effect restriction

Generated transactional handler APIs shall not expose generated sends for
non-transactional side-effecting messages.

Verification: compile-fail test

Status: proposed

### MACRO-REQ-030: Service instance generation

The macro shall support generation of service start APIs that return a service
instance owning one running service task and exposing that task's protocol
bindings.

Verification: inspection and test

Status: implemented

### MACRO-REQ-031: Service binding stopped behavior

Generated service protocol bindings may be cloned independently from the
service instance, but those bindings shall not keep the service task alive and
shall report stopped-task errors after the final service instance clone stops
the task.

Verification: compile-fail test and inspection

Status: implemented

### MACRO-REQ-032: Service final-drop stop

Generated service instance drop behavior shall close the service capability and
join the service runtime without sending an application or protocol message
from external scope.

Verification: test and inspection

Status: implemented

### MACRO-REQ-033: Omitted service start handler

A generated service task may omit an explicit start handler. When omitted, the
macro shall synthesize an empty no-argument start handler.

Verification: test and inspection

Status: proposed

### MACRO-REQ-034: Omitted service stop handler

A generated service task may omit an explicit stop handler. When omitted, the
macro shall synthesize an empty no-argument stop handler.

Verification: test and inspection

Status: proposed

### MACRO-REQ-035: Generated name collision diagnostics

If a user-declared message handler or task symbol collides with a generated
task API symbol, the macro shall produce a compile error that identifies the
collision.

Verification: compile-fail test

Status: proposed

### MACRO-REQ-036: Generated task supervision API

Generated task handles shall expose a task-scoped supervision method that
creates a cancellable task monitor for the handle's endpoint. The generated API
shall not expose external supervision subscription methods. Generated task
message enums shall carry infrastructure task-termination messages, and a task
may declare one `#[task_terminated]` handler for explicitly supervised events.

Verification: test and compile-fail test

Status: proposed

### MACRO-REQ-037: Generated timed suspension API

Generated task contexts shall expose `sleep_until(std::time::Instant)`, and
generated dispatch shall allow ordinary messages to start nested handler
continuations while another handler is suspended.

Verification: test and inspection

Status: approved

### MACRO-REQ-038: Runtime-owned generated stream waiters

Generated blocking and suspended stream APIs shall construct their delivery
waiters through `mpi-core` and shall not generate an independent
message-passing channel.

Verification: test and inspection

Status: approved

## Architecture

Architecture rules:

The original architecture IDs CMP-012, CMP-016, ARCH-100 through ARCH-109,
and ARCH-101A remain stable for `mpi-macros` protocol and generation behavior.
The macro also participates in generated task architecture IDs ARCH-011 through
ARCH-015 where code generation creates the corresponding runtime-facing types.

Stable architecture ID anchors:

- CMP-012, CMP-016
- ARCH-011, ARCH-012, ARCH-013, ARCH-014, ARCH-015
- ARCH-100, ARCH-101, ARCH-101A, ARCH-102, ARCH-103, ARCH-104, ARCH-105, ARCH-106, ARCH-107, ARCH-108, ARCH-109

- MACRO-ARCH-001: `#[task]` applies to an `impl` block for a task state type.
- MACRO-ARCH-002: The task impl attribute carries static queue-size configuration and may carry priority-reserved capacity configuration.
- MACRO-ARCH-003: Generated handles expose messaging methods only through
  task-internal context-aware APIs; external blocking messaging methods are not
  generated.
- MACRO-ARCH-004: Generated contexts provide scoped user-state access and runtime operations.
- MACRO-ARCH-005: Generated message enums implement the `mpi` runtime placement interface.
- MACRO-ARCH-006: Protocol declaration output is the source of truth for protocol-qualified message identities and associated Rust types.
- MACRO-ARCH-007: Protocol-instance bindings identify the concrete task, endpoint, or handle implementing a protocol message without redefining the message contract.
- MACRO-ARCH-008: Protocol transaction declarations are the source of truth for transaction-kind identities, allowed transactional message membership, and allowed child transaction hierarchy.
- MACRO-ARCH-009: Generated transaction-kind marker types and trait implementations encode whether a transaction kind allows a protocol message or child transaction kind.
- MACRO-ARCH-010: Generated send-effect checks are limited to generated `mpi` APIs and do not attempt to prove arbitrary Rust side effects inside handler bodies.
- MACRO-ARCH-011: Generated service instances own service task lifetime and
  expose protocol bindings that may be cloned independently without keeping the
  service task alive.
- A task opts into service generation by specifying both
  `service_instance = TypeName` and `service_start = function_name`. The task
  state must implement `Default`; generated start-handler arguments become the
  generated start function's arguments.
- MACRO-ARCH-012: Generated service final-drop logic closes the service
  capability and joins its runtime without external application messaging.
- MACRO-ARCH-013: Generated service start and stop handlers may be synthesized
  as empty no-argument handlers when the service task declaration omits them.
- MACRO-ARCH-014: Generated task API names are reserved within the generated
  handle surface. User-declared handlers or task symbols that would collide
  with those names fail during macro expansion.
- MACRO-ARCH-015: Generated supervision methods delegate to endpoint lifecycle
  monitoring and require generated task context. Termination delivery uses a
  hidden generated message variant in the subscriber task queue.
- MACRO-ARCH-016: Generated handler dispatch uses the runtime's dispatching
  future adapter so a pending timed handler does not defer unrelated ordinary
  messages until its deadline.
- MACRO-ARCH-017: Generated stream bindings delegate waiter construction and
  wake behavior to `mpi-core`, keeping one runtime synchronization model for
  calls and streams.

## Interface

The original interface IDs INT-006 through INT-019A, INT-018A, INT-018B,
INT-020 through INT-027B, INT-040 through INT-042A, and INT-075A through
INT-075B remain stable for `mpi-macros` generated API behavior.

Stable interface ID anchors:

- INT-006, INT-007, INT-008, INT-009, INT-009A, INT-009B, INT-009C, INT-009D, INT-009E
- INT-010, INT-011, INT-011A, INT-012, INT-013, INT-014, INT-015, INT-016, INT-017, INT-018, INT-018A, INT-018B, INT-019, INT-019A
- INT-020, INT-021, INT-022, INT-023, INT-024, INT-025, INT-026, INT-027, INT-027A, INT-027B
- INT-040, INT-041, INT-042, INT-042A
- INT-075A, INT-075B

Conceptual task declaration:

```rust
#[task(queue_size = 32, priority_reserved = 1)]
impl ServerTask {
    #[start]
    fn start(ctx: &mut ServerTaskContext, config: ServerConfig) {
        ctx.with_state(|state| {
            state.state = ServerState::new(config);
        });
    }

    #[event]
    fn set(ctx: &mut ServerTaskContext, key: String, value: Vec<u8>) {
        ctx.with_state(|state| {
            state.state.insert(key, value);
        });
    }

    #[call]
    fn get(ctx: &mut ServerTaskContext, key: String) -> GetReply {
        ctx.with_state(|state| GetReply {
            value: state.state.get(&key).cloned(),
        })
    }
}
```

Conceptual protocol declaration:

```rust
protocol! {
    pub protocol InventoryV1 {
        event reindex(ReindexRequest);
        call get_item(GetItemRequest) -> GetItemReply;
        stream watch_stock(WatchStockRequest) -> StockEvent, WatchStockError;
    }
}
```

Conceptual transaction declaration:

```rust
protocol! {
    pub protocol BankingV1 {
        call reserve_debit(ReserveDebitRequest) -> ReserveDebitReply;
        call reserve_credit(ReserveCreditRequest) -> ReserveCreditReply;
        call stage_entry(StageEntryRequest) -> StageEntryReply;

        #[side_effect]
        event write_audit(AuditEntry);

        transaction transfer_tx {
            message reserve_debit;
            message reserve_credit;
            child ledger_tx;
        }

        transaction ledger_tx {
            message stage_entry;
        }
    }
}
```

In this shape, `transfer_tx` and `ledger_tx` are transaction kinds. Runtime
transaction instances carry transaction paths, while generated transaction
handle types carry the transaction kind needed for compile-time membership and
hierarchy checks.

Interface rules:

- MACRO-INT-001: `#[start]` identifies the start handler.
- MACRO-INT-002: `#[event]` identifies an asynchronous message with no reply.
- MACRO-INT-003: `#[call]` identifies a synchronous request handler.
- MACRO-INT-004: `#[stream(item = T, error = E)]` identifies a streaming handler with item type `T` and error type `E`.
- MACRO-INT-005: `priority` is declared on the receiver's message declaration, not at each send site.
- MACRO-INT-006: `#[late_reply]` identifies an optional task callback for reported late replies.
- MACRO-INT-007: A `#[task]` attribute on a struct is non-authoritative and should not be required.
- MACRO-INT-008: Protocol interaction names should be declared in `snake_case`.
- MACRO-INT-009: Generated Rust modules for protocol interactions shall use `snake_case`, while generated receive identity types inside those modules shall use `PascalCase`.
- MACRO-INT-010: Protocol stream item and error return types are written as a comma-separated return type list.
- MACRO-INT-011: Transaction kind names should be declared in `snake_case`; generated Rust marker types shall use idiomatic Rust casing for type names.
- MACRO-INT-012: A protocol transaction declaration shall list allowed messages by protocol interaction name.
- MACRO-INT-013: A protocol transaction declaration shall list allowed direct child transactions by transaction kind name.
- MACRO-INT-014: Generated transactional APIs should accept a typed transaction handle rather than requiring ordinary users to manually pass transaction identifiers.
- MACRO-INT-015: Generated child transaction APIs should be named from the child transaction kind and should only exist or type-check for allowed parent-child pairs.
- MACRO-INT-016: Generated non-transactional side-effecting send APIs should be unavailable from transactional handler contexts.
- MACRO-INT-017: A service declaration shall identify the generated service
  instance type and the protocol binding or bindings exposed by that instance.
- MACRO-INT-018: Generated service instances may be cloneable, but only final
  clone drop shall close the service capability and join the runtime.
- MACRO-INT-019: `#[stop]` identifies a task-internal stop handler; it is not an
  externally callable message API.
- MACRO-INT-020: Omitted service start and stop handlers shall mean empty
  no-argument handlers, not missing message variants.
- MACRO-INT-020A: A service-enabled task declaration shall generate the named
  public service-instance type and start function. The instance shall expose a
  cloneable `binding()` while retaining sole ownership of runtime shutdown.
- MACRO-INT-021: A non-message task method named `stop` shall be rejected when
  the macro would otherwise synthesize the generated stop API.
- MACRO-INT-022: A message handler name shall be rejected when it collides with
  a built-in generated handle method or another generated messaging method.
- MACRO-INT-023: Each generated task handle shall expose `supervise(ctx)` or an
  equivalently explicit task-scoped method returning a `TaskMonitor`.
- MACRO-INT-024: `#[task_terminated]` shall declare at most one handler accepting
  the supervised `TaskTerminated` event; infrastructure termination for calls
  and streams shall not be dispatched to that handler.
- MACRO-INT-026: Generated task contexts shall expose
  `sleep_until(std::time::Instant) -> SleepUntil`.
- MACRO-INT-027: Generated blocking stream methods shall use the runtime's
  blocking stream channel constructor.

## Validation Scenarios

The original validation IDs VAL-001 through VAL-007, VAL-011, and VAL-015
remain stable for generated task and protocol usability. The `MACRO-VAL-*`
IDs below are grouping aliases.

| ID | Scenario | Status |
|---|---|---|
| MACRO-VAL-001 | A user declares a task and sends an event through a generated handle method. | approved |
| MACRO-VAL-002 | A user performs a generated typed synchronous call. | approved |
| MACRO-VAL-003 | Missing receive declarations fail at compile time. | approved |
| MACRO-VAL-004 | A protocol declaration generates namespace-qualified identities and typed send surfaces. | proposed |
| MACRO-VAL-005 | Unsupported handler shapes fail clearly instead of silently generating blocking behavior. | approved |
| MACRO-VAL-006 | A transaction declaration generates typed transaction handles that allow only declared messages. | proposed |
| MACRO-VAL-007 | A transaction declaration generates child transaction APIs only for declared parent-child relationships. | proposed |
| MACRO-VAL-008 | Transactional handler contexts cannot send generated non-transactional side-effecting messages. | proposed |
| MACRO-VAL-009 | A service declaration generates a service instance whose protocol bindings cannot outlive the instance and whose final drop stops the task. | proposed |
| MACRO-VAL-010 | A task declaration with user symbols that collide with generated handle methods fails with an explicit compile error. | proposed |
| MACRO-VAL-011 | A task supervises another generated task, observes panic termination, and external code cannot create the subscription. | proposed |
| MACRO-VAL-012 | Several generated handlers sleep until different deadlines and complete in deadline order. | approved |

## Verification

Verification should include:

- macro expansion inspection for generated message enums, contexts, handles, placement arms, spawn helpers, dispatch arms, calls, streams, and protocol conversion plumbing;
- runtime integration tests through `mpi` task-macro tests;
- compile-fail tests for missing receive declarations, wrong protocol receive identities, unsupported handler receiver forms, and invalid scoped-state usage;
- compile-fail tests for generated handle method name collisions;
- compile-fail tests for transactional sends whose message is not declared in the active transaction kind;
- compile-fail tests for child transaction creation whose child kind is not declared under the active parent kind;
- compile-fail tests for generated non-transactional side-effecting sends from transactional handler contexts;
- tests or macro expansion inspection for generated service instance lifetime,
  cloned binding stopped behavior, synthesized empty start and stop handlers,
  and final-drop stop behavior;
- examples demonstrating generated task APIs.
- tests and compile-fail tests for task-scoped supervision generation.

## Traceability

| Requirement area | Architecture | Interfaces | Validation |
|---|---|---|---|
| MACRO-REQ-001..MACRO-REQ-007 | MACRO-ARCH-001..MACRO-ARCH-005 | MACRO-INT-001..MACRO-INT-007 | MACRO-VAL-001, MACRO-VAL-002, MACRO-VAL-003, MACRO-VAL-005 |
| MACRO-REQ-010..MACRO-REQ-017 | MACRO-ARCH-006, MACRO-ARCH-007 | MACRO-INT-008..MACRO-INT-010 | MACRO-VAL-004 |
| MACRO-REQ-020..MACRO-REQ-025 | MACRO-ARCH-008..MACRO-ARCH-010 | MACRO-INT-011..MACRO-INT-016 | MACRO-VAL-006..MACRO-VAL-008 |
| MACRO-REQ-030..MACRO-REQ-034 | MACRO-ARCH-011..MACRO-ARCH-013 | MACRO-INT-017..MACRO-INT-020 | MACRO-VAL-009 |
| MACRO-REQ-035 | MACRO-ARCH-014 | MACRO-INT-021..MACRO-INT-022 | MACRO-VAL-010 |
| MACRO-REQ-036 | MACRO-ARCH-015 | MACRO-INT-023, MACRO-INT-024 | MACRO-VAL-011 |
| MACRO-REQ-037 | MACRO-ARCH-016 | MACRO-INT-026 | MACRO-VAL-012 |
| MACRO-REQ-038 | MACRO-ARCH-017 | MACRO-INT-027 | MACRO-VAL-002 |
