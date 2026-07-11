# mpi-rs

`mpi-rs` is a Rust message-passing workspace built around typed tasks,
bounded queues, generated task handles, synchronous calls, streaming replies,
and service-style helper crates.

The core abstraction is a **task**: an operating-system thread with an
associated bounded message queue. A task declares the messages it can receive,
and generated handle methods construct and enqueue those messages directly.

The repository is implementation-in-progress. The runtime, macro, timeout,
persistent-log, and transaction-decision-log pieces have usable APIs, while
some larger protocol, transaction, service, and OS-adapter capabilities remain
documented as proposed systems-engineering baseline items.

## Workspace Crates

| Crate | Problem solved | Relationship to the rest of the workspace |
|---|---|---|
| `ctx-future` | Future-like resumable computations that borrow scheduler context only while being resumed. | Standalone helper used by the runtime design; it has no `mpi` dependency. |
| `mpi-core` | Runtime types for task queues, handles, sessions, calls, streams, diagnostics, macro support, and gated transaction identifiers. | Owns the runtime surface re-exported by `mpi`; depends on `ctx-future` and `mpi-macros`. |
| `mpi` | User-facing facade. | Re-exports `mpi-core` and feature-gates optional workspace crates. |
| `mpi-macros` | `#[task]`, handler attributes, and `protocol!` code generation. | Generates code against `::mpi` runtime paths and is re-exported by `mpi-core`/`mpi`. |
| `mpi-os-events` | Bridges native OS or shell events into ordinary `mpi` messages. | Depends on the runtime surface but keeps platform dependencies outside `mpi-core`. |
| `timeout-service` | Local session-associated timeout delivery. | Uses `mpi-core` task handles and protocol bindings; available from `mpi` with `enable-timeout-service`. |
| `persistent-log-storage-service` | Crash-safe file-backed log storage for recovery records. | Serves a message-based storage protocol; available from `mpi` with `enable-persistent-log-storage-service`. |
| `mpi-transaction` | Transaction decision logging backed by the persistent log storage service. | Uses `mpi-core` transaction IDs plus `persistent-log-storage-service`; available from `mpi` with `enable-mpi-transaction`. |

## Feature Overview

The `mpi` facade uses optional feature names of the form
`enable-<crate-name>`:

```toml
[dependencies]
mpi = { path = "crates/mpi", features = [
    "enable-timeout-service",
    "enable-persistent-log-storage-service",
    "enable-mpi-transaction",
] }
```

Those features re-export the optional crates under their Rust crate
identifiers:

```rust
use mpi::timeout_service;
use mpi::persistent_log_storage_service;
use mpi::mpi_transaction;
```

## Learning Path

Start with the facade examples in `crates/mpi/examples/`:

- `ping_pong.rs` for a minimal generated task and event.
- `synchronous_call.rs` for typed request/reply behavior.
- `streaming_query.rs` for stream consumption.
- `priority_shutdown.rs` for priority placement.

Then read crate-specific guides:

- `crates/ctx-future/README.md`
- `crates/mpi-core/README.md`
- `crates/mpi/README.md`
- `crates/mpi-macros/README.md`
- `crates/mpi-os-events/README.md`
- `crates/timeout-service/README.md`
- `crates/persistent-log-storage-service/README.md`
- `crates/mpi-transaction/README.md`

## Error Handling Style

Most user-facing APIs return explicit typed errors. Treat queue-full,
task-stopped, call, storage, and transaction-log errors as normal recoverable
results unless the crate documentation says otherwise. For task handlers, use
context-aware task-internal APIs so the task thread can keep processing other
messages instead of blocking.
