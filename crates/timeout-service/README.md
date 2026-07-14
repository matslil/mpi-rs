# timeout-service

`timeout-service` provides a local `mpi`-based timeout service. It suspends a
call until an absolute monotonic deadline and supports best-effort cancellation
by `SessionId`.

## What It Solves

Many message-passing workflows need a timeout tied to a call, stream, or other
logical session. This crate gives those workflows one reusable local timer task
instead of having each application task build its own timer thread and
cancellation race handling.

## How It Interacts With Other Crates

- Uses `mpi-core` task handles, queues, session IDs, and protocol generation.
- Can be imported directly as `timeout_service`.
- Can be re-exported from the `mpi` facade with `enable-timeout-service`.
- Uses the `mpi` call `SessionId` to correlate each timeout request with its
  payload-free `()` reply.

## Usage

Examples are in `examples/`:

- [schedule_timeout.rs](examples/schedule_timeout.rs): start the service,
  schedule a timeout, and wait for its payload-free reply.
- [cancel_timeout.rs](examples/cancel_timeout.rs): cancel before expiry and
  handle the no-delivery case.

## Best Practices

Build deadlines from `Time::now()` plus a `Duration` so requesters and the
service use the same monotonic time basis.

Use one active timeout per `SessionId`. A duplicate active request is a service
error because it makes cancellation ambiguous.

Treat cancellation as best-effort. Dropping a suspended timeout call submits a
cancel using the request's generated `SessionId`. If expiry wins the race, the
normal `mpi` late-reply policy applies.

Handle `TimeoutServiceError`, `CallError`, and `SendError` explicitly. A stopped
requester is reported as a service error if expiry reply delivery has begun.
