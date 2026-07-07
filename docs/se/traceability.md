# Traceability Matrix

This document provides the initial traceability baseline for `mpi-rs`.

The matrix intentionally starts with many implementation and test entries marked `pending`, because the current baseline establishes the systems-engineering model rather than implementing runtime behavior.

## Traceability status values

Use the status values from `docs/agents/process.md`:

- proposed;
- approved;
- implemented;
- verified;
- validated;
- deferred;
- obsolete.

## High-level traceability

| Need | Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|---|
| SN-001 | REQ-001, REQ-002, REQ-003 | process | AGENTS.md, docs/agents/* | VAL-014 |
| SN-002 | REQ-002 | process | docs/agents/* | VAL-014 |
| SN-003 | REQ-003 | process | docs/agents/process.md | VAL-014 |
| SN-004 | REQ-001 | process | AGENTS.md | VAL-014 |
| SN-010 | REQ-050, REQ-101, REQ-114, REQ-120 | ARCH-001, ARCH-078 | INT-001, INT-010, INT-076, INT-090 | VAL-001, VAL-007, VAL-011 |
| SN-011 | REQ-025, REQ-026, REQ-070, REQ-071 | ARCH-011, CMP-011 | INT-040, INT-041, INT-042 | VAL-001, VAL-006 |
| SN-012 | REQ-027, REQ-028, REQ-051 | CMP-002, CMP-012 | INT-020, INT-021, INT-022 | VAL-001, VAL-003 |
| SN-013 | REQ-021, REQ-022, REQ-023, REQ-024, REQ-090, REQ-100 | CMP-009, CMP-010 | INT-013, INT-014, INT-015 | VAL-004, VAL-007 |
| SN-014 | REQ-012, REQ-013, REQ-014 | CMP-003, ARCH-020..ARCH-024 | INT-021, INT-100 | VAL-003 |
| SN-015 | REQ-060, REQ-061, REQ-062, REQ-064, REQ-113, REQ-114, REQ-121 | CMP-006, CMP-007, CMP-015, ARCH-044, ARCH-077, ARCH-078 | INT-004, INT-025, INT-043..INT-045, INT-076, INT-092 | VAL-004, VAL-011 |
| SN-016 | REQ-100..REQ-105, REQ-112, REQ-113, REQ-114 | ARCH-070..ARCH-078 | INT-070..INT-081 | VAL-007, VAL-009 |
| SN-017 | REQ-106, REQ-107 | ARCH-073, ARCH-074 | INT-073, INT-074 | VAL-008 |
| SN-020 | REQ-020..REQ-024 | glossary | docs/se/glossary.md | documentation review |
| SN-021 | REQ-051, REQ-110, REQ-151, REQ-152 | CMP-001..CMP-015 | INT-010..INT-111 | review evidence |
| SN-022 | REQ-150, REQ-151, REQ-152 | implementation phases, ctx-future crate structure | process | review evidence |
| SN-023 | REQ-070, REQ-071, REQ-072 | CMP-011 | INT-040..INT-042 | VAL-006 |
| SN-030 | VER-001..VER-005 | verification plan | docs/agents/test-agent.md | VAL-014 |
| SN-031 | validation scenarios | validation plan | docs/agents/validation-agent.md | VAL-001..VAL-014 |
| SN-032 | traceability matrix | process | docs/agents/traceability-agent.md | VAL-014 |
| SN-040 | REQ-030..REQ-035 | ARCH-020..ARCH-025 | INT-030..INT-032 | VAL-010 |
| SN-041 | REQ-040..REQ-043, REQ-053 | ARCH-030..ARCH-032 | INT-012, INT-017 | VAL-002 |
| SN-042 | REQ-080..REQ-097 | ARCH-050..ARCH-067 | INT-050..INT-068 | VAL-004, VAL-005 |
| SN-043 | REQ-095..REQ-097, REQ-100..REQ-113 | ARCH-070..ARCH-079 | INT-070..INT-081 | VAL-008, VAL-009 |
| SN-044 | REQ-130, REQ-131 | ARCH-090..ARCH-092 | INT-110, INT-111 | VAL-012 |
| SN-045 | REQ-082, REQ-094..REQ-097, REQ-140 | diagnostics, ARCH-064..ARCH-067 | INT-052, INT-063..INT-068 | VAL-013 |

## Requirement group traceability

| Requirement group | Source needs | Architecture | Interfaces | Implementation | Verification | Validation | Status |
|---|---|---|---|---|---|---|---|
| REQ-001..REQ-003 | SN-001..SN-004 | process | AGENTS.md, docs/agents/* | docs/se/*, docs/agents/* | inspection pending | VAL-014 | approved |
| REQ-010..REQ-015 | SN-010, SN-014, SN-040 | CMP-001..CMP-004, ARCH-010..ARCH-014 | generated task API, `TaskQueue`, `TaskContext` | `crates/mpi/src/queue.rs`, `crates/mpi/src/task.rs`, `crates/mpi-macros/src/lib.rs` | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`; REQ-015 inspection still needed | VAL-001, VAL-003 | implemented |
| REQ-020..REQ-028 | SN-011..SN-013, SN-020 | CMP-002, CMP-005, CMP-012 | INT-013..INT-025, INT-030..INT-032 | generated message enums, task handles, event/call/stream methods, direct queue enqueue | `crates/mpi/tests/task_macro.rs`; generated-code inspection still needed | VAL-001, VAL-003, VAL-004, VAL-007 | implemented |
| REQ-030..REQ-035 | SN-040, SN-042 | ARCH-020..ARCH-025 | INT-016, INT-030..INT-032 | `TaskMessage::placement`, `TaskQueue` normal/priority queues, generated placement arms | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`; REQ-035 partial through generated protocol placement | VAL-010 | verified |
| REQ-040..REQ-043 | SN-041 | ARCH-030..ARCH-032 | INT-012, INT-017 | `spawn_task`, generated `Start` placement | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | VAL-002 | verified |
| REQ-050..REQ-053 | SN-010, SN-012, SN-021, SN-041 | CMP-006, CMP-012 | INT-010..INT-025 | `#[task]` macro generates message enum, context, handle, send methods, placement, spawn helper, and dispatch | `crates/mpi/tests/task_macro.rs` | VAL-001, VAL-003 | implemented |
| REQ-060..REQ-064 | SN-015, SN-021, SN-042 | ARCH-040..ARCH-044, CMP-007, CMP-015 | INT-043..INT-045, context receive API pending | `crates/ctx-future`, `crates/mpi/src/runtime.rs::block_on_ctx_task`, `SuspendedCall` and `SuspendedStreamNext` ctx-future integration; ordinary messages are still deferred while an active handler waits; macro-generated async handler lowering still adapted through standard `Future` | `crates/ctx-future/tests/context_borrow.rs`, `crates/mpi/tests/runtime_baseline.rs`; REQ-062 and macro-native ctx-future handler lowering pending | VAL-004, VAL-005 | implemented |
| REQ-070..REQ-072 | SN-011, SN-023 | CMP-011 | INT-040..INT-042 | pending | compile-fail/inspection pending | VAL-006 | approved |
| REQ-080..REQ-084 | SN-042, SN-045 | ARCH-050..ARCH-054 | INT-050..INT-052 | `SessionId`, `EndpointId`, `SessionIdAllocator`, task-local session allocation | `crates/mpi/tests/runtime_baseline.rs`; broader tracing/debug use pending | VAL-004, VAL-005, VAL-013 | implemented |
| REQ-090..REQ-097 | SN-013, SN-042, SN-045 | ARCH-060..ARCH-067 | INT-060..INT-068 | `Response<T>`, generated call plumbing, session-matched waiters, `LateReplyPolicy`, `LateReplyRef`, `LateReplyAction`, default no-op late-reply handling, and optional generated `#[late_reply]` handler support | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs` | VAL-004, VAL-005, VAL-013 | implemented |
| REQ-100..REQ-114 | SN-010, SN-015, SN-016, SN-017, SN-021, SN-043 | ARCH-070..ARCH-079 | INT-070..INT-081 | stream events, consumer buffering, drop cancellation attempt, generated cancellation routing, producer credit cleanup, explicit stream-flow and stream-cancelled send errors, sink batching/end/error, late stream policy; producer suspension under backpressure remains partial | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/src/stream.rs` unit tests; REQ-113 pending | VAL-007, VAL-008, VAL-009 | implemented |
| REQ-120..REQ-121 | SN-010, SN-015 | ARCH-080..ARCH-082 | INT-090..INT-092 | generated `_blocking` methods and context-aware task-internal methods | `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs` | VAL-011 | verified |
| REQ-130..REQ-131 | SN-044 | ARCH-090..ARCH-092 | INT-110..INT-111 | pending | inspection pending | VAL-012 | approved |
| REQ-140 | SN-045 | diagnostics | diagnostics interfaces pending | pending | inspection pending | VAL-013 | approved |
| REQ-150..REQ-152 | SN-021, SN-022 | implementation phases, crate structure, CMP-015 | `docs/se/ctx-future.md`, `crates/ctx-future` | `crates/ctx-future` | `crates/ctx-future/tests/context_borrow.rs`, inspection | review evidence | approved |

## Known initial gaps

GAP-001: Several implementation links now exist, but generated-code inspection evidence remains incomplete for some macro-produced artifacts.

GAP-002: Test artifacts exist for early runtime phases, but validation examples for the public workflows remain pending.

GAP-003: The current compile-fail tests use temporary Cargo fixtures. A dedicated compile-fail framework may still be useful before expanding receive-declaration tests.

GAP-004: Diagnostics interfaces are intentionally deferred beyond the initial message model baseline.

GAP-005: Unix signal support is later-phase work and remains deferred for validation.

GAP-006: The `ctx-future` crate now has a task-local runtime integration point in `mpi`, but macro-generated async handlers are still adapted through standard `Future`; full generated lowering into native `CtxFuture` continuations remains pending.

## Traceability maintenance rules

- Add rows when requirements are added.
- Do not remove rows merely because implementation is missing.
- Mark obsolete rows explicitly before removal.
- Keep requirement IDs stable once used by tests or code comments.
- Link tests to requirement IDs.
- Link examples to validation scenario IDs.
