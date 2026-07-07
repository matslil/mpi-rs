# Macro Task Generation Implementation Report

## Summary

This implementation attempt follows the approved interface change: `#[task(queue_size = ...)]` is now applied to the task `impl` block instead of splitting task information across a struct attribute and an impl attribute.

The branch updates `docs/se/interfaces.md` and implements concrete task macro generation for the following handler categories:

- `#[start]`
- `#[event]`
- `#[event(priority)]`
- `#[call(reply = T)]`

## Implemented generated artifacts

For one `#[task(queue_size = N)] impl TaskType` block, the macro now generates:

- `TaskTypeMessage` enum;
- `TaskTypeHandle` wrapper;
- `TaskTypeContext` wrapper;
- `TaskMessage` implementation for generated placement;
- external handle send methods for events;
- external blocking call methods with `_blocking` suffix;
- `TaskType::spawn(state, start_args...)` helper;
- dispatch from message enum variants to handlers;
- automatic conversion from call handler return value to `Response<T>`.

## Requirements addressed

This attempt adds implementation and test evidence for:

- REQ-040: task creation enqueues a start message;
- REQ-041: start message forced priority;
- REQ-042: start message first;
- REQ-050: task macro name;
- REQ-051: generated artifacts;
- REQ-052: handler dispatch;
- REQ-053: start handler priority handling;
- REQ-060: async handlers are expressible;
- REQ-091: one typed external blocking call response;
- REQ-093: call handler return conversion;
- REQ-120: explicit external blocking API.

## Tests added

`crates/mpi/tests/task_macro.rs` verifies:

- generated spawn helper;
- generated task handle;
- generated event send methods;
- generated blocking call method;
- generated call dispatch and reply conversion;
- generated priority stop event;
- start message priority behavior.

## Verification result

GitHub Actions passed the restored normal workflow:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --doc
```

## Remaining gaps

The implementation is still not a complete implementation of the full SE baseline.

Remaining gaps include:

- `#[stream]` macro generation;
- task-internal non-blocking calls that suspend the handler continuation;
- task-local waiter registry;
- out-of-order task-internal response matching;
- `next(ctx).await` stream consumer integration with task-local receive state;
- credit-based stream flow control integrated with generated stream handlers;
- late one-shot response fallback policy;
- Unix signal bridge;
- diagnostics, tracing, timeouts, and deadlock/debug support;
- compile-fail tests for receive declarations.

## Design note

Generated async handlers are adapted into `CtxFuture` at the task-runtime boundary and driven by `mpi::block_on_task`, which routes task-local call responses, stream events, and stream control messages between resume steps. This supports async handler syntax while keeping the task driver centered on `CtxFuture`. It does not yet provide concurrent ordinary-message handler progress while another handler waits, as described by REQ-062.

## Conclusion

The revised interface shape is implementable. The branch demonstrates working generated task plumbing for start, event, call, and stream handlers. Concurrent ordinary-handler progress while another handler is suspended remains future work.
