//! Operating-system and application-shell event bridges for `mpi-rs`.
//!
//! This crate hosts optional integration code that observes native operating
//! system or framework events and translates them into ordinary `mpi` messages.
//! The core `mpi` crate remains focused on task, queue, call, and stream
//! primitives.

#[cfg(all(unix, feature = "unix-signals"))]
pub mod signal;

#[cfg(all(unix, feature = "unix-signals"))]
pub use signal::{SignalBridge, SignalBridgeError, forward_signals};
