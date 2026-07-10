# persistent-log-storage-service Design Baseline

This document defines the lightweight systems-engineering baseline for the
`persistent-log-storage-service` crate.

The crate provides a default file-backed persistent log store that transaction
coordination code can use for crash recovery. It is specified as a separate
crate so the storage interface, file format, and crash-safety rules can evolve
without being folded into the `mpi-core` crate.

## Purpose

`persistent-log-storage-service` stores ordered log entries, durably commits log
entries through a caller-specified index, durably discards log entries through a
caller-specified index, and reads back durable log entries after restart. The
public storage service boundary is message based: callers use the
`PersistentLogStorageProtocolV1` protocol to submit store, commit, discard, and
read calls and receive typed replies through a service instance. The only
public crate surface is the protocol, service instance, protocol-binding helper
methods, and the file-backed service start function.

The intended first consumer is transactional messaging recovery. A transaction
coordinator can append transaction decision records, commit through those
records, and recover committed records after a crash.

## Scope

`persistent-log-storage-service` is responsible for:

- defining persistent log storage requirements, architecture, and interfaces;
- providing a default file-backed implementation;
- appending ordered binary log entries;
- committing log entries through a specific index by waiting until the backing
  file has been synchronized to persistent storage;
- discarding log entries through a specific index by durably recording a discard
  watermark;
- reading back complete non-discarded log entries after restart;
- exposing a named MPI protocol with store, commit, discard, and read calls;
- providing a public service start function for the default file-backed storage
  service task;
- returning a service instance whose lifetime owns the storage task and its
  protocol binding;
- providing typed serialization helpers through a general serde interface;
- tolerating torn trailing records caused by crash or power loss.

`persistent-log-storage-service` is not responsible for:

- interpreting transaction record payloads;
- deciding transaction commit or abort outcomes;
- providing distributed replication;
- guaranteeing durability on storage devices or filesystems that acknowledge
  synchronization before data is actually persistent;
- preventing arbitrary application writes outside this crate.

## Stakeholder Needs

### PLS-SN-001: Transaction recovery log

Transactional messaging needs a durable log store for commit, abort, prepare,
and recovery records.

### PLS-SN-002: Default local file storage

Rust developers need a default local file-backed implementation before choosing
or integrating a custom storage backend.

### PLS-SN-003: Explicit persistence boundary

Transaction coordination needs an operation that does not return until all log
records through a selected index have been synchronized to persistent storage.

### PLS-SN-004: Log compaction by discard

Long-running applications need a way to discard log entries that are no longer
needed for recovery.

### PLS-SN-005: Crash-safe recovery reads

Recovery needs to read complete durable log entries after restart without
failing merely because the final record was torn by a crash.

### PLS-SN-006: Message-based and typed log access

Transaction coordination code needs the log storage interface to fit a
message-passing architecture, and application-facing recovery code needs
helpers that store serializable objects and retrieve deserialized objects.
The crate should enforce Wincode as the serde-backed binary encoding rather
than exposing several persistent serialization formats.

## Requirements

### PLS-REQ-001: Separate crate

The persistent log storage service implementation shall be provided by a
separate workspace crate named `persistent-log-storage-service`.

Source: human maintainer decision.

Verification: inspection

Status: proposed

### PLS-REQ-002: Append log entry

The crate shall provide an operation that appends a binary log entry and returns
the entry's monotonically increasing log index.

Source: PLS-SN-001.

Verification: test

Status: proposed

### PLS-REQ-003: Commit through index

The crate shall provide an operation that commits log entries through a
specified log index and does not return success until all log entries through
that index have been synchronized to persistent storage.

Source: PLS-SN-003.

Verification: test and inspection

Status: proposed

### PLS-REQ-004: Discard through index

The crate shall provide an operation that discards log entries through a
specified log index and durably records that discard before returning success.

Source: PLS-SN-004.

Verification: test

Status: proposed

### PLS-REQ-005: Read committed log entries

The crate shall provide an operation that reads back complete non-discarded log
entries from the backing store.

Source: PLS-SN-005.

Verification: test

Status: proposed

### PLS-REQ-006: Crash-safe trailing record handling

When reading the backing file, the file-backed implementation shall ignore a
torn or incomplete trailing record instead of returning it as a valid log entry.

Source: PLS-SN-005.

Verification: test

Status: proposed

### PLS-REQ-007: Payload opacity

The crate shall store log entry payloads as opaque bytes and shall not interpret
transaction-specific record contents.

Source: PLS-SN-001.

Verification: inspection

Status: proposed

### PLS-REQ-008: Safe Rust API

Ordinary public API use shall not require `unsafe` Rust.

Source: repository safety guidance.

Verification: inspection

Status: proposed

### PLS-REQ-009: Message-based log operations

The crate shall expose a named persistent log storage protocol through a
service instance with store, commit, discard, and read calls and corresponding
return payloads, rather than exposing only primitive parameter lists.

Source: PLS-SN-006.

Verification: test and inspection

Status: proposed

### PLS-REQ-010: Serde-backed typed payload helpers

