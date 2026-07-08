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
| SN-012 | REQ-027, REQ-028, REQ-051, REQ-169 | CMP-002, CMP-012, CMP-016 | INT-020, INT-021, INT-022, INT-009B | VAL-001, VAL-003, VAL-015 |
| SN-013 | REQ-021, REQ-022, REQ-023, REQ-024, REQ-090, REQ-100 | CMP-009, CMP-010 | INT-013, INT-014, INT-015 | VAL-004, VAL-007 |
| SN-014 | REQ-012, REQ-013, REQ-014 | CMP-003, ARCH-020..ARCH-024 | INT-021, INT-100 | VAL-003 |
| SN-015 | REQ-060, REQ-061, REQ-062, REQ-064, REQ-113, REQ-114, REQ-121 | CMP-006, CMP-007, CMP-015, ARCH-044, ARCH-077, ARCH-078 | INT-004, INT-018A, INT-025, INT-043..INT-045, INT-076, INT-092 | VAL-004, VAL-011 |
| SN-016 | REQ-100..REQ-105, REQ-112, REQ-113, REQ-114 | ARCH-070..ARCH-078 | INT-070..INT-081, INT-075A | VAL-007, VAL-009 |
| SN-017 | REQ-106, REQ-107 | ARCH-073, ARCH-074 | INT-073, INT-074 | VAL-008 |
| SN-018 | REQ-160..REQ-170 | CMP-016, ARCH-100..ARCH-109 | INT-006..INT-009B, INT-042A | VAL-015 |
| SN-020 | REQ-020..REQ-024 | glossary | docs/se/glossary.md | documentation review |
| SN-021 | REQ-051, REQ-110, REQ-151, REQ-152 | CMP-001..CMP-015 | INT-010..INT-111 | review evidence |
| SN-022 | REQ-150, REQ-151, REQ-152 | implementation phases, ctx-future crate structure | process | review evidence |
| SN-023 | REQ-070, REQ-071, REQ-072, REQ-166, REQ-168 | CMP-011, ARCH-105, ARCH-107 | INT-040..INT-042A, INT-009A | VAL-006, VAL-015 |
| SN-024 | REQ-160, REQ-162, REQ-164, REQ-165 | CMP-016, ARCH-103, ARCH-104 | protocol compatibility rules | VAL-015 |
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
| REQ-020..REQ-028 | SN-011..SN-013, SN-020 | CMP-002, CMP-005, CMP-012 | INT-013..INT-025, INT-030..INT-032 | generated message enums, task handles, event/call/stream methods, direct queue enqueue | `crates/mpi/tests/task_macro.rs`; `crates/mpi/examples/ping_pong.rs`, `crates/mpi/examples/synchronous_call.rs`, `crates/mpi/examples/streaming_query.rs`; `docs/reports/generated-task-api-inspection.md` | VAL-001, VAL-003, VAL-004, VAL-007 | implemented |
| REQ-030..REQ-035 | SN-040, SN-042 | ARCH-020..ARCH-025 | INT-016, INT-030..INT-032 | `TaskMessage::placement`, `TaskQueue` normal/priority queues, generated placement arms | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`; REQ-035 partial through generated protocol placement | VAL-010 | verified |
| REQ-040..REQ-043 | SN-041 | ARCH-030..ARCH-032 | INT-012, INT-017 | `spawn_task`, generated `Start` placement | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/examples/ping_pong.rs`, `crates/mpi/examples/priority_shutdown.rs` | VAL-002 | verified |
| REQ-050..REQ-053 | SN-010, SN-012, SN-021, SN-041 | CMP-006, CMP-012 | INT-010..INT-025 | `#[task]` macro generates message enum, context, handle, send methods, placement, spawn helper, and dispatch | `crates/mpi/tests/task_macro.rs`, `docs/reports/generated-task-api-inspection.md` | VAL-001, VAL-003 | implemented |
| REQ-060..REQ-064 | SN-015, SN-021, SN-042 | ARCH-040..ARCH-044, CMP-007, CMP-015 | INT-018A, INT-043..INT-045 | `crates/ctx-future`, `crates/mpi/src/runtime.rs::block_on_ctx_task`, `crates/mpi/src/runtime.rs::block_on_ctx_task_with_dispatch`, `crates/mpi/src/runtime.rs::block_on_handler`, `SuspendedCall` and `SuspendedStreamNext` ctx-future integration; handler declarations use ordinary `fn` syntax and generated dispatch enters the task-runtime `CtxFuture` boundary through `block_on_handler`; native `CtxFuture` handlers can dispatch ordinary messages while suspended, but generated dispatch still defers ordinary messages while an active handler waits | `crates/ctx-future/tests/context_borrow.rs`, `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`; REQ-062 has runtime-level ordinary dispatch evidence; REQ-063 session matching has direct out-of-order response evidence; generated-dispatch ordinary-message progress and native lowering of arbitrary handler bodies remain pending | VAL-001, VAL-004, VAL-005 | implemented |
| REQ-070..REQ-072 | SN-011, SN-023 | CMP-011, ARCH-105, ARCH-107 | INT-040..INT-042, INT-009A, INT-042A | generated context-aware call and stream methods require `CanReceive<T>` bounds; `#[task(..., receives(...))]` generates `CanReceive` impls for raw and protocol-qualified receive declarations | `crates/mpi/tests/scope_compile_fail.rs`, `crates/mpi/tests/task_macro.rs`; missing non-protocol call and stream receives fail to compile; missing and wrong protocol receive identities fail to compile | VAL-006 | verified |
| REQ-080..REQ-084 | SN-042, SN-045 | ARCH-050..ARCH-054 | INT-050..INT-052 | `SessionId`, `EndpointId`, `SessionIdAllocator`, task-local session allocation | `crates/mpi/tests/runtime_baseline.rs`; broader tracing/debug use pending | VAL-004, VAL-005, VAL-013 | implemented |
| REQ-090..REQ-097 | SN-013, SN-042, SN-045 | ARCH-060..ARCH-067 | INT-060..INT-068 | `Response<T>`, generated call plumbing, session-matched waiters, `LateReplyPolicy`, `LateReplyRef`, `LateReplyAction`, default no-op late-reply handling, and optional generated `#[late_reply]` handler support | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`; REQ-092 out-of-order response matching covered by `req_063_req_092_out_of_order_call_responses_match_session_ids` | VAL-004, VAL-005, VAL-013 | implemented |
| REQ-100..REQ-114 | SN-010, SN-015, SN-016, SN-017, SN-021, SN-043 | ARCH-070..ARCH-079 | INT-070..INT-081, INT-075A | stream events, consumer buffering, drop cancellation attempt, generated cancellation routing, producer credit cleanup, explicit stream-flow and stream-cancelled send errors, sink batching/end/error, producer-side native `CtxFuture` yield operations, late stream policy; producer suspension under backpressure remains partial | `crates/mpi/tests/runtime_baseline.rs`, `crates/mpi/tests/task_macro.rs`, `crates/mpi/src/stream.rs` unit tests; REQ-109 and REQ-112 have explicit mapped tests; REQ-113 has producer-yield scheduler-return evidence in `req_113_stream_sink_yield_item_returns_context_after_sending_batch`, while broader backpressure suspension remains pending | VAL-007, VAL-008, VAL-009 | implemented |
| REQ-120..REQ-121 | SN-010, SN-015 | ARCH-080..ARCH-082 | INT-090..INT-092 | generated `_blocking` methods and context-aware task-internal methods | `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs`, `crates/mpi/examples/ping_pong.rs`, `crates/mpi/examples/synchronous_call.rs` | VAL-011 | verified |
| REQ-130..REQ-131 | SN-044 | ARCH-090..ARCH-092 | INT-110..INT-111 | `crates/mpi/src/signal.rs`, Unix-only `forward_signals` bridge using safe `signal-hook` registration and an ordinary Rust forwarding thread | `docs/reports/unix-signal-bridge.md`; Unix-only unit test `req_130_req_131_signal_bridge_forwards_signal_outside_handler` | VAL-012 | verified |
| REQ-140 | SN-045 | diagnostics | INT-052, diagnostics interfaces pending for timeout/tracing/deadlock support | diagnostics roadmap in `docs/reports/diagnostics-roadmap.md`; `TaskQueueSnapshot`, `TaskDiagnosticsSnapshot`, and generated-context diagnostics access expose read-only queue and session state | `docs/reports/diagnostics-roadmap.md` inspection; `crates/mpi/tests/runtime_baseline.rs`; `crates/mpi/tests/task_macro.rs` | VAL-013 | implemented |
| REQ-150..REQ-152 | SN-021, SN-022 | implementation phases, crate structure, CMP-015 | `docs/se/ctx-future.md`, `crates/ctx-future` | `crates/ctx-future` | `crates/ctx-future/tests/context_borrow.rs`, inspection | review evidence | approved |
| REQ-160..REQ-170 | SN-012, SN-018, SN-023, SN-024 | CMP-002, CMP-012, CMP-016, ARCH-100..ARCH-109 | INT-006..INT-009B, INT-042A | candidate implementation in `crates/mpi-macros/src/lib.rs`, `crates/mpi/src/message.rs`, `crates/mpi/src/scope.rs`, and generated task contexts | `crates/mpi/tests/task_macro.rs`, `crates/mpi/tests/scope_compile_fail.rs`; full separate-binary validation pending | VAL-015 | proposed |

## Known initial gaps

GAP-001: Generated-code inspection evidence now covers the macro-produced message enum, handle methods, context type, placement implementation, spawn helper, dispatch arms, protocol conversion plumbing, and compile-time receive declarations. Native `CtxFuture` handler lowering remains a separate implementation gap.

GAP-002: Validation examples now exist for simple task declaration, predictable start, asynchronous events, typed synchronous calls, task-internal stream consumption, priority shutdown, and explicit external blocking calls. Tests now add evidence for out-of-order same-type call responses, ordinary-message non-discard, and stream flow-control credit enforcement. Validation examples for stream cancellation and flow-control stress behavior remain pending.

GAP-003: The current compile-fail tests use temporary Cargo fixtures. A dedicated compile-fail framework may still be useful if receive-declaration or macro-diagnostic coverage expands further.

GAP-004: A diagnostics roadmap and concrete read-only queue/task diagnostic snapshots now exist for REQ-140. Timeout APIs, tracing integration, richer session lifecycle diagnostics, and deadlock/debug support remain intentionally deferred beyond the initial message model baseline.

GAP-005: Unix signal support now has a Unix-only bridge implementation and inspection evidence. Cross-platform validation of an application-level signal-forwarding example remains deferred.

GAP-006: Handler declarations now use ordinary `fn` syntax and generated dispatch enters the task-runtime `CtxFuture` boundary through `block_on_handler`. The `ctx-future` crate still owns the standard-`Future` compatibility adapter needed for current `.await` handler bodies. The runtime has an ordinary-message dispatch hook for native `CtxFuture` handlers, and producer-side stream yield operations can send stream replies while returning control to that scheduler, but full generated lowering of arbitrary handler bodies into native `CtxFuture` continuations remains pending.

GAP-007: Protocol declarations now have a candidate macro/runtime
implementation for namespace-qualified message identities, explicit protocol
message types, protocol-derived send surfaces, and compile-time receive checks.
Separate-binary runtime capability discovery and validation remain pending.

GAP-008: Final implementation-readiness evidence is recorded in
`docs/reports/final-implementation-readiness.md`. Remaining gaps are explicit
human-decision or later-slice items rather than local verification failures.

## Traceability maintenance rules

- Add rows when requirements are added.
- Do not remove rows merely because implementation is missing.
- Mark obsolete rows explicitly before removal.
- Keep requirement IDs stable once used by tests or code comments.
- Link tests to requirement IDs.
- Link examples to validation scenario IDs.
