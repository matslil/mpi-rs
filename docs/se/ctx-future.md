# ctx-future Design Baseline

This document defines the lightweight systems-engineering baseline for the reusable `ctx-future` crate.

`ctx-future` is part of the repository engineering baseline, but it is intentionally independent from the `mpi-rs` message-passing model.

## Purpose

`ctx-future` provides future-like resumable computations that borrow caller-owned context only while being resumed.

The crate exists so a scheduler can keep multiple suspended computations while retaining ownership of the shared context needed to resume any one of them later.

## Scope

`ctx-future` is responsible for:

- context-returning suspension primitives;
- poll-like completion state;
- context borrowing rules that are independent of `mpi-rs`;
- tests and documentation proving the context borrow ends when `resume` returns.

`ctx-future` is not responsible for:

- task queues;
- message placement;
- `SessionId`;
- calls;
- streams;
- macro-generated `mpi-rs` task plumbing.

## Requirements

### CTX-REQ-001: Resume-time context borrow

A ctx-future shall receive mutable context only as an argument to its resume operation.

Verification: test

Status: approved

### CTX-REQ-002: No retained context borrow while pending

A ctx-future that returns pending shall not retain a mutable borrow of the context after the resume operation returns.

Verification: test

Status: approved

### CTX-REQ-003: Later resume

A pending ctx-future shall be resumable later with a compatible mutable context value.

Verification: test

Status: approved

### CTX-REQ-004: Standalone crate

The `ctx-future` crate shall be usable without depending on `mpi-rs`.

Verification: inspection

Status: approved

### CTX-REQ-005: Safe ordinary use

Ordinary use of the `ctx-future` public API shall not require `unsafe` Rust.

Verification: inspection

Status: approved

## Architecture

CTX-ARCH-001: The core abstraction is a future-like trait that receives context during resume.

CTX-ARCH-002: Completion state is represented by a poll-like result with pending and ready outcomes.

CTX-ARCH-003: Context ownership remains with the caller or scheduler.

CTX-ARCH-004: A pending ctx-future stores only its own continuation state, not a live mutable borrow of caller-owned context.

## Interface

The core interface is conceptually:

```rust
pub trait CtxFuture<Cx, Input = ()> {
    type Output;

    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output>;
}

pub enum CtxPoll<T> {
    Pending,
    Ready(T),
}
```

Interface rules:

CTX-INT-001: `CtxFuture::resume` shall borrow context for the duration of the call only.

CTX-INT-002: `CtxPoll::Pending` shall return control and context access to the caller.

CTX-INT-003: The crate shall not expose `mpi-rs` task, message, queue, session, call, or stream concepts.

## Verification

Verification should include tests showing that:

- context can be mutably used after a ctx-future returns pending;
- multiple pending ctx-futures can be stored while the caller continues to use context;
- a pending ctx-future can later be resumed with context;
- ordinary public API use does not require `unsafe`.

## Traceability

| ctx-future requirement | Architecture | Interface | Verification |
|---|---|---|---|
| CTX-REQ-001 | CTX-ARCH-001, CTX-ARCH-003 | CTX-INT-001 | test |
| CTX-REQ-002 | CTX-ARCH-003, CTX-ARCH-004 | CTX-INT-002 | test |
| CTX-REQ-003 | CTX-ARCH-001, CTX-ARCH-002 | CTX-INT-001, CTX-INT-002 | test |
| CTX-REQ-004 | standalone crate structure | CTX-INT-003 | inspection |
| CTX-REQ-005 | safe Rust implementation | public API inspection | inspection |
