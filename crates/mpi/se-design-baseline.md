# mpi Facade Design Baseline

This document defines the crate-local systems-engineering baseline for the
`mpi` facade crate.

## Purpose

`mpi` is the user-facing facade for the `mpi-rs` workspace. It re-exports the
runtime and macro surface from `mpi-core` without renaming public items, and it
owns repository feature aggregation for optional workspace crates and services.

## Scope

`mpi` is responsible for:

- re-exporting `mpi-core` public APIs so existing `mpi::...` paths remain valid;
- exposing optional workspace crates behind Cargo features named
  `enable-<crate-name>`;
- re-exporting optional crates under the same Rust crate identifier users would
  get from the original package name, such as `mpi_transaction`,
  `timeout_service`, and `persistent_log_storage_service`;
- enabling corresponding `mpi-core` functionality when a facade feature requires
  core runtime support.

`mpi` is not responsible for:

- implementing runtime queues, sessions, tasks, calls, streams, transactions, or
  service lifecycle primitives;
- implementing proc-macro parsing or code generation;
- implementing service task state or service-specific protocols.

## Requirements

### MPI-FACADE-REQ-001: Core re-export

The facade shall re-export the public `mpi-core` API without changing item
names.

Verification: inspection and test

Status: proposed

### MPI-FACADE-REQ-002: Optional crate feature names

The facade shall expose repository-owned optional crates and services through
Cargo features named `enable-<crate-name>`.

Verification: inspection

Status: proposed

### MPI-FACADE-REQ-003: Optional crate re-export names

When the facade re-exports an optional crate, the re-exported Rust identifier
shall be the same identifier produced by that crate's Cargo package name.

Verification: inspection

Status: proposed

### MPI-FACADE-REQ-004: Core feature forwarding

When a facade feature enables behavior that requires gated `mpi-core` runtime
support, the facade shall enable the corresponding `mpi-core` feature.

Verification: inspection

Status: proposed

## Architecture

| ID | Component | Responsibility |
|---|---|---|
| MPI-FACADE-CMP-001 | Facade re-export | Makes `mpi-core` APIs available through `mpi::...`. |
| MPI-FACADE-CMP-002 | Feature aggregation | Maps `enable-<crate-name>` features to optional workspace crates and required `mpi-core` features. |

Architecture rules:

- MPI-FACADE-ARCH-001: The runtime owner is `mpi-core`; `mpi` is a facade.
- MPI-FACADE-ARCH-002: The facade may depend on optional workspace crates, but
  those crates shall depend on `mpi-core` rather than the facade when they need
  runtime APIs.
- MPI-FACADE-ARCH-003: Re-export names shall not introduce shorter aliases that
  hide the original crate identifier.

## Interface

The facade exposes:

```rust
pub use mpi_core::*;
```

Optional crate re-exports are feature-gated:

```rust
pub use mpi_transaction;
pub use persistent_log_storage_service;
pub use timeout_service;
```

Interface rules:

- MPI-FACADE-INT-001: Existing user code using `mpi::TaskHandle`,
  `mpi::task`, or other `mpi-core` exports should not need an import rename.
- MPI-FACADE-INT-002: The transaction feature is named
  `enable-mpi-transaction` and forwards to `mpi-core/enable-mpi-transaction`.
- MPI-FACADE-INT-003: The timeout service feature is named
  `enable-timeout-service`.
- MPI-FACADE-INT-004: The persistent log storage service feature is named
  `enable-persistent-log-storage-service`.

## Validation Scenarios

| ID | Scenario | Status |
|---|---|---|
| MPI-FACADE-VAL-001 | A user imports task/runtime APIs from `mpi` and runs existing task macro tests. | proposed |
| MPI-FACADE-VAL-002 | A user enables an optional crate feature and imports the optional crate by its original Rust crate identifier under `mpi`. | proposed |

## Verification

Verification should include:

- inspection of facade Cargo features and re-exports;
- user-facing integration tests through the `mpi` crate;
- cargo checks that optional crate dependencies do not create dependency cycles.

## Traceability

| Requirement | Architecture | Interface | Validation |
|---|---|---|---|
| MPI-FACADE-REQ-001 | MPI-FACADE-ARCH-001 | MPI-FACADE-INT-001 | MPI-FACADE-VAL-001 |
| MPI-FACADE-REQ-002 | MPI-FACADE-CMP-002 | MPI-FACADE-INT-002..MPI-FACADE-INT-004 | MPI-FACADE-VAL-002 |
| MPI-FACADE-REQ-003 | MPI-FACADE-ARCH-003 | optional crate re-exports | MPI-FACADE-VAL-002 |
| MPI-FACADE-REQ-004 | MPI-FACADE-CMP-002 | MPI-FACADE-INT-002 | MPI-FACADE-VAL-002 |
