# Shared Workflow Interfaces

This document describes repository-level workflow interfaces. It intentionally does not define crate APIs.

## Agent Report Interface

Non-trivial agent reports should identify:

- documents inspected;
- files changed;
- affected crate-local requirements;
- verification commands run;
- verification result;
- validation or traceability impact;
- risks, gaps, or human decisions needed.

## Pull Request Evidence Interface

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

## Shared Interface Rules

REPO-INT-001: Shared workflow documents may define report, pull request, change request, release, and traceability templates.

REPO-INT-002: Shared workflow documents shall not define crate public APIs, crate internal APIs, macro syntax, runtime types, platform bridge APIs, or examples whose purpose is to specify crate behavior.

REPO-INT-003: Crate-local documentation shall define the API and interface rules for the crate it describes.

REPO-INT-004: Crate-local interface rules for interfaces between crates shall specify message-based interaction unless the human maintainer explicitly approves and documents another interface style.

REPO-INT-005: Repository-owned optional crate and service feature names shall
use the literal form `enable-<crate-name>`, where `<crate-name>` is the Cargo
package name defined by the affected crate.
