# Verification Report

This report summarizes the current local verification evidence for the
systems-engineering baseline and the implemented runtime subset.

## Summary

The repository now contains working implementation evidence for the early queue,
task, macro, call, late-reply, stream-basics, and `ctx-future` areas. The
implementation is not yet complete against the approved baseline.

Remaining blocking or major gaps are tracked explicitly:

- full task-local scheduling that dispatches ordinary messages while handlers
  are suspended;
- compile-time receive declarations and `CanReceive<T>` enforcement for
  task-internal calls and streams;
- producer-side suspension under stream backpressure;
- Unix signal bridge and diagnostics work, which remain later-phase/deferred
  areas in the baseline.

## Commands run

The following commands were run from the workspace root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
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
| Macro-generated task API | test, inspection | `crates/mpi-macros/src/lib.rs`, `crates/mpi/tests/task_macro.rs` | partial | Generates handles, contexts, dispatch, calls, streams, and blocking APIs; compile-time receive declarations remain missing. |
| Context-returning suspension primitive | test, inspection | `crates/ctx-future/tests/context_borrow.rs`, `crates/ctx-future/README.md` | passed | `ctx-future` is standalone and returns context between pending resumes. |
| Task-local suspended calls | test, inspection | `crates/mpi/src/runtime.rs`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Session-matched replies resume waiters; ordinary messages are still deferred while the active handler waits. |
| Compile-time receive checks | inspection | `crates/mpi/src/message.rs`, `crates/mpi-macros/src/lib.rs` | gap | `CanReceive<T>` exists as a trait, but generated receive declarations and bounds are not implemented. |
| Sessions and calls | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | partial | Session IDs, typed responses, external blocking calls, and late replies are covered; full receive-check coverage is pending. |
| Stream basics | test | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/src/stream.rs` unit tests | partial | Batch hiding, end, error, drop cancellation attempt, generated cancellation routing, producer credit cleanup, explicit stream-flow and stream-cancelled send errors, and late stream replies are covered; producer suspension under backpressure remains incomplete. |
| External blocking APIs | test, inspection | `crates/mpi/tests/task_macro.rs`, generated `_blocking` methods | passed | External APIs are explicit and distinct from context-aware task-internal APIs. |
| Unix signal bridge | inspection | no implementation files | deferred | Later-phase work. |
| Diagnostics | inspection | SE roadmap | deferred | Later-phase work. |

## Failing or Blocked Verification

No local Rust verification command is currently failing.

The following approved requirements remain blocked or incomplete because the
supporting implementation is not present yet:

- REQ-061 and REQ-062 for full task-local scheduling of ordinary messages while
  handlers are suspended;
- REQ-070, REQ-071, and REQ-072 for compile-time receive declaration checks;
- REQ-113 for stream flow-control suspension;
- REQ-130 and REQ-131 for Unix signal support.

## Deferred Verification

The following areas remain later-phase or explicitly incomplete:

- Unix signal bridge;
- diagnostics, timeouts, tracing, and deadlock/debug support;
- full validation examples for the public workflows listed in
  `docs/se/validation-scenarios.md`.

## Human Decisions Needed

- Define the task declaration syntax for response and stream-event receive
  declarations before implementing REQ-070 through REQ-072.
- Decide whether the current stream backpressure behavior should remain an
  explicit error-returning limitation while full producer suspension is pending.
