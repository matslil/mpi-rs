# Compile-Time Task Topology

This document is part of the authoritative systems-engineering baseline named `se-*.md`.

Scope: `crates/mpi`.

It records a human-approved design restriction for task instance topology: tasks that participate in the `mpi-rs` task system must be known at compile time. This restriction is intended to make generated task routing, task-local suspension, waiter matching, and compile-time receive checks simpler and more Rust-like.

## Scope

This document constrains task *instances* and task groups, not only task *types*.

The existing baseline already requires each task to declare at compile time which messages it can receive. This addendum adds the complementary rule that the set and shape of task instances in an `mpi-rs` system shall be statically known to generated code.

## Requirements

### REQ-124: Compile-time task topology

Task instances that participate in `mpi-rs` task-to-task message passing shall be declared in a topology known to generated code at compile time.

Rationale: A closed task topology allows generated handles, reply routes, suspension points, and receive capabilities to be represented using concrete Rust types instead of runtime task registration and dynamic lookup.

Source: Human maintainer decision, SN-011, SN-012, SN-015, SN-023, SN-042.

Verification: inspection.

Status: approved.

### REQ-125: Const-sized task arrays

When an `mpi-rs` topology contains multiple task instances of the same task type, the number of instances shall be represented by a compile-time-known array size or equivalent const generic value.

Rationale: A fixed-size worker group preserves the ability to generate concrete topology fields, routing, and receive-check evidence while still supporting repeated worker tasks.

Source: Human maintainer decision, SN-011, SN-012, SN-015, SN-023.

Verification: inspection.

Status: approved.

### REQ-126: No runtime-discovered task participation

The core `mpi-rs` task model shall not require runtime-discovered task instances to participate in task-internal call, stream, reply, or suspension routing.

Rationale: Runtime-discovered task participation would require dynamic registration and type-erased routing that weakens the compile-time receive-check and suspension design. External code may still hold generated handles, but task-to-task topology for generated task-internal APIs remains closed-world.

Source: Human maintainer decision, SN-011, SN-015, SN-023, SN-042.

Verification: inspection.

Status: approved.

## Architecture

### Topology model

Generated system topology may be represented conceptually as ordinary Rust structs and const-sized arrays:

```rust
struct SystemTopology {
    client: ClientRuntime,
    database: DatabaseRuntime,
    workers: [WorkerRuntime; 8],
}
```

The exact generated type names are implementation details. The architectural requirement is that the topology shape is statically available to macro/runtime integration.

### Architecture rules

ARCH-086: `mpi-rs` shall treat the task-to-task topology as closed-world for generated task-internal APIs.

ARCH-087: Generated topology support may represent repeated tasks using arrays whose length is known at compile time.

ARCH-088: Task routing, reply routing, waiter registration, and compile-time receive checks should prefer concrete generated topology types over runtime task registries.

ARCH-089: The core suspension design should not depend on dynamically creating task participants after topology generation.

## Interfaces

### Example task group shape

A fixed worker group should use a compile-time-known size:

```rust
const WORKERS: usize = 8;

struct WorkerGroup {
    workers: [WorkerHandle; WORKERS],
}
```

A dynamically sized task group is outside the core topology model:

```rust
struct DynamicWorkerGroup {
    workers: Vec<WorkerHandle>, // not part of generated task-internal topology
}
```

External application code may still own ordinary Rust collections of handles for non-generated orchestration, but such collections are not the source of task-internal compile-time receive and suspension routing.

### Interface rules

INT-096: Public topology declarations shall make the number and type of task instances visible to generated code.

INT-097: Public topology declarations may support const generics for repeated task instances.

INT-098: Task-internal generated call and stream APIs shall be designed against the closed-world topology model.

INT-099: Runtime `Vec`-like task collections shall not be required for the core task-to-task suspension model.

## Traceability

| Need | Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|---|
| SN-011 | REQ-124, REQ-125, REQ-126 | ARCH-086, ARCH-087, ARCH-088 | INT-096, INT-097 | VAL-001, VAL-006 |
| SN-012 | REQ-124, REQ-125 | ARCH-086, ARCH-088 | INT-096, INT-098 | VAL-003 |
| SN-015 | REQ-124, REQ-126 | ARCH-086, ARCH-088, ARCH-089 | INT-098, INT-099 | VAL-004, VAL-011 |
| SN-023 | REQ-124, REQ-125, REQ-126 | ARCH-088 | INT-096, INT-098 | VAL-006 |
| SN-042 | REQ-124, REQ-126 | ARCH-088, ARCH-089 | INT-098 | VAL-004, VAL-005 |

## System Engineering Agent Report

### Summary

The compile-time task topology restriction is consistent with the existing baseline requirement that tasks declare received messages at compile time. It narrows the system model from dynamically discovered actor sets to statically generated task topologies.

### Documents inspected

- `AGENTS.md`
- `docs/agents/process.md`
- `docs/agents/system-engineering-agent.md`
- `docs/se-stakeholders.md`
- `docs/se-requirements.md`
- `docs/se-architecture.md`
- `docs/se-interfaces.md`
- `docs/se-validation-scenarios.md`
- `docs/se-traceability.md`
- `docs/se-glossary.md`

### Consistency findings

| Severity | Area | Finding | Affected IDs | Recommended action |
|---|---|---|---|---|
| note | requirements | Existing REQ-025 covers compile-time message receive declarations, but not compile-time task instance topology. | REQ-025, REQ-124 | Add topology-specific requirements rather than changing REQ-025. |
| note | architecture | Existing selective receive and waiter matching rules benefit from closed-world topology but did not require it. | ARCH-040..ARCH-043, ARCH-086..ARCH-089 | Use the topology addendum to guide future suspension implementation. |
| note | interfaces | Existing generated handle APIs remain compatible; dynamic collections of handles are merely outside the generated task-internal topology model. | INT-020..INT-025, INT-096..INT-099 | Keep external orchestration separate from generated task-internal routing. |

### Traceability impact

This crate-level addendum includes local traceability rows for REQ-124 through REQ-126. Workspace-level traceability should continue to index these requirements when they affect cross-document review.
