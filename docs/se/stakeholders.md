# Stakeholders and Needs

This document identifies the initial stakeholders and needs for `mpi-rs`.

This document is part of the authoritative systems-engineering baseline under `docs/se/`.

## Project purpose

`mpi-rs` shall provide a message-passing model for Rust applications while keeping the public API idiomatic for Rust.

The library is intended to let developers structure software as tasks with typed messages, generated send methods, bounded queues, synchronous calls, streaming interactions, cancellation, and safe task-local receive behavior.

## Stakeholders

| ID | Stakeholder | Description |
|---|---|---|
| STK-001 | Human maintainer / system engineer | The repository owner who defines needs, approves requirements, reviews AI-generated work, and controls merge to `main`. |
| STK-002 | Rust application developer | A developer using `mpi-rs` to build message-passing Rust applications. |
| STK-003 | Library contributor | A human or AI-assisted contributor changing runtime, macro, tests, or documentation. |
| STK-004 | Test and validation workflow | AI or CI workflow responsible for checking that requirements are verified and scenarios are validated. |
| STK-005 | Runtime user / operator | A person or system running an application built with `mpi-rs` and needing predictable task behavior and diagnostics. |
| STK-006 | Safety and maintainability reviewer | A reviewer concerned with blocking behavior, queue correctness, cancellation, compile-time guarantees, and dependency risk. |

## Stakeholder needs

### Maintainer needs

SN-001: The maintainer needs a repository structure where systems-engineering documents can drive implementation, testing, review, validation, and traceability.

SN-002: The maintainer needs AI agents to work from explicit role instructions so implementation, testing, review, and validation remain separate activities.

SN-003: The maintainer needs human control over approval and merge decisions.

SN-004: The maintainer needs current authoritative documents to be clearly distinguished from outdated repository material.

### Rust developer needs

SN-010: Rust developers need an idiomatic task API rather than an API that feels like a foreign actor or signal system.

SN-011: Rust developers need tasks to declare at compile time which messages they can receive.

SN-012: Rust developers need generated task handles with send methods instead of manually constructing and enqueueing low-level message enums.

SN-013: Rust developers need asynchronous events, typed synchronous calls, and generator-style streams to be expressed with clear Rust APIs.

SN-014: Rust developers need queue-full errors to be explicit and recoverable.

SN-015: Rust developers need task-internal synchronous waits to avoid blocking the task OS thread.

SN-016: Rust developers need stream consumption to avoid one queued message per item by supporting batching behind a simple `next(ctx).await` API.

SN-017: Rust developers need cancellation of dropped streams to be automatic and non-blocking.

SN-018: Rust developers need reusable protocol declarations so independently
built applications can share typed message contracts across separate binaries.

### Contributor needs

SN-020: Contributors need stable terminology for message, event, call, stream, task, placement, priority, response, and session.

SN-021: Contributors need clear architecture and interface boundaries between runtime primitives, task macros, queues, receive logic, sessions, streams, and signal support.

SN-022: Contributors need a practical implementation order that allows incremental development and review.

SN-023: Contributors need compile-time receive checks to prevent handlers from waiting for undeclared response or stream event messages.

SN-024: Contributors need protocol compatibility rules that distinguish
compatible additions from incompatible message contract changes.

### Testing and validation needs

SN-030: Test workflows need each testable requirement to have an identifiable verification method and evidence artifact.

SN-031: Validation workflows need scenario-level evidence that the public API supports intended developer workflows.

SN-032: Review workflows need traceability from needs to requirements, architecture, interfaces, code, tests, and validation scenarios.

### Runtime and safety needs

SN-040: Runtime users need predictable queue ordering for normal and priority messages.

SN-041: Runtime users need the start message to be the first application message received by a newly spawned task.

SN-042: Runtime users need calls and streams to match replies by logical interaction so concurrent handlers do not receive each other's replies.

SN-043: Runtime users need cancellation and late stream replies to be handled safely without hiding ordinary protocol flaws.

SN-044: Runtime users need POSIX signal integration to avoid unsafe allocation or non-async-signal-safe operations in signal handlers.

SN-045: Maintainers and operators need diagnostics for sessions, queues, timeouts, deadlocks, unknown-session replies, and stream lifecycle issues.

## Constraints

CON-001: The implementation language is Rust.

CON-002: The user-facing API should remain idiomatic for Rust.

CON-003: The authoritative systems-engineering baseline is stored under `docs/se/`.

CON-004: The process should remain lightweight enough for a private open-source repository.

CON-005: Human approval is required for merge to `main`.

CON-006: AI implementation, testing, review, validation, and traceability activities should be separable.

## Notes

This document should evolve as the human maintainer makes explicit product, API, implementation, and release decisions.
