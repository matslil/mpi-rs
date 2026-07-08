# Final Implementation Readiness Report

## Summary

This report records the final implementation-readiness pass for the current
approved `mpi-rs` baseline after the July 8, 2026 main-branch sync.

The implementation now has local verification evidence for:

- bounded normal/priority task queues;
- generated task handles, contexts, message enums, placement, spawn helpers, and
  dispatch;
- start-message priority and first-message behavior;
- task-local session allocation, synchronous calls, out-of-order reply matching,
  late-reply handling, and explicit external blocking APIs;
- task-internal stream consumption, batching, end/error handling, drop
  cancellation, late stream events, and credit accounting;
- `ctx-future` context-returning suspension primitives;
- protocol-derived send surfaces and compile-time receive declarations for the
  proposed protocol baseline;
- diagnostics snapshots and roadmap evidence;
- Unix-only signal forwarding through a safe signal bridge, packaged behind a
  default-enabled optional feature.

## PR Sequence

- PR 1: Unix signal bridge implementation and evidence.
- Final PR: aggregate merge branch containing the Unix bridge plus this
  readiness evidence, targeting `main`.

## Requirements Closed By The Final Pass

- REQ-130: POSIX signal-handler work is delegated to `signal-hook`; task
  messages are constructed outside the handler.
- REQ-131: `mpi::forward_signals` provides a Unix-only bridge from observed
  signals to generated task messages.
- REQ-132: the signal bridge dependency is controlled by the default-enabled
  `unix-signals` feature.

## Remaining Non-Blocking Gaps

The following gaps remain visible for human system-engineering decision or later
implementation slices:

- REQ-061 and REQ-062: generated call-wait event handlers for direct awaited
  assignment, pre-await side effects, awaited-let reply projection, and two
  pending call futures now use native `CtxFuture` dispatch while suspended;
  standard-future fallback handlers for stream-next loops and other body shapes
  still defer ordinary messages while waiting.
- VAL-012: Unix signal support has implementation, inspection evidence, and an
  application-level example; running that example remains Unix-host validation
  evidence.
- REQ-160..REQ-170 and VAL-015 remain proposed protocol baseline items rather
  than approved implementation obligations.

These gaps are traceability items, not local build failures.

## Commands Run

The following commands passed from the workspace root:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --doc
cargo check -p mpi --target x86_64-unknown-linux-gnu
```

After the GitHub Actions run exposed new Rust 1.96 Clippy lints, the CI command
sequence was repeated with `cargo +stable` on Rust 1.96.1, including
`cargo +stable clippy --workspace --all-targets -- -D warnings`.

`cargo test` was run on the Windows host. The Unix signal bridge was additionally
type-checked for `x86_64-unknown-linux-gnu`; its Unix-only unit test is compiled
and executed on Unix targets.

## Human Decisions Needed

- Decide whether generated handler lowering for full REQ-062 behavior should be
  the next implementation slice.
- Decide whether to promote any proposed protocol requirements after reviewing
  the candidate implementation and validation needs.
- Run the Unix signal bridge example on a Unix host before marking VAL-012
  fully validated.

The human maintainer remains the approval authority for merge.
