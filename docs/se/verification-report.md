# Verification Report

This report summarizes the current local verification evidence for the
systems-engineering baseline and the implemented runtime subset.

## Summary

The repository now contains working implementation evidence for the early queue,
task, macro, call, late-reply, stream-basics, and `ctx-future` areas. The
implementation is not yet complete against the approved baseline.

Remaining blocking or major gaps are tracked explicitly:

- full macro-generated task-local scheduling that dispatches ordinary request
  messages while normalized handlers are suspended;
- producer-side suspension under stream backpressure;
- timeout APIs, tracing, and deadlock/debug support, which remain
  later-phase/deferred areas in the baseline.

## Commands run

The following commands were run from the workspace root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
cargo check -p mpi --target x86_64-unknown-linux-gnu
```

Result: all commands passed.

Note: an initial sandboxed `cargo test` run failed because temporary
compile-fail fixtures tried to access the crates.io index. The compile-fail
fixture harness now copies the workspace lockfile and runs `cargo check
--offline` so those tests use already-resolved workspace dependency versions
without contacting the network.

## Requirement Verification Status

| Requirement area | Method | Evidence | Result | Notes |
|---|---|---|---|---|
| Process source-of-truth handling | inspection | `AGENTS.md`, `docs/agents/process.md`, `docs/se/*` | partial | Source-of-truth process exists; human approval remains required. |
| Queue behavior | test | `crates/mpi/tests/runtime_baseline.rs` | passed | Covers static/shared capacity, queue-full error, normal FIFO, priority FIFO, and priority-before-normal. |
| Start message behavior | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | passed | Covers start enqueue/priority/first behavior and macro-forced priority. |
| Macro-generated task API | test, inspection | `crates/mpi-macros/src/lib.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs` | partial | Generates handles, contexts, dispatch, calls, streams, blocking APIs, protocol bindings, and compile-time receive declarations; ordinary-message scheduling while handlers are suspended remains incomplete. |
| Context-returning suspension primitive | test, inspection | `crates/ctx-future/tests/context_borrow.rs`, `crates/ctx-future/README.md` | passed | `ctx-future` is standalone and returns context between pending resumes. |
| Task-local suspended calls | test, inspection | `crates/mpi/src/runtime.rs`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Session-matched replies resume waiters, including out-of-order same-type responses; `block_on_ctx_task_with_dispatch` can dispatch ordinary messages while a native `CtxFuture` is suspended; ignored test `req_062_generated_task_receives_call_request_while_handler_is_suspended` confirms generated dispatch still defers a call request while the active handler waits. |
| Compile-time receive checks | test, inspection | `crates/mpi/src/message.rs`, `crates/mpi-macros/src/lib.rs`, `crates/mpi/tests/scope_compile_fail.rs`, `crates/mpi/tests/task_macro.rs` | passed | Generated `receives(...)` declarations implement `CanReceive<T>` for declared raw response and stream event types and for protocol-qualified reply/event wrapper types. Compile-fail tests cover missing non-protocol call and stream receive declarations, missing protocol receive declarations, and wrong protocol identity. |
| Sessions and calls | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs` | partial | Session IDs, typed responses, external blocking calls, out-of-order response matching, late replies, and receive-check enforcement are covered; task-local ordinary-message scheduling while suspended remains incomplete. |
| Stream basics | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/src/stream.rs` unit tests | partial | Batch hiding, end, error, drop cancellation attempt, generated cancellation routing, producer credit cleanup, explicit stream-flow and stream-cancelled send errors, late stream replies, ordinary-message non-discard, and mapped credit enforcement are covered; producer suspension under backpressure remains incomplete. |
| External blocking APIs | test, inspection | `crates/mpi/tests/task_macro.rs`, generated `_blocking` methods | passed | External APIs are explicit and distinct from context-aware task-internal APIs. |
| Unix signal bridge | inspection, Unix-only test | `crates/mpi/src/signal.rs`, `docs/reports/unix-signal-bridge.md` | passed by inspection | `forward_signals` delegates handler registration to `signal-hook` and constructs task messages on an ordinary bridge thread. The mapped unit test is compiled on Unix targets. |
| Diagnostics | inspection, test | `docs/reports/diagnostics-roadmap.md`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Roadmap plus read-only queue and task-context snapshots exist; timeout APIs, tracing, richer session lifecycle diagnostics, and deadlock/debug support remain deferred. |

## Failing or Blocked Verification

No default local Rust verification command is currently failing.

The following approved requirements remain intentionally partial because the
supporting implementation scope is not complete yet:

- REQ-061 and REQ-062 for full macro-generated task-local scheduling of
  ordinary request messages while normalized handlers are suspended;
- REQ-113 for stream flow-control suspension;

The ignored gap test below is expected to fail until generated handler lowering
can release task state while the handler is suspended:

```sh
cargo test -p mpi --test task_macro req_062_generated_task_receives_call_request_while_handler_is_suspended -- --ignored
```

## Deferred Verification

The following areas remain later-phase or explicitly incomplete:

- cross-platform validation of the Unix signal bridge through an application
  example;
- timeout APIs, tracing, richer session lifecycle diagnostics, and deadlock/debug support;
- full validation examples for the public workflows listed in
  `docs/se/validation-scenarios.md`.

## Human Decisions Needed

- Decide whether the current stream backpressure behavior should remain an
  explicit error-returning limitation while full producer suspension is pending.
