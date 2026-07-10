# Shared Repository Requirements

This document defines repository-level requirements for builds, process, and AI-assisted workflows.

Shared documentation shall not define crate behavior, crate architecture, crate interfaces, or crate validation scenarios. Crate-specific requirements live in crate-local `se-*.md` files beside the crate they describe.

## Process Requirements

### REQ-001 / REPO-REQ-001: Authoritative SE baseline

The repository shall treat current systems-engineering documents named `se-*.md` as the authoritative engineering baseline for their declared scope.

Verification: inspection

Status: approved

### REPO-REQ-002: Scope separation

Workspace-level `docs/se-*.md` files shall cover only repository builds, process, workflows, change control, verification reporting conventions, and traceability conventions.

Verification: inspection

Status: approved

### REPO-REQ-003: Crate-local product baselines

Requirements, architecture, interfaces, verification expectations, and validation scenarios for a crate shall live in that crate's own `se-*.md` files.

Verification: inspection

Status: approved

### REPO-REQ-004: Self-contained crate documentation

Each crate's systems-engineering documentation shall be sufficient to understand that crate's purpose, scope, requirements, architecture, interfaces, verification expectations, and traceability without relying on product requirements from shared documentation.

Verification: inspection

Status: approved

### REQ-002 / REPO-REQ-005: Agent role separation

The repository shall define separate instructions for system engineering, implementation, testing, review, validation, traceability, and release activities.

Verification: inspection

Status: approved

### REQ-003 / REPO-REQ-006: Human approval

The process shall require human maintainer approval before merge to `main`.

Verification: inspection

Status: approved

## Build Requirements

### REQ-150: Incremental implementation order

Implementation should follow the phase order approved in the affected crate-local baseline unless the human maintainer approves a different order.

Verification: inspection

Status: approved

### REQ-151: Suggested crate structure

The repository should organize crates along the conceptual crate structure indexed in `docs/se-index.md`, unless implementation evidence justifies a different structure.

Verification: inspection

Status: approved

### REPO-REQ-010: Common Rust verification commands

The repository workflow shall identify the common Rust verification commands expected for non-trivial implementation and test changes.

Verification: inspection

Status: approved

### REPO-REQ-011: Command-result reporting

Agent reports and pull requests shall state which verification commands were run, their result, and any command that could not be run.

Verification: inspection

Status: approved

## Traceability Requirements

### REPO-REQ-020: Traceability conventions

The shared traceability documentation shall define conventions for maintaining traceability, while crate-local baselines shall own crate-specific traceability rows.

Verification: inspection

Status: approved

### REPO-REQ-021: Stable IDs

Requirement, architecture, interface, verification, and validation IDs shall not be reused for unrelated meanings after they appear in tests, code comments, reports, or traceability.

Verification: inspection

Status: approved

### REPO-REQ-022: Cross-crate message boundaries

Interfaces between crates shall be message based unless the human maintainer explicitly approves and documents a non-message-based interface in the affected crate-local baselines.

Verification: inspection

Status: proposed

### REPO-REQ-023: Optional crate feature naming

Repository-owned optional crates and services shall use Cargo feature names of
the form `enable-<crate-name>`.

Verification: inspection

Status: proposed
