# persistent-log-storage-service

`persistent-log-storage-service` is a crash-safe local file-backed log service.
It appends opaque byte entries, commits entries through a selected index,
discards entries through a selected index, and reads complete non-discarded
entries after restart.

## What It Solves

Transaction recovery needs a durable decision boundary: once a commit or abort
decision is recorded and synchronized, restart recovery can continue that
decision instead of guessing. This crate provides the default local storage
service for those records.

## How It Interacts With Other Crates

- Uses `mpi-core` task and protocol support.
- Exposes `PersistentLogStorageProtocolV1` through an owning
  `PersistentLogStorageServiceInstance`.
- Is consumed by `mpi-transaction` for decision logging.
- Can be re-exported from the `mpi` facade with
  `enable-persistent-log-storage-service`.

## Usage

Examples are in `examples/`:

- [store_commit_read.rs](examples/store_commit_read.rs): store bytes, commit
  through the returned index, and read after restart.
- [discard_entries.rs](examples/discard_entries.rs): discard completed records.
- [typed_payload.rs](examples/typed_payload.rs): use serde-backed Wincode
  helpers for typed records.

## Best Practices

Store application records as opaque bytes unless the serde helper is a good
fit. The service does not interpret transaction payloads.

Call `commit_blocking(index)` after storing records that must survive restart.
The commit operation is the persistence boundary.

Use `discard_blocking(index)` only for entries no longer needed for recovery.
Discard is durable, so design recovery logic before compacting records.

Handle both layers of results. A blocking call can fail at the `mpi` call layer,
and the service operation can return a storage error string:

```rust
let index = service.store_blocking(bytes)??;
service.commit_blocking(index)??;
```
