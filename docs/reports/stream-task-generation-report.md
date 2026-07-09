# Stream Task Generation Implementation Report

> Superseded status: this is a historical stream-generation slice report. The
> current implementation-readiness summary is
> `docs/reports/final-implementation-readiness.md`, and authoritative current
> status remains in `docs/se-traceability.md` and
> `docs/se-verification-report.md`.

## Summary

This implementation attempt continues from the merged task macro generation slice and implements the next verifiable part of the remaining SE baseline: generated stream request plumbing for external blocking consumers.

The branch adds:

- `BlockingMessageStream<T, E>` for explicit external blocking stream consumption;
- `BoxStreamSink<T, E>` for generated stream handlers;
- `#[stream(item = T, error = E, batch_size = N)]` macro parsing;
- generated stream request message variants;
- generated `*_blocking` stream handle methods;
- generated producer-side `StreamSink` construction;
- generated stream `End` on successful handler return;
- generated stream `Error` on handler error return;
- generated hidden stream cancellation message routing when a blocking stream is dropped unfinished.

## Requirements addressed

This attempt adds implementation and test evidence for:

- REQ-051: generated task artifacts now include stream request plumbing;
- REQ-052: generated dispatch routes stream request variants to stream handlers;
- REQ-100: stream output uses `Batch`, `End`, and `Error` events;
- REQ-101: external blocking stream consumers can receive one item at a time;
- REQ-102: stream batching is generated through producer-side `StreamSink`;
- REQ-103: `BlockingMessageStream` hides batches and drains its local buffer first;
- REQ-104: stream end completes the consumer;
- REQ-105: stream error completes the consumer with the typed error;
- REQ-106: unfinished stream drop routes cancellation through generated stream control;
- REQ-111: generated dispatch sends `End` on success and `Error` on handler error.

## Tests added

`crates/mpi/tests/task_macro.rs` now verifies generated stream behavior:

- generated stream request method;
- generated stream dispatch;
- producer batching hidden from the consumer;
- normal stream end;
- stream error after buffered items are delivered.

## Interface note

Generated stream handlers are implemented with a retained mutable sink argument:

```rust
#[stream(item = Row, error = QueryError, batch_size = 64)]
async fn query(
    &mut self,
    ctx: &mut DatabaseContext,
    out: &mut mpi::BoxStreamSink<Row, QueryError>,
    sql: String,
) -> Result<(), QueryError> {
    // ...
}
```

This differs from the earlier by-value conceptual `StreamSink` parameter. Retaining the sink in generated dispatch allows the macro/runtime to send the required automatic `End` or `Error` event after the handler returns.

## Remaining gaps

The implementation is still not a complete implementation of the full SE baseline.

Remaining gaps include:

- task-internal non-blocking calls that suspend the handler continuation;
- task-local waiter registry;
- out-of-order task-internal response matching;
- task-internal `next(ctx).await` stream consumer integration with task-local receive state;
- credit-based generated stream flow control with producer suspension;
- late one-shot response fallback policy;
- Unix signal bridge;
- diagnostics, tracing, timeouts, and deadlock/debug support;
- compile-fail tests for receive declarations.

## Conclusion

The remaining stream macro generation is partially implementable and now has a working external blocking consumer path. Full task-internal suspended receive and flow-control behavior still requires the task-local runtime/waiter-registry design slice.
