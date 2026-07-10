# Shared Process Glossary

This glossary defines repository-level process terms.

Crate-specific terminology belongs in crate-local `se-*.md` files.

### Requirement

A testable or inspectable obligation stated with `shall`.

### Architecture

The structural decisions and component responsibilities for a documented scope.

### Interface

A public API, internal API, workflow contract, report shape, or command interface for a documented scope.

### Verification

Evidence that the implementation satisfies approved requirements.

### Validation

Evidence that the implementation satisfies stakeholder needs and intended usage scenarios.

### Traceability

Links between needs, requirements, architecture, interfaces, implementation, tests, verification evidence, and validation scenarios.

### Human system engineer

The human maintainer who defines and approves requirements, architecture, interfaces, process rules, release decisions, and merge decisions.

### Agent

An AI workflow with a defined role, inputs, outputs, permissions, and completion criteria.

### Shared documentation

Repository-level documentation under `docs/` that covers builds, process, workflows, change control, traceability conventions, and release evidence.

### Crate-local documentation

Systems-engineering documentation beside a crate that defines that crate's product behavior, interfaces, verification expectations, validation scenarios, and traceability.

### Service

A repository-level product concept for a long-lived message-based capability
provided by one or more crate-local task implementations. Crate-local baselines
define the concrete service behavior.

### Service instance

The owning Rust object returned when a service is started. Crate-local
baselines define which protocol bindings it exposes and how it owns the
service lifetime.

### Message handler

A function that handles an `mpi` message. Runtime and macro details belong to
the affected crate-local baselines.

### Callback

A function invoked by runtime or event machinery outside ordinary message
dispatch.
