# mpi-transaction Design Baseline

This document defines the crate-local systems-engineering baseline for the
`mpi-transaction` crate.

## Purpose

`mpi-transaction` contains transaction coordination support that depends on
message-based MPI protocols outside the core `mpi` crate. The first implemented
piece is a transaction decision log that records commit or abort decisions
through the `persistent-log-storage-service` protocol.

Keeping this crate separate prevents a dependency cycle: `persistent-log-storage-service`
depends on `mpi` to declare and serve its protocol, while `mpi-transaction`
depends on both `mpi` and `persistent-log-storage-service`.

## Requirements

### MPITX-REQ-001: Separate transaction crate

Storage-coupled transaction support shall live outside the core `mpi` crate.

Verification: inspection

Status: proposed

### MPITX-REQ-002: Protocol-backed decision log

The transaction decision log shall record decisions by sending store and commit
messages through the persistent log storage protocol.

Verification: test

Status: proposed

### MPITX-REQ-003: Recovery from protocol-backed log

The transaction decision log shall recover complete decision records by reading
entries through the persistent log storage protocol.

Verification: test

Status: proposed

## Architecture

- MPITX-ARCH-001: The core `mpi` crate owns transaction identifiers, paths, and
  transactional message envelopes that do not require persistent storage.
- MPITX-ARCH-002: `mpi-transaction` owns storage-coupled transaction logging.
- MPITX-ARCH-003: The transaction decision log depends on the
  `persistent_log_storage.v1` protocol rather than the storage implementation.
- MPITX-ARCH-004: Compiler-enforced side-effect and transaction-message
  separation remains a macro/type-system concern in `mpi` and `mpi-macros`; this
  crate consumes the generated protocol types without weakening those checks.

## Interface

The crate exposes:

```rust
pub struct TransactionDecisionLog<H>;
pub struct TransactionDecisionRecord;
pub enum TransactionLogError;
```

`TransactionDecisionLog<H>` is constructed from a
`persistent_log_storage::PersistentLogStorageProtocolV1::Binding<H>` and uses
blocking protocol calls for decision recording and recovery.

## Traceability

| Requirement | Architecture | Verification |
|---|---|---|
| MPITX-REQ-001 | MPITX-ARCH-001, MPITX-ARCH-002 | inspection |
| MPITX-REQ-002 | MPITX-ARCH-003 | test |
| MPITX-REQ-003 | MPITX-ARCH-003 | test |
