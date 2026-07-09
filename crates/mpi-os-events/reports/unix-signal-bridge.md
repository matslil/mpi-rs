# Unix Signal Bridge Inspection

## Summary

`mpi-rs` now provides Unix-only signal forwarding through
`mpi_os_events::forward_signals` when the default-enabled `unix-signals` feature is
active. The API registers POSIX signals, observes them through a background
bridge thread, and enqueues normal task messages from that ordinary Rust thread.

## Requirements

- REQ-130: async-signal-safe signal handling.
- REQ-131: signal bridge.
- REQ-132: optional default Unix signal feature.

## Architecture and Interface

- ARCH-090..ARCH-093: signal handlers do not allocate or construct Rust task
  messages; bridge code performs normal Rust work outside the handler context.
- INT-110..INT-112: signal forwarding is exposed through a bridge and the POSIX
  handler portion remains constrained.

## Implementation Evidence

- `crates/mpi-os-events/src/signal.rs` implements `forward_signals`, `SignalBridge`, and
  `SignalBridgeError` behind `cfg(all(unix, feature = "unix-signals"))`.
- `crates/mpi-os-events/src/lib.rs` exports the signal API only on Unix targets when the
  `unix-signals` feature is active.
- `crates/mpi-os-events/examples/unix_signal_bridge.rs` demonstrates forwarding a Unix
  signal into a generated task event.
- The bridge uses `signal-hook`'s `Signals` iterator. The dependency is used
  because Unix signal registration requires platform-specific async-signal-safe
  handler machinery, and the repository rules prohibit adding project-local
  unsafe Rust without an approved architecture decision. The dependency is
  optional and included by the crate's default feature set.

## Verification Evidence

- Inspection: message construction and `TaskHandle::send_message` calls happen
  inside the spawned bridge thread, not in the POSIX signal handler.
- Unix-only test: `req_130_req_131_signal_bridge_forwards_signal_outside_handler`
  raises `SIGUSR1` and observes the generated task message in the target queue.
- Feature opt-out evidence should include `cargo check -p mpi-os-events
  --no-default-features`.
- Unix-host validation should include running `cargo run -p mpi-os-events --example
  unix_signal_bridge`.

## Platform Note

The public module is compiled only for `cfg(all(unix, feature =
"unix-signals"))`. Windows builds keep the existing API surface unchanged, and
Unix applications can opt out of the `mpi-os-events` signal bridge dependency with
`default-features = false`.
