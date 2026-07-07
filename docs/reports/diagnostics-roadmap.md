# Diagnostics Roadmap

## Summary

REQ-140 requires the implementation to include or preserve a roadmap for
diagnostics, timeouts, tracing, and deadlock/debug support. This report records
the current roadmap and the implementation hooks that already preserve room for
that work.

This is roadmap evidence only. It does not claim that timeout handling,
structured tracing, deadlock detection, or full queue/session diagnostics are
implemented.

## Current Diagnostic Hooks

The current runtime already exposes or preserves these diagnostic anchors:

- `SessionId` contains an origin endpoint and sequence, and implements
  `Display`, `Debug`, equality, ordering, and hashing.
- `EndpointId` identifies task and external-call origins.
- `Response<T>`, `StreamEvent<T, E>`, `StreamPull`, and `StreamCancel` carry
  session IDs.
- `LateReplyRef<'_>` exposes the late reply session ID, late-reply kind, and
  read-only downcast access to the payload for task-local diagnosis.
- `LateReplyAction` lets a task continue or terminate after a reported late
  reply.
- `SendError`, `RecvError`, and `CallError` distinguish queue-full, stopped
  task, stream flow-limit, stream cancellation, closed receive, disconnected
  reply, and unexpected reply type conditions.
- `TaskQueue` exposes capacity, length, empty, and full state.
- `TaskContext` tracks task-local call waiters, stream waiters, stream credit,
  released calls, and stopped state.

## Roadmap Slices

### Queue Diagnostics

Future queue diagnostics should expose a read-only snapshot for a task queue:

- configured capacity;
- current total length;
- current normal and priority lengths;
- closed or accepting state.

The snapshot should not let callers mutate queue internals or choose placement
at send sites.

### Session Diagnostics

Future session diagnostics should use `SessionId` as the stable key and expose:

- origin endpoint;
- sequence number;
- protocol kind, such as call or stream;
- active, completed, cancelled, late, or released state;
- optional task-local waiter state.

Diagnostics must not require user code to construct protocol messages manually.

### Late Reply Diagnostics

Late reply reporting already routes unexpected call responses and stream events
through `LateReplyRef<'_>` unless the declaration opts into
`late_reply = "ignore"`.

Future diagnostics may add structured logging or counters around this path, but
ordinary application messages must remain outside the late-reply discard policy.

### Timeout Support

Future timeout support should be expressed as a defined call or stream failure
outcome, not as a silent dropped message. A timed-out interaction should release
its waiter and rely on the late-reply policy if a response arrives afterward.

The design should preserve the distinction between task-internal suspension and
external blocking calls.

### Deadlock and Blocked-Progress Debugging

Future blocked-progress diagnostics should inspect task-local waiters, queued
message counts, and session state. The first useful milestone is likely a
read-only debug snapshot rather than automatic deadlock recovery.

Any future detection should respect the current task model: handlers waiting for
protocol messages should suspend; task threads should not be blocked by
task-internal call or stream waits.

### Tracing

Future tracing should be keyed by `EndpointId` and `SessionId` and should cover:

- enqueue attempt and result;
- dequeue and dispatch;
- session allocation;
- call response routing;
- stream batch, end, error, pull, and cancel events;
- late reply reporting or explicit ignore.

Tracing should be optional and should not add a required runtime dependency
without a documented human-approved decision.

## Deferred Work

The following remain implementation gaps:

- public diagnostic snapshot types;
- timeout APIs and timeout tests;
- tracing integration;
- deadlock or blocked-progress analysis;
- validation evidence for VAL-013 beyond late-reply handling and explicit error
  categories.

## Human Decisions Needed

- Choose the public shape, if any, for queue and task diagnostic snapshots.
- Decide whether tracing should use `tracing`, a feature-gated dependency, or a
  dependency-free callback interface.
- Define timeout syntax for generated call and stream APIs before implementing
  timeout behavior.
