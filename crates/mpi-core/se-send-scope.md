# Send API Scope

This document is part of the authoritative systems-engineering baseline named
`se-*.md`.

Scope: `crates/mpi-core`.

It records the human-approved decision that application messaging is available
only from generated task scope. External code may construct tasks, supply their
initial dependencies, and join their runtimes, but it may not send or receive
application or protocol messages.

## Requirements

### REQ-122: External construction scope

External code shall be able to construct tasks, provide creation-time state and
service dependencies, and join task runtimes, but shall not send or receive
application or protocol messages.

Source: Human maintainer decision.

Verification: compile-fail test and inspection

Status: approved

### REQ-123: Task-scoped messaging

Generated event, call, stream, cancellation, reply, and forwarding APIs shall
require access to generated task scope.

Source: Human maintainer decision, SN-012, SN-015, SN-023.

Verification: compile-fail test and inspection

Status: approved

### REQ-127: Message routing header

Every ordinary protocol message shall carry infrastructure-owned sender and
`SessionId` metadata in addition to its declared payload. The receiver shall be
implicit from the queue in which the message is delivered, and the protocol
shall define which messages reply to which requests.

Source: Human maintainer decision.

Verification: test and inspection

Status: approved

### REQ-128: Proxy envelope

The infrastructure shall support an explicitly type-erased envelope whose
payload is another complete message. Sending the envelope creates a new outer
sender and session while preserving the inner message sender, session, and
payload. Replies to the outer message shall return through the proxy.

Source: Human maintainer decision.

Verification: test

Status: approved

## Architecture rules

- ARCH-083: Generated messaging methods require task scope; no external
  `_blocking` application messaging surface is generated.
- ARCH-084: Runtime-internal queue operations remain available to task startup,
  dispatch, reply, forwarding, and shutdown machinery but are not an external
  application messaging API.
- ARCH-085: Main or other external construction scope may spawn tasks, inject
  initial dependencies, and join runtimes only.
- ARCH-086: Sender and session are infrastructure-owned message-header fields;
  receiver and request/reply relationships are not duplicated in the message.
- ARCH-087: Proxy forwarding is represented by an outer message whose payload
  is a type-erased inner message; both request and reply therefore traverse the
  proxy unless an application deliberately defines another protocol.

## Interface rules

- INT-093: Generated event, call, stream, cancellation, reply, and forwarding
  methods shall require a generated task context or sealed `TaskScope` trait.
- INT-094: Generated handles and protocol bindings shall not expose external
  `_blocking` send or receive methods.
- INT-095: Examples shall show main constructing tasks and joining runtimes,
  while task handlers perform all messaging.
- INT-096: Direct typed sends use generated protocol bindings; proxy code opts
  into infrastructure-provided type erasure and runtime receiver validation.

## Traceability

| Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|
| REQ-122, REQ-123 | ARCH-083, ARCH-084, ARCH-085 | INT-093, INT-094, INT-095 | VAL-004, VAL-011 |
| REQ-127 | ARCH-086 | INT-093 | VAL-004, VAL-005 |
| REQ-128 | ARCH-087 | INT-096 | proxy forwarding test |
