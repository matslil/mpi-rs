# Change Process

This document defines the lightweight change process for `mpi-rs`.

The process is designed for a private open-source Rust repository where the human maintainer acts as system engineer and AI workflows may implement, test, review, validate, and maintain traceability.

## Principles

CHG-001: Human approval is required for merge to `main`.

CHG-002: Engineering intent should be captured in documents before or alongside implementation.

CHG-003: Requirements should not be weakened merely to make implementation easier.

CHG-004: Implementation, testing, review, validation, and traceability should be separable activities.

CHG-005: Pull requests should contain enough evidence for human review.

CHG-006: Outdated documents should not be treated as authoritative.

## Change request template

Use this structure for issues or pull request descriptions:

```markdown
# Change Request

## Purpose

## Stakeholder needs affected

## Requirements affected

## Architecture affected

## Interfaces affected

## Validation scenarios affected

## Proposed change

## Verification plan

## Traceability impact

## Risks and ambiguities

## Human decisions needed
```

## Pull request evidence template

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

## Change classes

### Engineering-only change

Updates systems-engineering documents, agent documents, or process rules without changing Rust code.

Required evidence:

- documents changed;
- consistency impact;
- traceability impact;
- human decisions needed.

### Implementation change

Updates production Rust code or generated macro behavior.

Required evidence:

- affected requirements;
- affected architecture/interface sections;
- tests added or updated;
- commands run;
- validation impact;
- traceability update or gap.

### Test-only change

Updates tests, fixtures, compile-fail tests, or test helpers.

Required evidence:

- requirements verified by the tests;
- reason for changed expectations;
- commands run;
- traceability update or gap.

### Validation change

Updates examples, scenarios, user-facing documentation, or validation evidence.

Required evidence:

- stakeholder needs affected;
- scenarios evaluated;
- validation status;
- usability or documentation gaps.

### Release change

Prepares release notes, versioning, package checks, or release readiness evidence.

Required evidence:

- release scope;
- requirement status;
- verification status;
- validation status;
- known limitations;
- human release decision needed.

## Requirement change rules

- New requirements should receive stable IDs.
- Existing IDs should not be reused for different meanings.
- Obsolete requirements should be marked obsolete before removal.
- Requirements should include verification method and status.
- Requirements that affect public API should link to interfaces and validation scenarios.

## Architecture and interface change rules

Architecture or interface changes should identify:

- affected requirements;
- affected examples;
- migration impact;
- test impact;
- validation impact.

Interface changes that affect user-facing Rust API should update the affected crate-local interface baseline and relevant validation scenarios.

## Test change rules

Tests should not be removed or weakened unless:

- the requirement is obsolete;
- the test was incorrect;
- a better test replaces it;
- the human maintainer approves the change.

The reason shall be documented in the PR.

## Traceability change rules

Traceability should be updated when:

- a requirement is added, changed, deferred, or obsoleted;
- implementation files are added or moved;
- tests are added or moved;
- validation scenarios are added, changed, or evaluated;
- verification status changes.

## Human decision points

Escalate to the human maintainer when:

- requirements conflict;
- the implementation would require changing crate behavior outside the approved crate-local baseline;
- a testable behavior is ambiguous;
- public API ergonomics are unclear;
- a dependency or runtime choice is needed;
- `unsafe` Rust is proposed;
- a requirement should be deferred or made obsolete;
- validation shows the API works but feels wrong.

## Merge rule

The human maintainer decides whether to merge. Agent reports, passing tests, review findings, validation results, and traceability are evidence for that decision.
