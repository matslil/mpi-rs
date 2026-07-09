# mpi-os-events Design Baseline

This document defines the lightweight systems-engineering baseline for the
`mpi-os-events` crate.

`mpi-os-events` provides operating-system and application-shell event bridges
for `mpi-rs`. It is intentionally separate from the core `mpi` crate so the
message runtime can remain usable without platform integration dependencies.

## Purpose

The crate observes native operating-system events or framework-provided
application events and translates them into ordinary `mpi` messages.

The bridge is responsible for deciding where native event capture ends and
normal Rust message construction begins. It shall not require `mpi` core users
to depend on GUI, mobile, signal, or platform-event packages unless they opt
into this crate.

## Requirements

### OS-EVT-REQ-001: Native OS event bridge

The crate shall support bridges that receive native operating-system or
framework events and translate them to messages sent to `mpi` task handles.

Verification: inspection

Status: approved

### OS-EVT-REQ-002: Interaction kind preservation

For each supported OS or framework event, the bridge design shall consider
whether the source event is asynchronous or synchronous. Asynchronous OS events
shall be translated to asynchronous messages when no reply is required.
Synchronous OS events shall be translated to synchronous `mpi` interactions
when the operating system or framework expects a reply or decision.

Verification: inspection

Status: approved

### OS-EVT-REQ-003: Linux support

The crate shall support Linux native OS events.

Verification: inspection and test where available

Status: approved

### OS-EVT-REQ-004: Windows support

The crate shall support Windows native OS events.

Verification: inspection and test where available

Status: approved

### OS-EVT-REQ-005: macOS support

The crate shall support macOS native OS events.

Verification: inspection and test where available

Status: approved

### OS-EVT-REQ-006: Mobile framework adapters

The crate shall support Android and iOS OS or application lifecycle events
through framework adapters. Tauri shall be the initial mobile framework adapter,
but the design shall allow future adapters for other mobile frameworks.

Verification: inspection

Status: approved

### OS-EVT-REQ-007: Async-signal-safe Unix signal handling

The POSIX signal handler portion of any Unix signal support shall perform only
async-signal-safe operations.

Verification: inspection

Status: approved

### OS-EVT-REQ-008: Unix signal bridge

Unix signal forwarding shall use a bridge that observes async-signal-safe state
or notification and then sends normal Rust messages outside the signal handler.

Verification: inspection

Status: approved

### OS-EVT-REQ-009: Optional Unix signal feature

Unix signal bridge support shall be controlled by an optional crate feature
that is included in the default feature set.

Verification: test

Status: approved

## Architecture

OS-EVT-ARCH-001: `mpi-os-events` depends on `mpi` and sends messages through
`TaskHandle` or equivalent public task-handle APIs.

OS-EVT-ARCH-002: Event observation is platform-specific; message construction
shall happen in ordinary Rust execution context unless the source API explicitly
permits ordinary Rust work in its callback.

OS-EVT-ARCH-003: Unix signal handling uses an async-signal-safe registration
mechanism and a normal bridge thread to construct and send messages.

OS-EVT-ARCH-004: Linux, Windows, and macOS bridges should use native platform
event APIs rather than requiring an application framework.

OS-EVT-ARCH-005: Android and iOS bridge support is framework-mediated. The
initial framework target is Tauri, and framework-specific code shall be kept
behind adapter boundaries so another framework can be added later.

OS-EVT-ARCH-006: A bridge shall document whether each source event maps to an
asynchronous event message, a synchronous call, or another typed interaction.

## Interface

OS-EVT-INT-001: Bridge APIs shall accept a target task handle or equivalent
send surface and a mapping from platform event data to the target task's message
or interaction.

OS-EVT-INT-002: Asynchronous source events should use event-style APIs that do
not imply an operating-system or framework reply.

OS-EVT-INT-003: Synchronous source events should use call-style APIs or another
explicit reply path when the operating system or framework requires a decision.

OS-EVT-INT-004: Unix signal APIs shall be available on Unix targets when the
default-enabled `unix-signals` feature is active, and shall be removable from
the public API by disabling that feature.

## Verification

Verification should include:

- inspection that bridge dependencies are isolated from `mpi` core;
- inspection that Unix signal handlers do not construct normal Rust messages;
- tests for feature-gated Unix signal forwarding on Unix hosts;
- native Linux, Windows, and macOS bridge tests as those implementations are
  added;
- framework-adapter tests for Android and iOS lifecycle events, beginning with
  Tauri and leaving evidence for future adapter portability.

## Traceability

| Requirement | Architecture | Interface | Verification |
|---|---|---|---|
| OS-EVT-REQ-001 | OS-EVT-ARCH-001, OS-EVT-ARCH-002 | OS-EVT-INT-001 | inspection |
| OS-EVT-REQ-002 | OS-EVT-ARCH-006 | OS-EVT-INT-002, OS-EVT-INT-003 | inspection |
| OS-EVT-REQ-003 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test |
| OS-EVT-REQ-004 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test |
| OS-EVT-REQ-005 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test |
| OS-EVT-REQ-006 | OS-EVT-ARCH-005 | OS-EVT-INT-001 | inspection |
| OS-EVT-REQ-007 | OS-EVT-ARCH-003 | OS-EVT-INT-004 | inspection |
| OS-EVT-REQ-008 | OS-EVT-ARCH-003 | OS-EVT-INT-001, OS-EVT-INT-004 | inspection |
| OS-EVT-REQ-009 | OS-EVT-ARCH-003 | OS-EVT-INT-004 | test |
