# Send API Scope

This document is part of the authoritative systems-engineering baseline named `se-*.md`.

Scope: `crates/mpi-core`.

It records the human-approved design decision that generated send APIs are split by caller scope:

- external code may use explicit blocking send APIs;
- task message handlers may use context-aware non-blocking send APIs;
- the two surfaces shall remain visually and type-system distinct.

## Requirements

### REQ-122: External blocking send scope

Generated blocking send APIs shall only be used by code that is outside task message handler scope.

Rationale: External callers do not have a task queue or task-local receive state. Blocking APIs make the thread-blocking behavior explicit and avoid pretending that an external caller can participate in task-local suspension.

Source: Human maintainer decision, SN-010, SN-015

Verification: inspection

Status: approved

### REQ-123: Task-internal non-blocking send scope

Generated non-blocking send APIs shall only be used from within task message handler scope and shall require access to the generated task context.

Rationale: Task-internal code has task-local receive state and must not accidentally block the task OS thread. Requiring the generated task context makes the call site visibly task-internal and gives the runtime a place to allocate sessions, register waiters, and suspend/resume handlers.

Source: Human maintainer decision, SN-012, SN-015, SN-023

Verification: inspection

Status: approved

## Architecture rules

ARCH-083: Generated blocking send methods belong to the external caller surface and shall not accept a task context.

ARCH-084: Generated non-blocking send methods belong to the task-internal surface and shall require a generated task context or a trait implemented only by generated task contexts.

ARCH-085: Generated task handles may expose both surfaces, but their method names and signatures shall make the caller scope unambiguous.

## Interface rules

INT-093: External blocking send methods shall be explicitly named as blocking, for example with a `_blocking` suffix.

INT-094: Task-internal non-blocking send methods shall require a context argument such as `ctx` and shall not be callable from ordinary external code that lacks a generated task context.

INT-095: Examples and validation scenarios shall not show task handlers calling external blocking send APIs.

## Examples

External caller:

```rust
let reply = server.get_blocking(key)?;
```

Task-internal caller:

```rust
let reply = server.get(ctx, key).await?;
```

The first form is explicit about blocking an ordinary thread. The second form is explicit about using task-local context and must suspend the handler rather than blocking the task OS thread.

## Traceability

| Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|
| REQ-122, REQ-123 | ARCH-083, ARCH-084, ARCH-085 | INT-093, INT-094, INT-095 | VAL-004, VAL-011 |

## System Engineering Agent Report

### Summary

The send API scope decision clarifies an existing ambiguity in REQ-120 and REQ-121. It narrows external blocking APIs to non-task code and narrows non-blocking context-aware APIs to task message handler code.

### Documents inspected

- `AGENTS.md`
- `docs/agents/process.md`
- `docs/agents/system-engineering-agent.md`
- `docs/se-requirements.md`
- `docs/se-architecture.md`
- `docs/se-interfaces.md`
- `docs/se-verification-plan.md`
- `docs/se-validation-scenarios.md`
- `docs/se-traceability.md`

### Requirements affected

- REQ-120
- REQ-121
- REQ-122
- REQ-123

### Consistency findings

| Severity | Area | Finding | Affected IDs | Recommended action |
|---|---|---|---|---|
| note | external caller API | Existing requirements already require explicit blocking APIs and distinct task-internal APIs, but they did not state an exclusive caller scope. | REQ-120, REQ-121 | Add REQ-122 and REQ-123 as scope rules. |
| note | task-local runtime | Requiring context for non-blocking APIs aligns with suspended handler and waiter-registry requirements. | REQ-061, REQ-062, REQ-063 | Use context-aware signatures for task-internal calls and streams. |

### Traceability impact

This crate-level addendum extends the external caller requirement group with REQ-122 and REQ-123. Workspace-level traceability should continue to index these requirements when they affect cross-document review.
