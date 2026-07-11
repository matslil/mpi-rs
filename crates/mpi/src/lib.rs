//! Facade crate for the `mpi-rs` workspace.
//!
//! Core runtime APIs are re-exported from `mpi-core`. Optional repository
//! crates are exposed behind `enable-<crate-name>` Cargo features.

pub use mpi_core::*;

#[cfg(feature = "enable-mpi-transaction")]
pub use mpi_transaction;

#[cfg(feature = "enable-persistent-log-storage-service")]
pub use persistent_log_storage_service;

#[cfg(feature = "enable-timeout-service")]
pub use timeout_service;
