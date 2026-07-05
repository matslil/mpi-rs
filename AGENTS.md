# Repository AI Instructions

This repository uses a lightweight systems-engineering process for AI-assisted Rust development.

The human maintainer acts as the system engineer and approval authority. AI agents may implement, test, review, validate, and maintain traceability, but they must not approve their own changes.

## Source of truth

The authoritative project baseline is the systems-engineering document set under `docs/se/`.

The most important baseline documents are:

- `docs/se/stakeholders.md`
- `docs/se/requirements.md`
- `docs/se/architecture.md`
- `docs/se/ctx-future.md`
- `docs/se/interfaces.md`
- `docs/se/verification-plan.md`
- `docs/se/validation-scenarios.md`
- `docs/se/traceability.md`
- `docs/se/glossary.md`
- `docs/se/change-process.md`

When there is a conflict between generated code and the systems-engineering documents, the systems-engineering documents win. When two current systems-engineering documents conflict, report the conflict instead of guessing.

## Required reading before making changes

Before changing production code, tests, examples, or process documents, read:

- `docs/agents/process.md`
- the role-specific document under `docs/agents/`
- `docs/se/requirements.md`
- `docs/se/architecture.md`
- `docs/se/ctx-future.md`
- `docs/se/interfaces.md`
- `docs/se/verification-plan.md`
- `docs/se/traceability.md`

Read `docs/se/stakeholders.md` when changing needs, scope, constraints, or process assumptions.

Read `docs/se/validation-scenarios.md` when the change affects public usability, examples, task behavior, calls, streams, cancellation, external APIs, signal handling, or diagnostics.

Read `docs/se/glossary.md` when adding or changing terminology.

## Core project intent

`mpi-rs` provides a message-passing model for Rust applications while keeping the user-facing API idiomatic for Rust.

The core concepts are:

- a task is an operating-system thread with an associated bounded message queue;
- a task declares, at compile time, the messages it can receive;
- `Message` is the umbrella term for all received items;
- an `event` is an asynchronous message with no reply;
- a `call` is a synchronous exchange with exactly one typed response;
- a `stream` is a generator-style exchange with zero or more typed stream events followed by end, error, or cancellation;
- `SessionId` identifies logical interactions for calls and streams;
- normal and priority messages are placed according to the receiver's declaration, not the sender's choice;
- the start message is forced to priority and must be the first application message received by a new task;
- handlers suspend while waiting for replies or stream events instead of blocking the task thread.

## Agent authority model

Agents may:

- propose changes;
- create implementation changes on a branch;
- add tests and verification evidence;
- update traceability when the supporting artifacts exist;
- report inconsistencies, risks, and ambiguities.

Agents must not:

- invent requirements;
- weaken requirements to make implementation easier;
- remove tests merely to make a build pass;
- merge their own work;
- silently ignore a requirement, interface rule, architecture rule, or validation scenario;
- treat outdated documents as authoritative;
- introduce `unsafe` Rust unless an approved requirement or architecture decision explicitly permits it;
- introduce a new runtime dependency without documenting the reason.

## Required evidence in pull requests

Every non-trivial pull request should include:

- affected requirement IDs;
- affected architecture or interface sections;
- tests added or updated;
- commands run;
- verification result;
- validation impact;
- traceability updates or traceability gaps;
- open questions requiring human decision.

## Required test discipline

Tests that verify requirements should reference the relevant requirement ID in at least one of:

- test function name;
- test module name;
- test comment;
- traceability table.

Example:

```rust
#[test]
fn req_030_start_message_is_received_first() {
    // ...
}
```

## Ambiguity rule

If a requirement, architecture rule, or interface expectation is ambiguous, do not invent missing behavior. Report the ambiguity and propose a clarification for the human system engineer.

## Human-in-the-loop rule

The human maintainer reviews and merges. AI-generated implementation, tests, review, validation, and traceability updates are evidence for the human decision; they are not the decision itself.