The crate shall provide helpers that store serializable payload objects and
retrieve deserialized payload objects through a general serde interface.

Source: PLS-SN-006.

Verification: test

Status: proposed

### PLS-REQ-011: Wincode serialization encoding

The crate shall use Wincode for serde-backed typed payload serialization and
shall not expose multiple persistent serialization format choices in the log
storage interface.

Source: human maintainer decision.

Verification: test and inspection

Status: proposed

### PLS-REQ-012: Service instance lifetime

The file-backed persistent log storage service start function shall return a
service instance that owns the storage task and exposes the
`PersistentLogStorageProtocolV1` binding.

Source: repository service convention.

Verification: inspection and test

Status: proposed

### PLS-REQ-013: Feature-gated service inclusion

The `mpi` facade feature `enable-persistent-log-storage-service` shall enable the
persistent log storage service and any supporting `mpi` or `mpi-macros`
integration required by that service.

Source: repository optional crate feature naming convention.

Verification: inspection

Status: proposed

## Architecture

| ID | Component | Responsibility |
|---|---|---|
| PLS-CMP-001 | Persistent log storage service task | Serves the public MPI protocol for store, commit, discard, and read-back calls. |
| PLS-CMP-002 | File log storage | Provides the default crash-safe file-backed implementation. |
| PLS-CMP-003 | Log record codec | Encodes and validates append and discard records. |
| PLS-CMP-004 | Recovery scanner | Reads complete records, applies the latest discard watermark, and ignores torn trailing records. |
| PLS-CMP-005 | Persistent log storage protocol | Defines the protocol calls and return payloads for store, commit, discard, and read operations. |
| PLS-CMP-006 | Serialization adapter | Encodes serde-serializable objects with Wincode before store and deserializes retrieved payloads after read. |
| PLS-CMP-007 | Persistent log storage service instance | Owns the file-backed storage task lifetime and exposes the protocol binding. |

Architecture rules:

- PLS-ARCH-001: Log entry indexes are monotonically increasing `u64` values.
- PLS-ARCH-002: A file log uses append records for payloads and discard records
  for discard watermarks.
- PLS-ARCH-003: `commit(CommitLogEntries { through: index })` synchronizes the backing file before
  returning success.
- PLS-ARCH-004: `discard(DiscardLogEntries { through: index })` appends a discard watermark and
  synchronizes the backing file before returning success.
- PLS-ARCH-005: Recovery scanning stops before a torn trailing record and does
  not return that record as a valid entry.
- PLS-ARCH-006: Log payload bytes remain opaque to this crate.
- PLS-ARCH-007: The file format remains byte-oriented even when callers use
  typed serialization helpers.
- PLS-ARCH-008: Serialization helpers shall be feature-gated, shall use
  Wincode for encoded bytes, and shall not require transaction-specific payload
  knowledge in this crate.
- PLS-ARCH-009: The public protocol shall be declared as
  `PersistentLogStorageProtocolV1`. Compatible protocol evolution may add
  request or reply message variants, while incompatible changes require a new
  protocol version.
- PLS-ARCH-010: The default file-backed implementation shall be exposed as a
  service whose service instance owns the task lifetime and contains the
  protocol binding.
- PLS-ARCH-011: Direct access to file-backed service task state is outside the
  public interface.

## Interface

Public interface:

```rust
protocol! {
    pub protocol PersistentLogStorageProtocolV1 {
        call store(Vec<u8>) -> Result<u64, String>;
        call commit(u64) -> Result<(), String>;
        call discard(u64) -> Result<(), String>;
        call read(Option<u64>) -> Result<Vec<(u64, Vec<u8>)>, String>;
    }
}

pub fn start_file_log_storage_service(
    path: impl AsRef<Path>,
) -> Result<PersistentLogStorageServiceInstance, String>;

pub struct PersistentLogStorageServiceInstance {
    // owns the storage service task and exposes the protocol binding
}

impl PersistentLogStorageServiceInstance {
    pub fn store_serialized_blocking<T>(
        &self,
        payload: T,
    ) -> Result<Result<u64, String>, mpi::CallError>;

    pub fn read_serialized_blocking<T>(
        &self,
        from: Option<u64>,
    ) -> Result<Result<Vec<(u64, T)>, String>, mpi::CallError>;
}
```

Interface rules:

- PLS-INT-001: The persistent log storage protocol shall be declared as
  `PersistentLogStorageProtocolV1`.
- PLS-INT-002: The protocol shall declare exactly these calls: `store`,
  `commit`, `discard`, and `read`.
- PLS-INT-003: Protocol replies shall be represented by each call's declared
  return payload.
- PLS-INT-004: Raw log payloads shall be passed and returned as byte sequences
  inside store and read messages.
- PLS-INT-005: `store` shall return the assigned log index in its reply
  payload.
- PLS-INT-006: `Commit` shall accept a log index and return only after
  entries through that index have been synchronized.
- PLS-INT-007: `Discard` shall accept a log index and persist the
  discard watermark before returning.
- PLS-INT-008: `Read` shall return complete non-discarded entries in
  increasing log index order.
