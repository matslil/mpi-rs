# Shared Workflow Validation Scenarios

This document defines validation scenarios for repository process and workflows.

Crate usability and product validation scenarios belong in crate-local `se-*.md` files.

## Scenario Status Values

Use these values:

- `proposed`: scenario drafted but not yet approved;
- `approved`: scenario approved for validation;
- `passed`: evidence shows the scenario works;
- `partial`: scenario partly works or has limitations;
- `failed`: scenario does not work;
- `blocked`: scenario cannot yet be evaluated;
- `deferred`: scenario intentionally postponed.

## REPO-VAL-001: AI workflow implements from crate-local requirements

Status: approved

A maintainer asks AI workflows to implement, test, review, validate, or update traceability for a crate change.

Expected outcome:

- the agent reads shared process instructions;
- the agent reads the affected crate's local `se-*.md` files;
- the agent identifies affected crate-local requirements;
- implementation, testing, review, validation, and traceability remain separable;
- human approval remains required.

Evidence type: process demonstration through pull request evidence.

## REPO-VAL-002: Shared docs remain process-only

Status: approved

A repository documentation change updates shared `docs/se-*.md` files.

Expected outcome:

- shared docs describe builds, process, workflows, traceability conventions, or release evidence;
- crate behavior is placed in crate-local documentation;
- any cross-crate change updates each affected crate baseline.

Evidence type: documentation inspection.
