# timeout-service Design Baseline

This document defines the lightweight systems-engineering baseline for the
`timeout-service` crate.

The `timeout-service` crate provides an `mpi`-based local timeout service. It is
specified as a separate crate so timeout behavior, platform timer assumptions,
and timeout request/cancel protocol rules can evolve without being folded into
the `mpi-core` crate.

## Purpose

`timeout-service` lets tasks suspend a typed call until a session-associated
deadline expires.

The timeout service receives call requests with an absolute monotonic deadline.
The `mpi` call machinery supplies a mandatory `SessionId` that correlates each
request with its reply. If the deadline expires before a matching cancel is
processed, the timeout service replies with `()`, so neither the request nor the
reply carries an application-specific timeout payload.

## Scope

`timeout-service` is responsible for:

- defining the timeout service requirements, architecture, and public timing
  interface;
- providing a crate-owned monotonic time type and `Time::now()` function;
- accepting timeout requests keyed by `SessionId`;
- accepting best-effort timeout cancel messages keyed by `SessionId`;
- detecting illegal duplicate active timeout requests for the same `SessionId`;
- replying to expired timeout calls with `()` through `mpi` call behavior;
- returning a service instance whose lifetime owns the timeout task and its
  protocol binding;
- documenting platform timer expectations for Linux, Windows, macOS, iOS, and
  Android.

`timeout-service` is not responsible for:

- defining application-specific timeout payload types;
- replacing `mpi` task queues, `SessionId`, message placement, or cancellation
  semantics;
- implementing distributed clock synchronization.

## Stakeholder Needs

### TOS-SN-001: Local timeout scheduling

Rust developers need a reusable local service that can deliver an `mpi` message
when a session-associated deadline expires.

### TOS-SN-002: Consistent timeout clock

Rust developers need timeout requesters and the timeout service to use the same
monotonic time source and deadline type.

### TOS-SN-003: Cancellation race tolerance

Runtime users need timeout cancellation races to be safe when a timeout expires
while a matching cancel message is in transit.

### TOS-SN-004: Payload-free timeout replies

Rust developers need timeout expiry to be represented by completion of the
original typed call, without echoing an application payload through the timeout
service.

### TOS-SN-005: Portable local timers

Runtime users need timeout behavior to be implementable on Linux, Windows,
macOS, iOS, and Android.

### TOS-SN-006: Timeout service lifetime

Rust developers need the timeout service protocol binding to be owned by a
service instance so sends cannot outlive the timeout service task.

## Requirements

### TOS-REQ-001: Separate crate

The timeout service shall be provided by a separate workspace crate named
`timeout-service`.

Source: human maintainer decision.

Verification: inspection

Status: proposed

### TOS-REQ-002: Depends on mpi

The `timeout-service` crate shall depend on the `mpi` crate for message handling
and session identity.

Source: TOS-SN-001.

Verification: inspection

Status: proposed

### TOS-REQ-003: Crate-owned monotonic time API

The `timeout-service` crate shall provide a crate-owned monotonic time interface
named `Time` with a `now()` operation that returns the deadline type used by
timeout requests.

Source: TOS-SN-002.

Verification: test and inspection

Status: proposed

### TOS-REQ-004: Absolute monotonic deadlines

A timeout request shall identify its timeout using an absolute deadline measured
against the `timeout-service` monotonic time source.

Source: TOS-SN-002.

Verification: test

Status: proposed

### TOS-REQ-005: Timeout request identity

Each timeout request shall carry a `SessionId` that identifies the logical
timeout interaction.

Source: TOS-SN-001, TOS-SN-003.

Verification: test

Status: proposed

### TOS-REQ-006: Payload-free timeout reply

The timeout service call shall reply with `()` on expiry and shall not accept,
store, or return an application-specific timeout payload.

Source: TOS-SN-004.

Verification: inspection and test

Status: proposed

### TOS-REQ-007: Expiry delivery

When an active timeout request reaches its deadline before a matching cancel is
processed, the timeout service shall complete the original call with a `()`
reply carrying the same `SessionId` as the request.

Source: TOS-SN-001.

Verification: test

Status: proposed

### TOS-REQ-008: Duplicate active request rejection

The timeout service shall reject a timeout request whose `SessionId` already has
an active timeout request.

Source: human maintainer decision.

Verification: test

Status: proposed

### TOS-REQ-009: Best-effort cancellation

A timeout cancel message shall discard the active timeout request with the
matching `SessionId` when the cancel is processed before expiry delivery begins.

Source: TOS-SN-003.

Verification: test

Status: proposed

### TOS-REQ-010: Cancellation race semantics

A timeout cancel message shall be best-effort: if expiry delivery has already
begun or completed, the cancel is not required to prevent timeout message
delivery.

Source: TOS-SN-003.

Verification: analysis and test

Status: proposed

### TOS-REQ-011: Priority cancel receive placement

The timeout service's receive declaration shall place timeout cancel messages in
the priority placement class.

Source: human maintainer decision, `mpi` receiver-declared placement model.

Verification: inspection

Status: proposed

### TOS-REQ-012: Unknown timeout receiver guidance (obsolete)

