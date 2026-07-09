# Systems Engineering Index

This file is the entry point for the authoritative systems-engineering baseline.

Current systems-engineering documents are named `se-*.md`. Workspace-level documents live under `docs/`. Crate-level documents live beside the crate they describe. Module-level documents may live beside the module they describe.

## Workspace-Level Baseline

- `docs/se-index.md`
- `docs/se-stakeholders.md`
- `docs/se-requirements.md`
- `docs/se-architecture.md`
- `docs/se-protocols.md`
- `docs/se-interfaces.md`
- `docs/se-verification-plan.md`
- `docs/se-validation-scenarios.md`
- `docs/se-traceability.md`
- `docs/se-glossary.md`
- `docs/se-change-process.md`
- `docs/se-verification-report.md`

## Crate-Level Baseline

- `crates/ctx-future/se-design-baseline.md`
- `crates/mpi/se-send-scope.md`
- `crates/mpi/se-task-topology.md`

## Scope Rules

- A workspace-level SE document applies to the whole repository or to behavior spanning multiple crates.
- A crate-level SE document applies to the crate directory that contains it.
- A module-level SE document applies to the module directory that contains it.
- If a requirement affects more than one crate, define or index it at workspace level and trace it to each affected crate.
- If two current SE documents conflict, report the conflict instead of guessing.
