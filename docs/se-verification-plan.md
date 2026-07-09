# Shared Verification Plan

This document defines repository-level verification workflow conventions.

Crate-specific verification expectations belong in crate-local `se-*.md` files.

## Verification Methods

| Method | Meaning |
|---|---|
| test | Automated Rust unit, integration, property, doctest, or compile-fail test. |
| inspection | Review of source code, generated code, macro output, documentation, or configuration. |
| analysis | Reasoned argument, static analysis, model checking, or proof-like evidence. |
| demonstration | Executable example or manual scenario showing behavior. |

## Required Command Baseline

For implementation and test changes, run the strongest applicable subset of:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --doc
```

If available later, also consider:

```sh
cargo nextest run
cargo miri test
cargo deny check
cargo audit
```

If a command cannot be run, the agent report shall explain why.

## Evidence Rules

REPO-VER-001: Every approved requirement with `Verification: test` shall have at least one automated test or an explicit documented gap.

REPO-VER-002: Requirement-verifying tests shall reference the relevant requirement ID in the test name, module name, comment, or traceability matrix.

REPO-VER-003: Inspection evidence shall identify the inspected file, interface, macro expansion, or document section.

REPO-VER-004: Demonstration evidence shall identify the example, command, and expected result.

REPO-VER-005: Analysis evidence shall state assumptions, reasoning, and limitations.

REPO-VER-006: Verification reports shall identify the crate-local baseline used for crate-specific verification.

## Report Template

```markdown
# Verification Report

## Summary

## Baselines inspected

## Commands run

## Requirement verification status

| Requirement | Method | Evidence | Result | Notes |
|---|---|---|---|---|

## Failing or blocked verification

## Deferred verification

## Human decisions needed
```