The timeout service design shall document that receivers should discard
unknown-session timeout messages when cancellation races are expected.

Source: TOS-SN-003.

Verification: inspection

Status: obsolete

### TOS-REQ-013: Expiry send backpressure

The timeout service shall use normal `mpi` call-reply behavior when an expired
timeout reply cannot immediately be enqueued to its requester.

Source: human maintainer decision.

Verification: test

Status: proposed

### TOS-REQ-014: Locally bounded expiry send wait (obsolete)

The timeout service shall bound its own wait while delivering an expired timeout
message with a timeout handled by local timer primitives rather than by
submitting another timeout request to itself.

Source: human maintainer decision.

Verification: test and inspection

Status: obsolete

### TOS-REQ-015: Supported platform timer basis

The timeout service design shall support implementation on Linux, Windows,
macOS, iOS, and Android using local OS or standard-library monotonic timer
primitives.

Source: TOS-SN-005.

Verification: inspection

Status: proposed

### TOS-REQ-016: Service instance lifetime

The timeout service start function shall return a service instance that owns
the timeout task and exposes the timeout protocol binding.

Source: TOS-SN-006.

Verification: inspection and test

Status: proposed

### TOS-REQ-017: Feature-gated service inclusion

The `mpi` facade feature `enable-timeout-service` shall enable the timeout service
and any supporting `mpi` or `mpi-macros` integration required by that service.

Source: repository optional crate feature naming convention.

Verification: inspection

Status: proposed

## Architecture

| ID | Component | Responsibility |
|---|---|---|
| TOS-CMP-001 | Timeout service task | Receives timeout calls and cancels, stores active requests, and completes expired calls. |
| TOS-CMP-002 | Time API | Provides `Time::now()` and the crate-owned monotonic deadline type. |
| TOS-CMP-003 | Active timeout registry | Tracks active requests by `SessionId` and rejects duplicate active requests. |
| TOS-CMP-004 | Timer backend | Waits until the next active deadline using local monotonic timer primitives. |
| TOS-CMP-005 | Call reply | Holds the payload-free reply sender associated with the request `SessionId`. |
| TOS-CMP-006 | Timeout service instance | Owns the timeout task lifetime and exposes the timeout protocol binding. |

Architecture rules:

- TOS-ARCH-001: Timeout deadlines are absolute values from the crate-owned
  monotonic time source.
- TOS-ARCH-002: The timeout service stores active requests by `SessionId`.
- TOS-ARCH-003: At most one active timeout request may exist for a `SessionId`.
- TOS-ARCH-004: Timeout cancel processing removes a matching active request if
  the request has not begun expiry delivery.
- TOS-ARCH-005: Timeout cancellation is race-tolerant and best-effort.
- TOS-ARCH-006: Timeout cancel messages are priority messages in the timeout
  service receive declaration.
- TOS-ARCH-007: Timeout requests contain only the absolute deadline payload;
  timeout replies contain `()`.
- TOS-ARCH-008: Expired timeout delivery uses `mpi` call-reply semantics and
  the request `SessionId` for correlation.
- TOS-ARCH-009: Obsolete; payload-free call replies replace the former opaque
  delivery-operation retry loop.
- TOS-ARCH-010: Platform-specific timer backend details are hidden behind the
  crate time and timer backend interfaces.
- TOS-ARCH-011: The timeout service is exposed through a service instance whose
  lifetime owns the timeout task and protocol binding.

## Interface

Conceptual public timing interface:

```rust
pub struct Time;

impl Time {
    pub fn now() -> TimeoutInstant;
}

pub struct TimeoutInstant {
    // crate-owned monotonic deadline representation
}
```

Conceptual timeout request fields:

```text
TimeoutRequest {
    deadline: TimeoutInstant,
}
```

Conceptual timeout cancel fields:

```text
TimeoutCancel {
    session_id: SessionId,
}
```

Conceptual service interface:

```rust
mpi::protocol! {
    pub protocol TimeoutServiceProtocolV1 {
        call request(TimeoutRequest) -> ();
        event cancel(TimeoutCancel);
    }
}

pub fn start_timeout_service<const N: usize>() -> TimeoutServiceInstance<N>;

impl<const N: usize> TimeoutServiceInstance<N> {
    pub fn protocol(&self) -> TimeoutServiceProtocolV1::Binding<&TimeoutServiceEndpoint<N>>;
}
```

Interface rules:

- TOS-INT-001: Timeout requesters shall use `Time::now()` or values derived
  from the same crate-owned monotonic time type when constructing deadlines.
- TOS-INT-002: Timeout request deadlines shall use `TimeoutInstant`.
- TOS-INT-003: The `mpi` call machinery shall add a mandatory `SessionId` to
  timeout requests, and timeout cancels shall carry that `SessionId`.
- TOS-INT-004: The timeout request payload shall contain only the absolute
  deadline, and the timeout reply payload shall be `()`.
- TOS-INT-005: Timeout cancel messages shall be declared as priority messages by
  the timeout service.
- TOS-INT-006: Dropping an active suspended timeout call shall submit a
  best-effort cancel keyed by its `SessionId`; late replies use the caller's
  declared `mpi` late-reply policy.
