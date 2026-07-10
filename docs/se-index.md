# Systems Engineering Index

This file is the entry point for the authoritative systems-engineering baseline.

Current systems-engineering documents are named `se-*.md`. Workspace-level documents live under `docs/`. Crate-level documents live beside the crate they describe. Module-level documents may live beside the module they describe.

## Workspace-Level Baseline

Workspace-level documents cover repository builds, process, workflows, change control, verification/reporting conventions, and traceability conventions. They do not define crate behavior.

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
- `crates/mpi/se-design-baseline.md`
- `crates/mpi-macros/se-design-baseline.md`
- `crates/mpi-os-events/se-design-baseline.md`
- `crates/mpi-transaction/se-design-baseline.md`
- `crates/persistent-log-storage-service/se-design-baseline.md`
- `crates/timeout-service/se-design-baseline.md`
- `crates/mpi/se-send-scope.md`
- `crates/mpi/se-task-topology.md`

## Scope Rules

- A workspace-level SE document applies only to builds, process, workflows, change control, verification/reporting conventions, and traceability conventions.
- A crate-level SE document applies to the crate directory that contains it.
- A module-level SE document applies to the module directory that contains it.
- If a product change affects more than one crate, document each crate's responsibilities in that crate's own baseline and use shared docs only for workflow coordination.
- If two current SE documents conflict, report the conflict instead of guessing.
