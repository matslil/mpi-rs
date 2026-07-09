# Verification Plan

This document defines how `mpi-rs` requirements are verified.

Verification answers: did we build the system correctly according to the approved requirements?

## Verification methods

Use these methods:

| Method | Meaning |
|---|---|
| test | Automated Rust unit, integration, property, doctest, or compile-fail test. |
| inspection | Review of source code, generated code, macro output, documentation, or configuration. |
| analysis | Reasoned argument, static analysis, model checking, or proof-like evidence. |
| demonstration | Executable example or manual scenario showing the behavior. |

## Required command baseline

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

## Verification evidence rules

VER-001: Every approved requirement with `Verification: test` shall have at least one automated test or an explicit documented gap.

VER-002: Requirement-verifying tests shall reference the relevant requirement ID in the test name, module name, comment, or traceability matrix.

VER-003: Inspection evidence shall identify the inspected file, interface, macro expansion, or document section.

VER-004: Demonstration evidence shall identify the example, command, and expected result.

VER-005: Analysis evidence shall state assumptions, reasoning, and limitations.

## Verification categories

### Process verification

Process requirements such as source-of-truth handling, role separation, and human approval are verified by inspection of repository files and pull request process evidence.

Relevant requirements:

- REQ-001
- REQ-002
- REQ-003

### Queue verification

Queue tests should verify:

- static capacity;
- shared capacity across normal and priority queues;
- explicit queue-full error;
- receiver-owned queue-capacity reservation after queue-full task-internal sends;
- FIFO sender registration with at most one reservation per sender task;
- reservation consumption before unreserved capacity for the reserved sender;
- failed queue-space wakeup delivery releases the reservation;
- configurable priority-reserved capacity, including the default one-slot reservation;
- normal messages cannot consume priority-reserved capacity;
- priority messages can use priority-reserved capacity;
- FIFO order among normal messages;
- FIFO order among priority messages;
- priority-before-normal receive order;
- receiver-declared placement.

Relevant requirements:

- REQ-012
- REQ-013
- REQ-014
- REQ-014A
- REQ-014B
- REQ-014C
- REQ-014D
- REQ-014E
- REQ-030
- REQ-032
- REQ-033
- REQ-034
- REQ-036
- REQ-037

### Start message verification

Start-message tests should verify:

- task creation enqueues a start message;
- start message is forced priority;
- start message is received before other application messages;
- explicit normal placement for start is rejected or forced to priority.

Relevant requirements:

- REQ-040
- REQ-041
- REQ-042
- REQ-053

### Macro and dispatch verification

Macro and dispatch verification should inspect or test:

- generated message enums;
- generated contexts;
- generated task handles;
- generated send methods;
- placement implementation;
- dispatch routing from enum variants to handlers;
- generated call and stream plumbing.

Relevant requirements:

- REQ-026
- REQ-027
- REQ-028
- REQ-050
- REQ-051
- REQ-052

### Protocol verification

Protocol verification should inspect:

- namespace-qualified protocol identities;
- protocol message identities;
- explicit payload, reply, stream item, and stream error type declarations;
- receive declarations matching protocol-qualified identities and declared types;
- generated send surfaces derived from protocol declarations or protocol-instance bindings;
- append-only compatibility rules;
- incompatible-change guidance requiring a new protocol name;
- compile-time receive checks derived from protocol-declared reply and stream event types.

Relevant requirements:

- REQ-160
- REQ-161
- REQ-162
- REQ-163
- REQ-164
- REQ-165
- REQ-166
- REQ-167
- REQ-168
- REQ-169
- REQ-170

### Handler and selective receive verification

Handler tests should verify:

- handler declarations support the approved task syntax while preserving
  suspending behavior;
- waiting for replies or stream events suspends the handler continuation;
- the task continues to receive other messages while a handler is suspended;
- waiter matching uses message kind and `SessionId`.
- suspended continuations do not retain mutable borrows of task state or task context while pending.

Relevant requirements:

- REQ-060
- REQ-061
- REQ-062
- REQ-063
- REQ-064

### Context future verification

Context future tests should verify:

- a ctx-future receives mutable context only during resume;
- context can be mutably used after a ctx-future returns pending;
- multiple pending ctx-futures can be stored while the caller continues using context;
- a pending ctx-future can later be resumed with context;
- `ctx-future` can be built and tested without depending on `mpi-rs`;
- ordinary public API use does not require `unsafe`.

