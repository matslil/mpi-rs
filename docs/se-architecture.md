# Shared Repository Architecture

This document describes repository-level process and workflow architecture.

It intentionally does not define the architecture of any crate. Crate architecture belongs in crate-local `se-*.md` files.

## Documentation Structure

Workspace-level systems-engineering documents under `docs/` describe:

- source-of-truth rules;
- AI agent workflows;
- build and verification command conventions;
- change-control process;
- traceability conventions;
- release evidence expectations.

Crate-level systems-engineering documents beside each crate describe that crate's:

- product purpose and scope;
- requirements;
- architecture;
- public and internal interfaces;
- verification expectations;
- validation scenarios;
- traceability.

## Architecture Rules

REPO-ARCH-001: `docs/se-index.md` is the entry point for the systems-engineering document set.

REPO-ARCH-002: Workspace-level documents shall not define crate message models, runtime behavior, macro behavior, platform bridge behavior, or crate public APIs.

REPO-ARCH-003: Crate-level documents shall not rely on shared documents for crate product behavior.

REPO-ARCH-004: If a change affects more than one crate, each affected crate shall update or reference its own crate-local baseline for its responsibilities.

REPO-ARCH-005: Shared workflow documents may describe how agents coordinate cross-crate changes, but not what any crate's behavior shall be.

REPO-ARCH-006: Cross-crate architecture changes shall treat crate boundaries as message boundaries unless the affected crate-local baselines document a human-approved exception.

REPO-ARCH-007: Repository-level feature naming conventions may describe how
optional crate and service features are named, but crate-local baselines shall
define the behavior enabled by those features.

## Workflow Components

| ID | Component | Responsibility |
|---|---|---|
| REPO-CMP-001 | Human maintainer | Approves requirements, architecture, interfaces, releases, and merge decisions. |
| REPO-CMP-002 | Agent instructions | Define role-specific AI workflow behavior. |
| REPO-CMP-003 | Shared SE docs | Define repository process, build, workflow, and traceability conventions. |
| REPO-CMP-004 | Crate SE docs | Define crate-specific engineering baselines. |
| REPO-CMP-005 | Verification commands | Provide repeatable evidence for build and test status. |
