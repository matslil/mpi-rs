# Unix Signal Bridge Inspection

## Summary

`mpi-rs` now provides Unix-only signal forwarding through
`mpi::forward_signals`. The API registers POSIX signals, observes them through a
background bridge thread, and enqueues normal task messages from that ordinary
Rust thread.

## Requirements

- REQ-130: async-signal-safe signal handling.
- REQ-131: signal bridge.

## Architecture and Interface

- ARCH-090..ARCH-092: signal handlers do not allocate or construct Rust task
  messages; bridge code performs normal Rust work outside the handler context.
- INT-110..INT-111: signal forwarding is exposed through a bridge and the POSIX
  handler portion remains constrained.

## Implementation Evidence

- `crates/mpi/src/signal.rs` implements `forward_signals`, `SignalBridge`, and
  `SignalBridgeError` behind `cfg(unix)`.
- `crates/mpi/src/lib.rs` exports the signal API only on Unix targets.
- The bridge uses `signal-hook`'s `Signals` iterator. The dependency is used
  because Unix signal registration requires platform-specific async-signal-safe
  handler machinery, and the repository rules prohibit adding project-local
  unsafe Rust without an approved architecture decision.

## Verification Evidence

- Inspection: message construction and `TaskHandle::send_message` calls happen
  inside the spawned bridge thread, not in the POSIX signal handler.
- Unix-only test: `req_130_req_131_signal_bridge_forwards_signal_outside_handler`
  raises `SIGUSR1` and observes the generated task message in the target queue.
- Local command evidence: `cargo check -p mpi --target
  x86_64-unknown-linux-gnu` passed, confirming the Unix-only API type-checks
  from this Windows development host.

## Platform Note

The public module is compiled only for `cfg(unix)`. Windows builds keep the
existing API surface unchanged.
