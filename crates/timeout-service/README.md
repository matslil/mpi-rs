# timeout-service

`timeout-service` represents each timeout as a single-result MPI stream.

Inside task scope, start a timeout with an absolute deadline:

```rust,ignore
let mut timeout = service.timeout(ctx, Time::now() + duration)?;
let occurred = timeout.next(ctx).await?;
assert_eq!(occurred, Some(TimeoutOccurred));
assert_eq!(timeout.next(ctx).await?, None);
```

Drop an unfinished stream to cancel it. Session routing, cancellation,
backpressure, target termination, and stream completion use the ordinary MPI
stream infrastructure. The timeout handler waits with
`ctx.sleep_until(deadline).await`, so concurrent timeout requests do not block
the timeout task thread.

See `examples/schedule_timeout.rs` and `examples/cancel_timeout.rs`.
