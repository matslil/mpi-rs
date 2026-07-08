# Suspended Handler Request Progress Audit

## Summary

The suspected implementation gap is real.

Generated task dispatch can route replies and stream events to active waiters
while a handler is suspended, but it cannot yet dispatch another ordinary call
or request handler on the same task while the active generated handler waits.

## Affected baseline

- REQ-061: suspended receive
- REQ-062: continued task processing during wait
- REQ-063: waiter matching
- ARCH-040 through ARCH-044: handler execution and selective receive
- INT-018A and INT-045: generated handler lowering and no retained task-context
  borrow while pending
- VAL-004 and VAL-005: task-internal call progress and same-type call matching

## Evidence

The ignored regression test
`req_062_generated_task_receives_call_request_while_handler_is_suspended` in
`crates/mpi/tests/task_macro.rs` creates this sequence:

1. a client handler starts a task-internal call to a delayed counter;
2. the client handler suspends while waiting for the reply;
3. an external caller sends a blocking call request to the same client;
4. the blocking call request does not complete until the delayed counter is
   released and the original handler resumes.

That behavior confirms generated dispatch still defers request handling during
the suspended wait.

## Root cause

The current generated compatibility path normalizes handlers into Rust
`async fn` bodies and adapts the resulting standard future through
`block_on_handler`. A standard future created from a handler borrowing
`&mut self` can retain the task-state borrow across `.await`. While that future
is pending, safe generated code cannot run another handler that also needs
mutable access to the same task state.

The runtime already has `block_on_ctx_task_with_dispatch`, which can dispatch
ordinary messages while a native `CtxFuture` is suspended. Closing the generated
task gap requires generated handler lowering that does not retain mutable task
state or task context while pending.

## Other implementation gaps found

No additional final implementation gap was found in the already-implemented
call/session substrate beyond this generated-dispatch issue.

The remaining known gaps are either partial or later-phase baseline items:

- REQ-061 and REQ-062: generated standard-future handlers still defer ordinary
  request handling while the active handler waits;
- Unix signal bridge validation still needs a Unix-host run of the application
  example before VAL-012 is fully validated;
- diagnostics beyond the current roadmap and snapshots, including timeouts,
  tracing, and deadlock/debug support, remain deferred;
- full separate-binary protocol validation remains pending for proposed
  protocol requirements.

## Recommended next implementation slice

Implement generated handler lowering to native `CtxFuture`-style continuations
or an equivalent context-returning state machine. The lowering must return task
state and task context ownership to the scheduler whenever a handler waits for a
reply or stream event, so the receive loop can dispatch other request messages
before resuming the original handler.
