# Shared Traceability Conventions

This document defines repository-level traceability conventions.

Crate-specific traceability belongs in crate-local `se-*.md` files. Shared traceability should point to the owning crate baseline rather than duplicating crate behavior.

## Status Values

Use the status values from `docs/agents/process.md`:

- proposed;
- approved;
- implemented;
- verified;
- validated;
- deferred;
- obsolete.

## Repository Traceability

| Shared requirement | Architecture | Interface | Validation | Evidence |
|---|---|---|---|---|
| REQ-001 / REPO-REQ-001 | REPO-ARCH-001 | `docs/se-index.md` | REPO-VAL-001 | inspection |
| REPO-REQ-002 | REPO-ARCH-002 | REPO-INT-002 | REPO-VAL-002 | inspection |
| REPO-REQ-003 | REPO-ARCH-003, REPO-ARCH-004 | REPO-INT-003 | REPO-VAL-001 | crate-local `se-*.md` |
| REPO-REQ-004 | REPO-ARCH-003 | REPO-INT-003 | REPO-VAL-001 | crate-local `se-*.md` |
| REQ-002 / REPO-REQ-005 | REPO-CMP-002 | agent reports | REPO-VAL-001 | `docs/agents/*` |
| REQ-003 / REPO-REQ-006 | REPO-CMP-001 | PR evidence | REPO-VAL-001 | human review |
| REQ-150, REQ-151 | REPO-CMP-003, REPO-CMP-004 | crate baseline index | REPO-VAL-001 | `docs/se-index.md` |
| REPO-REQ-010 | REPO-CMP-005 | command reports | REPO-VAL-001 | `docs/se-verification-plan.md` |
| REPO-REQ-011 | REPO-CMP-005 | agent and PR reports | REPO-VAL-001 | command evidence |
| REPO-REQ-020 | REPO-CMP-003, REPO-CMP-004 | traceability tables | REPO-VAL-001 | this document and crate baselines |
| REPO-REQ-021 | REPO-CMP-003, REPO-CMP-004 | stable IDs | REPO-VAL-001 | inspection |
| REPO-REQ-022 | REPO-ARCH-006 | REPO-INT-004 | REPO-VAL-002 | pending human approval |
| REPO-REQ-023 | REPO-ARCH-007 | REPO-INT-005 | REPO-VAL-002 | pending human approval |

## Crate Baseline Index

| Crate | Baseline |
|---|---|
| `ctx-future` | `crates/ctx-future/se-design-baseline.md` |
| `mpi` | `crates/mpi/se-design-baseline.md`, `crates/mpi/se-send-scope.md`, `crates/mpi/se-task-topology.md` |
| `mpi-macros` | `crates/mpi-macros/se-design-baseline.md` |
| `mpi-os-events` | `crates/mpi-os-events/se-design-baseline.md` |
| `mpi-transaction` | `crates/mpi-transaction/se-design-baseline.md` |
| `persistent-log-storage-service` | `crates/persistent-log-storage-service/se-design-baseline.md` |
| `timeout-service` | `crates/timeout-service/se-design-baseline.md` |

## Maintenance Rules

- Add or update crate-local traceability when crate requirements, architecture, interfaces, tests, or validation scenarios change.
- Do not duplicate crate-specific requirement rows in shared traceability.
- Keep IDs stable once referenced by tests, code comments, reports, or other documents.
- Mark obsolete rows explicitly before removal.