Relevant requirements:

- REQ-064
- REQ-152

### Compile-time receive verification

Compile-time receive checks should use compile-fail or trybuild-style tests once a test framework is chosen.

Tests should show that:

- callers can await declared responses;
- callers cannot await undeclared responses;
- callers can await declared stream events;
- callers cannot await undeclared stream events.

Relevant requirements:

- REQ-070
- REQ-071
- REQ-072

### Session and call verification

Session and call tests should verify:

- session IDs identify logical interactions;
- session sequence allocation does not need global synchronization;
- responses carry the correct session ID;
- a call has exactly one response or defined failure outcome;
- concurrent same-type calls with out-of-order replies resume the correct handler;
- late one-shot responses invoke the task late-reply handler by default;
- the default late-reply handler returns ignore;
- a late-reply handler receives a borrowed reply reference and may return terminate;
- one-shot responses declared with `late_reply = "ignore"` bypass the late-reply handler.
- task-internal call responses suspend through receiver-owned queue-capacity reservations when the caller queue has no available capacity.

Relevant requirements:

- REQ-080
- REQ-081
- REQ-082
- REQ-083
- REQ-084
- REQ-090
- REQ-091
- REQ-092
- REQ-093
- REQ-094
- REQ-095
- REQ-096
- REQ-097
- REQ-098

### Stream verification

Stream tests should verify:

- batch, end, and error event handling;
- one-item-at-a-time public `next(ctx).await` behavior;
- local buffer draining before awaiting another batch;
- cancellation on dropped unfinished stream;
- late stream replies invoke the task late-reply handler by default;
- stream replies declared with `late_reply = "ignore"` bypass the late-reply handler;
- no silent discard of ordinary application messages;
- producer remains within task model;
- automatic end and error events;
- credit-based flow control or documented interim limitation;
- producer suspension under backpressure or stream-control waiting;
- producer-side native stream yield operations return context to the task-local runtime after sending stream replies;
- producer-side `yield_item()` and `yield_batch()` suspend under no-credit backpressure rather than requiring user retry loops;
- stream item, end, and error replies suspend through receiver-owned queue-capacity reservations when the consumer queue has no available capacity;
- future standard stream compatibility does not violate task-local receive-state constraints.

Relevant requirements:

- REQ-100
- REQ-101
- REQ-102
- REQ-103
- REQ-104
- REQ-105
- REQ-106
- REQ-107
- REQ-095
- REQ-096
- REQ-097
- REQ-108
- REQ-109
- REQ-110
- REQ-111
- REQ-112
- REQ-113
- REQ-115
- REQ-116
- REQ-114

### External caller verification

External caller verification should demonstrate explicit blocking APIs and inspect that task-internal APIs remain distinct.

Relevant requirements:

- REQ-120
- REQ-121
- REQ-122
- REQ-123

### Task topology verification

Task topology verification should inspect that generated task-to-task APIs use a closed-world topology known to generated code. Repeated task instances should use const-sized arrays or equivalent compile-time-known shapes rather than runtime-discovered task participants for task-internal call, stream, reply, or suspension routing.

Relevant requirements:

- REQ-124
- REQ-125
- REQ-126

### Unix signal verification

Unix signal support verification should inspect that signal handlers perform only async-signal-safe operations and that normal message construction occurs in a bridge task or thread. It should also verify that the signal bridge API is feature-gated behind a default-enabled optional feature.

Relevant requirements:

- REQ-130
- REQ-131
- REQ-132

### Diagnostics verification

Diagnostics are later-phase work. Verification should inspect roadmap and eventually test tracing, timeouts, deadlock/debug support, and late protocol-message reporting.

Relevant requirements:

- REQ-140

## Initial verification report template

Use this format in reports:

```markdown
# Verification Report

## Summary

## Commands run

## Requirement verification status

| Requirement | Method | Evidence | Result | Notes |
|---|---|---|---|---|

## Failing or blocked verification

## Deferred verification

## Human decisions needed
```

## Minimum evidence for early implementation phases

For early queue and task-handle work, the minimum evidence should include:

- queue capacity test;
- normal FIFO test;
- priority FIFO test;
- priority-before-normal test;
- queue-full test;
- placement inspection or test;
- start-message-first test when task spawn exists.
