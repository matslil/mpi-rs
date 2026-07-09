# Shared Stakeholders and Process Needs

This document identifies stakeholders and needs for repository builds, process, and AI-assisted workflows.

Crate-specific users, product needs, and usability scenarios belong in crate-local systems-engineering documents.

## Stakeholders

| ID | Stakeholder | Description |
|---|---|---|
| REPO-STK-001 | Human maintainer / system engineer | Defines scope, approves requirements, reviews AI-generated work, and controls merge to `main`. |
| REPO-STK-002 | Library contributor | A human or AI-assisted contributor changing code, tests, or documentation. |
| REPO-STK-003 | Test and validation workflow | AI or CI workflow responsible for checking that requirements are verified and scenarios are validated. |
| REPO-STK-004 | Release workflow | Human or AI-assisted workflow preparing release evidence. |

## Needs

The original process-oriented stakeholder need IDs remain stable after this split:

- SN-001: The maintainer needs a repository structure where systems-engineering documents can drive implementation, testing, review, validation, and traceability.
- SN-002: The maintainer needs AI agents to work from explicit role instructions so implementation, testing, review, and validation remain separate activities.
- SN-003: The maintainer needs human control over approval and merge decisions.
- SN-004: The maintainer needs current authoritative documents to be clearly distinguished from outdated repository material.
- SN-030: Test workflows need each testable requirement to have an identifiable verification method and evidence artifact.
- SN-031: Validation workflows need scenario-level evidence that the public API supports intended developer workflows.
- SN-032: Review workflows need traceability from needs to requirements, architecture, interfaces, code, tests, and validation scenarios.

REPO-SN-001: The maintainer needs a repository structure where systems-engineering documents can drive implementation, testing, review, validation, and traceability.

REPO-SN-002: The maintainer needs shared documentation to be limited to builds, process, and workflows so crate behavior is owned by crate-local documentation.

REPO-SN-003: The maintainer needs AI agents to work from explicit role instructions so implementation, testing, review, validation, and traceability remain separate activities.

REPO-SN-004: The maintainer needs human control over approval and merge decisions.

REPO-SN-005: Contributors need current authoritative documents to be clearly distinguished from outdated repository material.

REPO-SN-006: Test workflows need expected commands, reporting conventions, and traceability conventions.

REPO-SN-007: Release workflows need enough evidence for human release decisions.

## Constraints

The original process constraints remain stable after this split:

- CON-001: The implementation language is Rust.
- CON-002: The user-facing API should remain idiomatic for Rust.
- CON-003: The authoritative systems-engineering baseline is stored in current documents named `se-*.md`, with workspace-level documents under `docs/` and crate-level documents beside the crate they describe.
- CON-004: The process should remain lightweight enough for a private open-source repository.
- CON-005: Human approval is required for merge to `main`.
- CON-006: AI implementation, testing, review, validation, and traceability activities should be separable.

REPO-CON-001: The implementation language for workspace crates is Rust.

REPO-CON-002: The process should remain lightweight enough for a private open-source repository.

REPO-CON-003: Human approval is required for merge to `main`.

REPO-CON-004: AI implementation, testing, review, validation, and traceability activities should be separable.