- PLS-INT-009: Typed serialization helpers shall accept any payload type that
  satisfies the required serde traits and shall return deserialization errors
  with the log index that failed.
- PLS-INT-010: The typed serialization interface shall use Wincode internally
  and shall not expose caller-selected persistent file formats.
- PLS-INT-011: The file-backed service start function shall return a
  `PersistentLogStorageServiceInstance`.
- PLS-INT-012: `PersistentLogStorageServiceInstance` shall expose the
  `PersistentLogStorageProtocolV1` binding without allowing that binding to
  outlive the service instance.
- PLS-INT-013: The service shall be enabled by the `mpi` facade feature
  `enable-persistent-log-storage-service`.

## Verification

Verification should include:

- inspection that the crate exists in the workspace;
- tests that append assigns increasing indexes;
- tests that reopening the file-backed store reads appended entries;
- tests that `discard` hides discarded entries after reopen;
- tests that `commit` calls the file synchronization path before
  returning success, as far as can be verified in an ordinary unit test;
- tests that a torn trailing record is ignored on recovery;
- tests that message-based store, commit, discard, and read operations preserve
  the existing persistence behavior;
- tests that the named protocol dispatches calls to the appropriate storage
  behavior and returns the corresponding reply payloads;
- tests that the serde helper stores serializable objects with Wincode and
  retrieves deserialized objects;
- inspection that payload bytes are opaque and public API use requires no
  `unsafe` Rust;
- inspection that the file-backed start API returns a service instance and
  exposes no detached protocol binding;
- inspection that the `mpi` facade feature name is
  `enable-persistent-log-storage-service`.

## Validation

### PLS-VAL-001: Recover transaction decision records

Status: proposed

A transaction coordinator appends opaque transaction decision records, commits
through the decision record index, restarts, and reads the records back.

Expected outcome:

- the coordinator receives increasing log indexes;
- `commit` returns only after file synchronization succeeds;
- recovery reads back complete non-discarded records.

Evidence type: test or demonstration

### PLS-VAL-002: Discard completed recovery records

Status: proposed

A transaction coordinator no longer needs records through an index and discards
them.

Expected outcome:

- `discard` persists the discard watermark;
- reopening the store does not return discarded entries.

Evidence type: test

### PLS-VAL-003: Ignore torn trailing record after crash

Status: proposed

A crash or power loss leaves an incomplete final record in the log file.

Expected outcome:

- recovery ignores the torn final record;
- earlier complete records remain readable.

Evidence type: test

### PLS-VAL-004: Use file-backed storage as a service

Status: proposed

A transaction coordinator starts the file-backed persistent log storage service,
uses the protocol binding through the returned service instance, and drops the
final service instance clone.

Expected outcome:

- the protocol binding is accessed through the service instance;
- the binding cannot outlive the service instance;
- dropping the final service instance clone synchronizes service task stop.

Evidence type: test or demonstration

## Traceability

| Requirement | Architecture | Interface | Verification | Validation |
|---|---|---|---|---|
| PLS-REQ-001 | crate structure | crate manifest | inspection | PLS-VAL-001 |
| PLS-REQ-002 | PLS-CMP-001, PLS-CMP-002 | PLS-INT-004, PLS-INT-005 | test | PLS-VAL-001 |
| PLS-REQ-003 | PLS-CMP-001, PLS-CMP-002, PLS-ARCH-003 | PLS-INT-006 | test, inspection | PLS-VAL-001 |
| PLS-REQ-004 | PLS-CMP-001, PLS-CMP-003, PLS-ARCH-004 | PLS-INT-007 | test | PLS-VAL-002 |
| PLS-REQ-005 | PLS-CMP-004 | PLS-INT-008 | test | PLS-VAL-001, PLS-VAL-002 |
| PLS-REQ-006 | PLS-CMP-003, PLS-CMP-004, PLS-ARCH-005 | recovery scanner | test | PLS-VAL-003 |
| PLS-REQ-007 | PLS-ARCH-006 | PLS-INT-004 | inspection | PLS-VAL-001 |
| PLS-REQ-008 | safe Rust implementation | public API | inspection | PLS-VAL-001 |
| PLS-REQ-009 | PLS-CMP-005, PLS-ARCH-009 | PLS-INT-001..PLS-INT-008 | test, inspection | PLS-VAL-001, PLS-VAL-002 |
| PLS-REQ-010 | PLS-CMP-006, PLS-ARCH-007, PLS-ARCH-008 | PLS-INT-009, PLS-INT-010 | test | PLS-VAL-001 |
| PLS-REQ-011 | PLS-CMP-006, PLS-ARCH-008 | PLS-INT-010 | test, inspection | PLS-VAL-001 |
| PLS-REQ-012 | PLS-CMP-007, PLS-ARCH-010, PLS-ARCH-011 | PLS-INT-011, PLS-INT-012 | test, inspection | PLS-VAL-004 |
| PLS-REQ-013 | feature configuration | PLS-INT-013 | inspection | PLS-VAL-004 |
