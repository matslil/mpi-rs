# mpi-macros Design Baseline

This document defines the lightweight systems-engineering baseline for the `mpi-macros` crate.

`mpi-macros` owns proc-macro parsing and generated Rust code for task declarations, protocol declarations, generated handles, contexts, message enums, dispatch, protocol bindings, and compile-time receive declarations. Runtime semantics live in `crates/mpi/se-design-baseline.md`.

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

## Scope

`mpi-macros` is responsible for:

- `#[task]` task declaration macro behavior;
- task message enum generation;
- task context and handle generation;
- handler dispatch generation;
- start, event, call, stream, and late-reply handler plumbing;
- receive declaration generation;
- protocol declaration and protocol-instance binding generation;
- generated compile-time receive checks.

`mpi-macros` is not responsible for:

- queue implementations;
- session allocation algorithms;
- stream buffering or flow control algorithms;
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
- MACRO-ARCH-003: Generated handles expose task-internal context-aware methods and explicit external blocking methods where applicable.
- MACRO-ARCH-004: Generated contexts provide scoped user-state access and runtime operations.
- MACRO-ARCH-005: Generated message enums implement the `mpi` runtime placement interface.
- MACRO-ARCH-006: Protocol declaration output is the source of truth for protocol-qualified message identities and associated Rust types.
- MACRO-ARCH-007: Protocol-instance bindings identify the concrete task, endpoint, or handle implementing a protocol message without redefining the message contract.

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

Interface rules:

- MACRO-INT-001: `#[start]` identifies the start handler.
- MACRO-INT-002: `#[event]` identifies an asynchronous message with no reply.
- MACRO-INT-003: `#[call]` identifies a synchronous request handler.
- MACRO-INT-004: `#[stream(item = T, error = E)]` identifies a streaming handler with item type `T` and error type `E`.
- MACRO-INT-005: `priority` is declared on the receiver's message declaration, not at each send site.
- MACRO-INT-006: `#[late_reply]` identifies an optional task handler for reported late replies.
- MACRO-INT-007: A `#[task]` attribute on a struct is non-authoritative and should not be required.
- MACRO-INT-008: Protocol interaction names should be declared in `snake_case`.
- MACRO-INT-009: Generated Rust modules for protocol interactions shall use `snake_case`, while generated receive identity types inside those modules shall use `PascalCase`.
- MACRO-INT-010: Protocol stream item and error return types are written as a comma-separated return type list.

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

## Verification

Verification should include:

- macro expansion inspection for generated message enums, contexts, handles, placement arms, spawn helpers, dispatch arms, calls, streams, and protocol conversion plumbing;
- runtime integration tests through `mpi` task-macro tests;
- compile-fail tests for missing receive declarations, wrong protocol receive identities, unsupported handler receiver forms, and invalid scoped-state usage;
- examples demonstrating generated task APIs.

## Traceability

| Requirement area | Architecture | Interfaces | Validation |
|---|---|---|---|
| MACRO-REQ-001..MACRO-REQ-007 | MACRO-ARCH-001..MACRO-ARCH-005 | MACRO-INT-001..MACRO-INT-007 | MACRO-VAL-001, MACRO-VAL-002, MACRO-VAL-003, MACRO-VAL-005 |
| MACRO-REQ-010..MACRO-REQ-017 | MACRO-ARCH-006, MACRO-ARCH-007 | MACRO-INT-008..MACRO-INT-010 | MACRO-VAL-004 |
