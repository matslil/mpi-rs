# mpi

`mpi` is the user-facing facade for the workspace. Use it when writing an
application or example: it re-exports the runtime and macro surface from
`mpi-core`, and it provides feature flags for optional workspace crates.

## What It Solves

Without the facade, users would need to know whether each public item lives in
`mpi-core`, `mpi-macros`, `timeout-service`, `persistent-log-storage-service`,
or `mpi-transaction`. The facade keeps ordinary application code centered on
`mpi::...` imports.

## How It Interacts With Other Crates

- `pub use mpi_core::*` makes runtime types and macros available through
  `mpi`.
- `enable-timeout-service` re-exports `timeout_service`.
- `enable-persistent-log-storage-service` re-exports
  `persistent_log_storage_service`.
- `enable-mpi-transaction` re-exports `mpi_transaction` and forwards the
  required `mpi-core/enable-mpi-transaction` runtime feature.

## Usage

Start with the basic task examples:

- [ping_pong.rs](examples/ping_pong.rs): minimal generated task and event send.
- [synchronous_call.rs](examples/synchronous_call.rs): blocking external call
  and simple `Result` handling.
- [streaming_query.rs](examples/streaming_query.rs): streaming call and item
  iteration.
- [priority_shutdown.rs](examples/priority_shutdown.rs): priority messages for
  shutdown-style behavior.

For optional services, enable the facade feature and import the re-export:

```toml
[dependencies]
mpi = { path = "../mpi", features = ["enable-timeout-service"] }
```

```rust
use mpi::timeout_service::{Time, TimeoutCancel, TimeoutRequest};
```

## Best Practices

Use `mpi` imports in application code and examples. Use `mpi-core` directly
only when developing workspace internals or a crate that must avoid depending
on the facade.

Keep task-internal and external APIs visually distinct. External code should
use explicit blocking methods such as `get_blocking(...)`. Task handlers should
use context-aware async methods such as `get(ctx, ...).await` so the task thread
does not block.

Handle `SendError`, `CallError`, and crate-specific service errors explicitly.
Queue-full and task-stopped outcomes are part of normal message-passing
backpressure and lifecycle behavior.
