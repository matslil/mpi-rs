# Verification Report

This report summarizes the current local verification evidence for the
systems-engineering baseline and the implemented runtime subset.

## Summary

The repository now contains working implementation evidence for the early queue,
task, macro, call, late-reply, stream-basics, and `ctx-future` areas. The
implementation is not yet complete against the approved baseline.

Remaining partial or later-slice gaps are tracked explicitly:

- generated handler lowering for arbitrary body shapes beyond direct
  awaited-assignment event handlers;
- timeout APIs, tracing, and deadlock/debug support, which remain
  later-phase/deferred areas in the baseline.

## Commands run

The following commands were run from the workspace root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check -p mpi --no-default-features
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
| Macro-generated task API | test, inspection | `crates/mpi-macros/src/lib.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs` | partial | Generates handles, contexts, dispatch, calls, streams, blocking APIs, protocol bindings, and compile-time receive declarations; direct awaited-assignment event handlers can dispatch ordinary messages while suspended, while arbitrary handler body lowering remains incomplete. |
| Context-returning suspension primitive | test, inspection | `crates/ctx-future/tests/context_borrow.rs`, `crates/ctx-future/README.md` | passed | `ctx-future` is standalone and returns context between pending resumes. |
| Task-local suspended calls | test, inspection | `crates/mpi/src/runtime.rs`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Session-matched replies resume waiters, including out-of-order same-type responses; `block_on_ctx_task_with_dispatch` can dispatch ordinary messages while a native `CtxFuture` is suspended; `req_062_generated_task_receives_call_request_while_handler_is_suspended` verifies generated dispatch for the direct awaited-assignment lowered shape. |
| Compile-time receive checks | test, inspection | `crates/mpi/src/message.rs`, `crates/mpi-macros/src/lib.rs`, `crates/mpi/tests/scope_compile_fail.rs`, `crates/mpi/tests/task_macro.rs` | passed | Generated `receives(...)` declarations implement `CanReceive<T>` for declared raw response and stream event types and for protocol-qualified reply/event wrapper types. Compile-fail tests cover missing non-protocol call and stream receive declarations, missing protocol receive declarations, and wrong protocol identity. |
| Sessions and calls | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs` | partial | Session IDs, typed responses, external blocking calls, out-of-order response matching, late replies, and receive-check enforcement are covered; task-local ordinary-message scheduling while suspended remains incomplete. |
| Stream basics | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/src/stream.rs` unit tests | passed | Batch hiding, end, error, drop cancellation attempt, generated cancellation routing, producer credit cleanup, explicit stream-flow and stream-cancelled send errors, late stream replies, ordinary-message non-discard, mapped credit enforcement, and REQ-115 no-credit `yield_item()`/`yield_batch()` suspension are covered. |
| External blocking APIs | test, inspection | `crates/mpi/tests/task_macro.rs`, generated `_blocking` methods | passed | External APIs are explicit and distinct from context-aware task-internal APIs. |
| Unix signal bridge | inspection, Unix-only test, feature check | `crates/mpi/src/signal.rs`, `crates/mpi/examples/unix_signal_bridge.rs`, `docs/reports/unix-signal-bridge.md` | partial | `forward_signals` delegates handler registration to `signal-hook` and constructs task messages on an ordinary bridge thread. The API is behind the default-enabled `unix-signals` feature. The mapped unit test and real example execution require a Unix host. |
| Diagnostics | inspection, test | `docs/reports/diagnostics-roadmap.md`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Roadmap plus read-only queue and task-context snapshots exist; timeout APIs, tracing, richer session lifecycle diagnostics, and deadlock/debug support remain deferred. |

## Failing or Blocked Verification

No default local Rust verification command is currently failing.

The following approved requirements remain intentionally partial because the
supporting implementation scope is not complete yet:

- REQ-061 and REQ-062 for full macro-generated task-local scheduling of
  ordinary request messages across arbitrary handler body shapes.

The generated-dispatch gap test for direct awaited-assignment handlers is no
longer ignored:

```sh
cargo test -p mpi --test task_macro req_062_generated_task_receives_call_request_while_handler_is_suspended
```

## Deferred Verification

The following areas remain later-phase or explicitly incomplete:

- Unix-host validation of the Unix signal bridge through the application
  example;
- timeout APIs, tracing, richer session lifecycle diagnostics, and deadlock/debug support;
- full validation examples for the public workflows listed in
  `docs/se/validation-scenarios.md`.

## Human Decisions Needed

- Decide whether lowering additional generated handler body shapes should be the
  next implementation slice.
