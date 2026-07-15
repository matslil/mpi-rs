# timeout-service Design Baseline

## Purpose and scope

`timeout-service` provides a local MPI service whose timeout operation is a
single-result stream. A requester supplies an absolute monotonic deadline. The
stream emits one payload-free `TimeoutOccurred` item at the deadline and then
finishes. Dropping the stream before completion uses ordinary MPI stream
cancellation.

The crate owns its monotonic `Time` and `TimeoutInstant` types. It does not own
custom request identity, reply routing, cancellation, backpressure, or target
termination machinery; those are supplied by the MPI stream protocol.

## Requirements

### TOS-REQ-001: Separate MPI service crate

The timeout service shall remain a separate crate depending on `mpi`.

Status: approved

### TOS-REQ-002: Crate-owned monotonic deadline

The crate shall expose `Time::now()` and `TimeoutInstant`; timeout stream
requests shall use an absolute deadline from that clock.

Verification: test and inspection

Status: approved

### TOS-REQ-003: Single-result timeout stream

Starting a timeout shall return an MPI stream that emits exactly one
payload-free `TimeoutOccurred` item at or after the deadline and then ends.

Verification: test and demonstration

Status: approved

### TOS-REQ-004: Standard stream cancellation

Dropping an unfinished timeout stream shall cancel it through ordinary MPI
stream cancellation. The timeout service shall define no separate timeout
cancel message.

Verification: test and inspection

Status: approved

### TOS-REQ-005: Standard stream infrastructure

Session identity, sender routing, target termination, queue backpressure,
terminal delivery, and cancellation shall use the existing MPI stream
infrastructure without a timeout-specific delivery closure or endpoint.

Verification: inspection and test

Status: approved

### TOS-REQ-006: Non-blocking timed handler

The timeout stream handler shall await `ctx.sleep_until(deadline).await`; it
shall not block its task thread with `thread::sleep`.

Verification: inspection and test

Status: approved

### TOS-REQ-007: Concurrent deadline ordering

Multiple timeout streams shall coexist, and later requests with earlier
deadlines may complete before earlier requests with later deadlines.

Verification: test

Status: approved

### TOS-REQ-008: Service lifetime

The returned service instance shall own the timeout task. Final instance drop
shall close and join that task.

Verification: test and inspection

Status: approved

## Architecture

- TOS-ARCH-001: `TimeoutInstant` wraps `std::time::Instant` and is created from
  the crate-owned `Time` API.
- TOS-ARCH-002: One generated MPI stream handler represents one timeout.
- TOS-ARCH-003: The handler awaits the shared task-context timed-suspension
  primitive, emits one item, and returns to finish the stream.
- TOS-ARCH-004: Ordinary stream drop and cancel messages cancel pending
  timeouts; no active-timeout registry or bespoke cancel protocol exists.
- TOS-ARCH-005: Generated MPI stream machinery owns session routing,
  backpressure, target termination, and terminal outcomes.
- TOS-ARCH-006: The service instance owns the generated timeout task handle and
  runtime join capability.

## Interface

```rust
pub struct TimeoutOccurred;

pub fn start_timeout_service() -> TimeoutServiceInstance;

impl TimeoutServiceInstance {
    pub fn timeout<C>(
        &self,
        ctx: &mut C,
        deadline: TimeoutInstant,
    ) -> Result<SuspendedMessageStream<TimeoutOccurred, TimeoutError>, SendError>;
}
```

- TOS-INT-001: `timeout` requires task scope and a declared ability to receive
  its stream events.
- TOS-INT-002: `TimeoutOccurred` carries no application payload.
- TOS-INT-003: Consumers use `stream.next(ctx).await` and cancel by dropping the
  stream.
- TOS-INT-004: Main scope may create the service and tasks but cannot start or
  consume timeout streams.

## Validation

### TOS-VAL-001: Receive a timeout

A task starts a timeout stream, awaits one occurrence, observes stream end, and
continues without blocking the timeout task.

Status: approved

### TOS-VAL-002: Cancel a timeout

A task drops a long timeout stream, then successfully receives a second short
timeout, demonstrating ordinary stream cancellation and continued service use.

Status: approved

### TOS-VAL-003: Complete out of request order

Several timeout streams with out-of-order deadlines complete in deadline order.

Status: approved

## Traceability

| Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|
| TOS-REQ-001, TOS-REQ-008 | TOS-ARCH-006 | TOS-INT-004 | TOS-VAL-001 |
| TOS-REQ-002, TOS-REQ-006 | TOS-ARCH-001..TOS-ARCH-003 | TOS-INT-001 | TOS-VAL-001 |
| TOS-REQ-003, TOS-REQ-005 | TOS-ARCH-002, TOS-ARCH-003, TOS-ARCH-005 | TOS-INT-001..TOS-INT-003 | TOS-VAL-001 |
| TOS-REQ-004 | TOS-ARCH-004 | TOS-INT-003 | TOS-VAL-002 |
| TOS-REQ-007 | TOS-ARCH-002, TOS-ARCH-003 | TOS-INT-001 | TOS-VAL-003 |
