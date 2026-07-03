# AI Engineering Process

This document defines how AI agents are expected to work in the `mpi-rs` repository.

The process is intentionally lightweight. It uses systems-engineering artifacts to keep AI-generated Rust implementation, tests, review, validation, and traceability aligned with the project intent.

## Process goals

The process shall:

- keep the human maintainer in the systems-engineering role;
- make requirements and interfaces explicit before implementation;
- allow implementation, testing, review, validation, and traceability to be performed by separate AI workflows;
- preserve human approval for merge to `main`;
- reduce self-confirming AI mistakes by separating roles;
- keep the repository practical for a private open-source Rust project.

## Authoritative inputs

The authoritative systems-engineering baseline is:

- `docs/se/stakeholders.md`
- `docs/se/requirements.md`
- `docs/se/architecture.md`
- `docs/se/interfaces.md`
- `docs/se/verification-plan.md`
- `docs/se/validation-scenarios.md`
- `docs/se/traceability.md`
- `docs/se/glossary.md`
- `docs/se/change-process.md`

Role instructions are stored in:

- `docs/agents/system-engineering-agent.md`
- `docs/agents/implementation-agent.md`
- `docs/agents/test-agent.md`
- `docs/agents/review-agent.md`
- `docs/agents/validation-agent.md`
- `docs/agents/traceability-agent.md`
- `docs/agents/release-agent.md`

Historical design notes are non-authoritative unless a current systems-engineering document explicitly references them.

## Normal change flow

1. The human system engineer updates or approves changes to needs, requirements, architecture, interfaces, validation scenarios, or process documents.
2. The System Engineering Agent checks the consistency of the engineering documents.
3. The Implementation Agent implements the approved requirements in Rust.
4. The Test Agent derives or updates tests from the requirements and verification plan.
5. The Traceability Agent updates traceability between needs, requirements, architecture, code, tests, and validation scenarios.
6. The Review Agent reviews the complete change against requirements, architecture, interfaces, tests, and safety constraints.
7. The Validation Agent checks the change against intended usage scenarios and developer experience.
8. The human maintainer reviews the evidence and decides whether to merge.

## Authority model

The human maintainer is the approval authority.

Agents may propose updates to engineering documents, but they shall not approve changes to requirements, architecture, interfaces, validation scenarios, or process rules.

The Implementation Agent and Test Agent should be separate workflows. The Review Agent and Validation Agent should not rely only on implementation-agent claims; they should inspect the diff, source artifacts, and evidence.

## Change classes

### Engineering-only change

An engineering-only change updates documents such as requirements, architecture, interfaces, validation scenarios, process rules, or traceability without changing Rust code.

Required checks:

- System Engineering Agent consistency review;
- Traceability Agent gap review;
- human approval.

### Implementation change

An implementation change modifies production Rust code or generated macro behavior.

Required checks:

- affected requirements identified;
- architecture and interface impact identified;
- tests added or updated;
- cargo checks run where possible;
- review completed;
- traceability updated or gaps reported.

### Test-only change

A test-only change modifies tests, fixtures, examples used as tests, or verification reports.

Required checks:

- requirements covered by the tests identified;
- tests must not be weakened without explicit rationale;
- verification plan and traceability updated when affected.

### Validation change

A validation change modifies examples, README usage, user-facing scenarios, or validation evidence.

Required checks:

- stakeholder needs affected;
- validation scenario status updated;
- usability concerns reported.

### Release change

A release change prepares a versioned release.

Required checks:

- required tests pass;
- verification report current;
- validation status current;
- traceability has no blocking gaps;
- release notes mention requirement-level changes.

## Required pull request evidence

Every non-trivial pull request should include:

```markdown
## Summary

## Affected requirements

## Affected architecture and interfaces

## Tests and verification

## Validation impact

## Traceability impact

## Commands run

## Risks, gaps, or human decisions needed
```

## Requirement status values

Use these status values in reports and traceability:

- `proposed`: drafted but not approved for implementation;
- `approved`: approved by the human system engineer;
- `implemented`: implementation exists but verification is incomplete;
- `verified`: verification evidence exists and passes;
- `validated`: validation scenario evidence supports the stakeholder need;
- `deferred`: intentionally postponed;
- `obsolete`: no longer applicable.

## Verification methods

Use the following verification method names:

- `test`: automated unit, integration, property, or doctest;
- `inspection`: review of code, generated code, macro expansion, or documentation;
- `analysis`: reasoning, model checking, static analysis, or proof-like argument;
- `demonstration`: executable example or manual scenario run.

## Completion rule

A change is not complete merely because code compiles. It is complete when the affected requirements, implementation, tests, verification evidence, validation impact, and traceability are consistent enough for human review.

## Ambiguity handling

When an agent finds ambiguity, it shall:

1. state the ambiguity;
2. identify affected requirement, architecture, interface, or validation IDs;
3. explain the implementation or testing consequence;
4. propose one or more clarifying edits;
5. stop short of inventing behavior unless explicitly instructed by the human maintainer.

## Outdated-document rule

Pre-existing documents outside `docs/se/`, `docs/agents/`, and `AGENTS.md` are not authoritative for the current baseline unless a current systems-engineering document explicitly references them.
