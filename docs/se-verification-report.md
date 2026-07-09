# Verification Report

This shared report records repository-level verification evidence conventions and the last known command baseline.

Crate-specific verification status belongs in crate-local baselines or crate-specific reports.

## Commands Previously Reported

The following commands were previously reported as passing from the workspace root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check -p mpi --no-default-features
cargo check -p mpi-os-events --no-default-features
```

This document does not claim current command status. Agents shall rerun applicable commands for new changes and report fresh results.

## Shared Verification Status

| Area | Method | Evidence | Status |
|---|---|---|---|
| Shared documentation boundary | inspection | `docs/se-*.md`, crate-local `se-*.md` | documented |
| Agent process | inspection | `docs/agents/*`, `AGENTS.md` | documented |
| Command reporting convention | inspection | `docs/se-verification-plan.md` | documented |

## Deferred Verification

Fresh command execution is required for each non-trivial change.
