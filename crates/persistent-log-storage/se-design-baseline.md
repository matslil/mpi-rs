# persistent-log-storage Design Baseline

This document defines the lightweight systems-engineering baseline for the
`persistent-log-storage` crate.

The crate provides a default file-backed persistent log store that transaction
coordination code can use for crash recovery. It is specified as a separate
crate so the storage interface, file format, and crash-safety rules can evolve
without being folded into the core `mpi` crate.

## Purpose

`persistent-log-storage` stores ordered log entries, durably commits log entries
through a caller-specified index, durably discards log entries through a
caller-specified index, and reads back durable log entries after restart.

The intended first consumer is transactional messaging recovery. A transaction
coordinator can append transaction decision records, commit through those
records, and recover committed records after a crash.

## Scope

`persistent-log-storage` is responsible for:

- defining persistent log storage requirements, architecture, and interfaces;
- providing a default file-backed implementation;
- appending ordered binary log entries;
- committing log entries through a specific index by waiting until the backing
  file has been synchronized to persistent storage;
- discarding log entries through a specific index by durably recording a discard
  watermark;
- reading back complete non-discarded log entries after restart;
- tolerating torn trailing records caused by crash or power loss.

`persistent-log-storage` is not responsible for:

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

## Requirements

### PLS-REQ-001: Separate crate

The persistent log storage implementation shall be provided by a separate
workspace crate named `persistent-log-storage`.

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

## Architecture

| ID | Component | Responsibility |
|---|---|---|
| PLS-CMP-001 | Persistent log trait | Defines append, commit, discard, and read-back behavior. |
| PLS-CMP-002 | File log storage | Provides the default crash-safe file-backed implementation. |
| PLS-CMP-003 | Log record codec | Encodes and validates append and discard records. |
| PLS-CMP-004 | Recovery scanner | Reads complete records, applies the latest discard watermark, and ignores torn trailing records. |

Architecture rules:

- PLS-ARCH-001: Log entry indexes are monotonically increasing `u64` values.
- PLS-ARCH-002: A file log uses append records for payloads and discard records
  for discard watermarks.
- PLS-ARCH-003: `commit_through(index)` synchronizes the backing file before
  returning success.
- PLS-ARCH-004: `discard_through(index)` appends a discard watermark and
  synchronizes the backing file before returning success.
- PLS-ARCH-005: Recovery scanning stops before a torn trailing record and does
  not return that record as a valid entry.
- PLS-ARCH-006: Log payload bytes remain opaque to this crate.

## Interface

Conceptual interface:

```rust
pub type LogIndex = u64;

pub struct LogEntry {
    pub index: LogIndex,
    pub payload: Vec<u8>,
}

pub trait PersistentLogStorage {
    fn append(&mut self, payload: &[u8]) -> Result<LogIndex, LogStorageError>;
    fn commit_through(&mut self, index: LogIndex) -> Result<(), LogStorageError>;
    fn discard_through(&mut self, index: LogIndex) -> Result<(), LogStorageError>;
    fn read_entries(&self) -> Result<Vec<LogEntry>, LogStorageError>;
}
```

Default file-backed implementation:

```rust
pub struct FileLogStorage;

impl FileLogStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LogStorageError>;
}
```

Interface rules:

- PLS-INT-001: Log payloads shall be passed and returned as byte sequences.
- PLS-INT-002: `append` shall return the assigned log index.
- PLS-INT-003: `commit_through` shall accept a log index and return only after
  entries through that index have been synchronized.
- PLS-INT-004: `discard_through` shall accept a log index and persist the
  discard watermark before returning.
- PLS-INT-005: `read_entries` shall return complete non-discarded entries in
  increasing log index order.

## Verification

Verification should include:

- inspection that the crate exists in the workspace;
- tests that append assigns increasing indexes;
- tests that reopening the file-backed store reads appended entries;
- tests that `discard_through` hides discarded entries after reopen;
- tests that `commit_through` calls the file synchronization path before
  returning success, as far as can be verified in an ordinary unit test;
- tests that a torn trailing record is ignored on recovery;
- inspection that payload bytes are opaque and public API use requires no
  `unsafe` Rust.

## Validation

### PLS-VAL-001: Recover transaction decision records

Status: proposed

A transaction coordinator appends opaque transaction decision records, commits
through the decision record index, restarts, and reads the records back.

Expected outcome:

- the coordinator receives increasing log indexes;
- `commit_through` returns only after file synchronization succeeds;
- recovery reads back complete non-discarded records.

Evidence type: test or demonstration

### PLS-VAL-002: Discard completed recovery records

Status: proposed

A transaction coordinator no longer needs records through an index and discards
them.

Expected outcome:

- `discard_through` persists the discard watermark;
- reopening the store does not return discarded entries.

Evidence type: test

### PLS-VAL-003: Ignore torn trailing record after crash

Status: proposed

A crash or power loss leaves an incomplete final record in the log file.

Expected outcome:

- recovery ignores the torn final record;
- earlier complete records remain readable.

Evidence type: test

## Traceability

| Requirement | Architecture | Interface | Verification | Validation |
|---|---|---|---|---|
| PLS-REQ-001 | crate structure | crate manifest | inspection | PLS-VAL-001 |
| PLS-REQ-002 | PLS-CMP-001, PLS-CMP-002 | PLS-INT-001, PLS-INT-002 | test | PLS-VAL-001 |
| PLS-REQ-003 | PLS-CMP-001, PLS-CMP-002, PLS-ARCH-003 | PLS-INT-003 | test, inspection | PLS-VAL-001 |
| PLS-REQ-004 | PLS-CMP-001, PLS-CMP-003, PLS-ARCH-004 | PLS-INT-004 | test | PLS-VAL-002 |
| PLS-REQ-005 | PLS-CMP-004 | PLS-INT-005 | test | PLS-VAL-001, PLS-VAL-002 |
| PLS-REQ-006 | PLS-CMP-003, PLS-CMP-004, PLS-ARCH-005 | recovery scanner | test | PLS-VAL-003 |
| PLS-REQ-007 | PLS-ARCH-006 | PLS-INT-001 | inspection | PLS-VAL-001 |
| PLS-REQ-008 | safe Rust implementation | public API | inspection | PLS-VAL-001 |
