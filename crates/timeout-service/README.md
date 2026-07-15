# timeout-service

`timeout-service` provides a local `mpi`-based timeout service. It schedules a
session-associated timeout event for an absolute monotonic deadline and
supports best-effort cancellation by `SessionId`.

## What It Solves

Many message-passing workflows need a timeout tied to a call, stream, or other
logical session. This crate gives those workflows one reusable local timer task
instead of having each application task build its own timer thread and
cancellation race handling.

## How It Interacts With Other Crates

- Uses `mpi-core` task handles, queues, session IDs, and protocol generation.
- Can be imported directly as `timeout_service`.
- Can be re-exported from the `mpi` facade with `enable-timeout-service`.
- Emits a payload-free timeout-occurred event containing only the request's
  `SessionId`. The infrastructure derives its return endpoint from the
  requesting task scope; callers do not supply a delivery closure.

## Usage

Examples are in `examples/`:

- [schedule_timeout.rs](examples/schedule_timeout.rs): start the service,
  schedule a timeout event, and receive the payload-free timeout occurrence.
- [cancel_timeout.rs](examples/cancel_timeout.rs): cancel before expiry and
  handle the no-delivery case.

## Best Practices

Build deadlines from `Time::now()` plus a `Duration` so requesters and the
service use the same monotonic time basis.

Construct requests inside a task with `TimeoutRequest::new(ctx, session_id,
deadline)`, declare the timeout handler with `#[event(receive)]`, and send the
request through `service.protocol().request(ctx, request)`.

Use one active timeout per `SessionId`. A duplicate active request is a service
error because it makes cancellation ambiguous.

Treat cancellation as best-effort. If a timeout expires while cancellation is
in flight, the timeout-occurred event may still be delivered. Receivers that
expect this race should discard events for unknown `SessionId` values.

Handle `TimeoutServiceError` and `SendError` explicitly. Queue-full delivery is
retried for the configured local delivery timeout; a stopped delivery target is
reported as an error.