- TOS-INT-007: The timeout service start function shall return a timeout
  service instance.
- TOS-INT-008: The timeout service instance shall expose the timeout protocol
  binding without allowing that binding to outlive the service instance.
- TOS-INT-009: The service shall be enabled by the `mpi` facade feature
  `enable-timeout-service`.

## Verification

Verification should include:

- inspection that the crate is separate and depends on `mpi`;
- tests or inspection showing `Time::now()` and timeout request deadlines use
  the same monotonic time type;
- tests showing timeout expiry completes the original call with `()`;
- tests showing timeout cancel removes a pending request;
- tests or analysis showing timeout/cancel races are best-effort and safe;
- tests showing duplicate active requests for a `SessionId` are rejected;
- inspection showing timeout cancel receive placement is priority;
- inspection showing request/reply correlation uses the call `SessionId`;
- inspection that the timeout service start API returns a service instance and
  exposes no detached protocol binding;
- inspection that the `mpi` facade feature name is `enable-timeout-service`;
- platform inspection for Linux, Windows, macOS, iOS, and Android timer support.

## Validation

### TOS-VAL-001: Schedule and receive a timeout

Status: proposed

A task makes a timeout call with an absolute monotonic deadline. The `mpi` call
machinery supplies the request `SessionId`; the timeout service stores the
request and later replies with `()` when the deadline expires.

Expected outcome:

- the sender and timeout service use the same time type;
- the timeout service receives no application payload;
- the original suspended call completes with a payload-free reply correlated by
  its `SessionId`.

Evidence type: test or demonstration

### TOS-VAL-002: Cancel before expiry

Status: proposed

A task schedules a timeout and sends a matching timeout cancel before the
deadline expires.

Expected outcome:

- the timeout cancel is priority in the timeout service;
- the active request is discarded;
- no timeout reply completes the canceled request.

Evidence type: test

### TOS-VAL-003: Race between expiry and cancel

Status: proposed

A timeout expires while a matching cancel is in transit or processed too late to
prevent delivery.

Expected outcome:

- the timeout service treats cancellation as best-effort;
- the timeout reply may be delivered;
- if the suspended call has been dropped, the caller's declared late-reply
  policy handles the reply.

Evidence type: analysis and test

### TOS-VAL-004: Timeout reply backpressure

Status: proposed

An expired timeout reply cannot immediately enqueue because the requester queue
is full.

Expected outcome:

- the timeout service follows normal `mpi` suspension behavior;
- the timeout service uses the normal call-reply queue-space wakeup path.

Evidence type: test

### TOS-VAL-005: Use timeout as a service

Status: proposed

A task starts the timeout service, schedules and cancels timeouts through the
protocol binding exposed by the returned service instance, and drops the final
service instance clone.

Expected outcome:

- the protocol binding is accessed through the service instance;
- the binding cannot outlive the service instance;
- dropping the final service instance clone synchronizes timeout task stop.

Evidence type: test or demonstration

## Traceability

| Requirement | Architecture | Interface | Verification | Validation |
|---|---|---|---|---|
| TOS-REQ-001 | crate structure | crate manifest | inspection | TOS-VAL-001 |
| TOS-REQ-002 | TOS-CMP-001 | crate manifest | inspection | TOS-VAL-001 |
| TOS-REQ-003, TOS-REQ-004 | TOS-CMP-002, TOS-ARCH-001 | TOS-INT-001, TOS-INT-002 | test, inspection | TOS-VAL-001 |
| TOS-REQ-005 | TOS-CMP-003, TOS-ARCH-002 | TOS-INT-003 | test | TOS-VAL-001, TOS-VAL-002 |
| TOS-REQ-006, TOS-REQ-007 | TOS-CMP-005, TOS-ARCH-007, TOS-ARCH-008 | TOS-INT-004 | test, inspection | TOS-VAL-001 |
| TOS-REQ-008 | TOS-CMP-003, TOS-ARCH-003 | duplicate rejection result | test | TOS-VAL-001 |
| TOS-REQ-009, TOS-REQ-010 | TOS-ARCH-004, TOS-ARCH-005 | TOS-INT-003 | test, analysis | TOS-VAL-002, TOS-VAL-003 |
| TOS-REQ-011 | TOS-ARCH-006 | TOS-INT-005 | inspection | TOS-VAL-002 |
| TOS-REQ-012 (obsolete) | TOS-ARCH-005 | TOS-INT-006 | inspection | TOS-VAL-003 |
| TOS-REQ-013 | TOS-ARCH-008 | `mpi` call reply | test, inspection | TOS-VAL-004 |
| TOS-REQ-014 (obsolete) | TOS-ARCH-009 | none | inspection | none |
| TOS-REQ-015 | TOS-CMP-004, TOS-ARCH-010 | platform backend interface | inspection | TOS-VAL-001 |
| TOS-REQ-016 | TOS-CMP-006, TOS-ARCH-011 | TOS-INT-007, TOS-INT-008 | test, inspection | TOS-VAL-005 |
| TOS-REQ-017 | feature configuration | TOS-INT-009 | inspection | TOS-VAL-005 |
