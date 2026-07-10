# mpi-os-events Design Baseline

This document defines the lightweight systems-engineering baseline for the
`mpi-os-events` crate.

`mpi-os-events` provides operating-system and application-shell event bridges
for `mpi-rs`. It is intentionally separate from the `mpi-core` crate so the
message runtime can remain usable without platform integration dependencies.

## Purpose

The crate observes native operating-system events or framework-provided
application events and translates them into ordinary `mpi` messages.

The bridge is responsible for deciding where native event capture ends and
normal Rust message construction begins. It shall not require `mpi` core users
to depend on GUI, mobile, signal, or platform-event packages unless they opt
into this crate.

## Stakeholder Needs

The following original stakeholder need IDs remain part of this crate baseline:

- SN-044: Runtime users need POSIX signal integration to avoid unsafe allocation or non-async-signal-safe operations in signal handlers.
- SN-046: Runtime users need operating-system and application-framework events to be translated into typed `mpi` messages without making the core message runtime depend on every supported platform integration.

## Requirements

The original workspace-level `REQ-*` IDs remain stable after migration. The
`OS-EVT-REQ-*` IDs below are local grouping aliases; they do not replace the
original IDs used by tests, reports, and traceability.

### Migrated stable requirements

- REQ-130: The POSIX signal handler portion of any Unix signal support shall perform only async-signal-safe operations.
- REQ-131: Unix signal forwarding shall use a bridge that observes async-signal-safe state or notification and then sends normal Rust messages outside the signal handler.
- REQ-132: Unix signal bridge support shall be controlled by an optional `mpi-os-events` crate feature that is included in that crate's default feature set.
- REQ-133: Operating-system and application-framework event bridge support shall live in a separate workspace crate named `mpi-os-events`.
- REQ-134: The OS event bridge shall support translating native operating-system events into typed `mpi` messages.
- REQ-135: For each supported OS or framework event, the bridge design shall consider whether the source event is asynchronous or synchronous.
- REQ-136: The OS event bridge shall support Linux, Windows, and macOS through native platform event APIs.
- REQ-137: The OS event bridge shall support Android and iOS through framework adapters; Tauri shall be the initial Android and iOS framework adapter, preserving a path to future adapters.

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

The original architecture IDs CMP-013 and ARCH-090 through ARCH-096 remain
stable for this crate. The `OS-EVT-ARCH-*` IDs below are grouping aliases.

Stable architecture ID anchors:

- CMP-013
- ARCH-090, ARCH-091, ARCH-092, ARCH-093, ARCH-094, ARCH-095, ARCH-096

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

The original interface IDs INT-110 through INT-115 remain stable for this
crate. The `OS-EVT-INT-*` IDs below are grouping aliases.

Stable interface ID anchors:

- INT-110, INT-111, INT-112, INT-113, INT-114, INT-115

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

## Validation

The original validation ID VAL-012 remains stable for this crate. The
`OS-EVT-VAL-*` IDs below are grouping aliases.

### OS-EVT-VAL-001: Forward Unix signals safely

Status: approved

A developer forwards a Unix signal into a target `mpi` task as an ordinary Rust message.

Expected outcome:

- the POSIX signal handler performs only async-signal-safe operations;
- a bridge thread constructs and sends the normal Rust message;
- disabling default features removes Unix signal bridge APIs from this crate.

Evidence type: inspection, Unix-host test, and demonstration

### OS-EVT-VAL-002: Preserve OS interaction kind

Status: approved

A developer maps an operating-system or framework event into a target `mpi` interaction.

Expected outcome:

- asynchronous source events map to event-style messages when no reply is required;
- synchronous source events map to call-style interactions or another explicit reply path when the platform expects a decision;
- platform-specific dependencies remain outside the `mpi-core` crate.

Evidence type: inspection and adapter-specific tests as adapters are added

## Traceability

| Requirement | Architecture | Interface | Verification | Validation |
|---|---|---|---|---|
| OS-EVT-REQ-001 | OS-EVT-ARCH-001, OS-EVT-ARCH-002 | OS-EVT-INT-001 | inspection | OS-EVT-VAL-002 |
| OS-EVT-REQ-002 | OS-EVT-ARCH-006 | OS-EVT-INT-002, OS-EVT-INT-003 | inspection | OS-EVT-VAL-002 |
| OS-EVT-REQ-003 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test | OS-EVT-VAL-002 |
| OS-EVT-REQ-004 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test | OS-EVT-VAL-002 |
| OS-EVT-REQ-005 | OS-EVT-ARCH-004 | OS-EVT-INT-001 | inspection, test | OS-EVT-VAL-002 |
| OS-EVT-REQ-006 | OS-EVT-ARCH-005 | OS-EVT-INT-001 | inspection | OS-EVT-VAL-002 |
| OS-EVT-REQ-007 | OS-EVT-ARCH-003 | OS-EVT-INT-004 | inspection | OS-EVT-VAL-001 |
| OS-EVT-REQ-008 | OS-EVT-ARCH-003 | OS-EVT-INT-001, OS-EVT-INT-004 | inspection | OS-EVT-VAL-001 |
| OS-EVT-REQ-009 | OS-EVT-ARCH-003 | OS-EVT-INT-004 | test | OS-EVT-VAL-001 |
