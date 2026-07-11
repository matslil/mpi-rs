# mpi-os-events

`mpi-os-events` contains bridges from operating-system or application-shell
events into ordinary `mpi` messages. The crate keeps platform integration
dependencies outside the core runtime.

## What It Solves

Native event sources often have strict callback rules. POSIX signal handlers,
for example, cannot safely allocate or construct arbitrary Rust values. This
crate splits native event capture from ordinary Rust message sending so user
tasks can handle platform events as typed `mpi` messages.

## How It Interacts With Other Crates

- Depends on the runtime surface from `mpi-core`.
- Sends translated events through task handles or equivalent generated
  protocol bindings.
- Keeps OS or framework dependencies out of `mpi-core` and the `mpi` facade
  unless users opt into this crate.

## Usage

Unix signal forwarding is available on Unix targets when the default
`unix-signals` feature is enabled:

- [examples/unix_signal_bridge.rs](examples/unix_signal_bridge.rs)

Disable default features to remove the Unix signal API:

```toml
[dependencies]
mpi-os-events = { path = "../mpi-os-events", default-features = false }
```

## Best Practices

Keep platform callbacks small. Do native capture only in the platform-required
context, then construct typed Rust messages from a normal Rust thread or
framework callback where allocation and ordinary error handling are allowed.

Map asynchronous source events to event-style messages when no reply is needed.
Use call-style interactions only when the OS or framework expects a decision.

Handle send failures from the target task. `QueueFull` means the target is
backpressured; `TaskStopped` means the target is no longer available.
