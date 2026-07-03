# Message Model Replacement Readiness

This note records whether the current `docs/se/` baseline is complete enough to replace the earlier standalone message-model design note.

## Decision

`docs/se/` is now complete enough to act as the authoritative baseline for the message model.

The old standalone message-model document may be deleted or moved to a historical/archive location once the maintainer is comfortable doing so.

## What was checked

The following message-model areas are represented in the current SE baseline:

| Area | SE location |
|---|---|
| Project purpose and OSE-inspired Rust API intent | `docs/se/stakeholders.md`, `docs/se/requirements.md`, `docs/se/architecture.md` |
| Message, event, call, stream, and `SessionId` terminology | `docs/se/glossary.md`, `docs/se/requirements.md` |
| Task model and generated task artifacts | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/interfaces.md` |
| Normal and priority queue placement | `docs/se/requirements.md`, `docs/se/architecture.md` |
| Start-message guarantee | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/validation-scenarios.md` |
| Task macro declaration style | `docs/se/interfaces.md` |
| TaskMessage and MessagePlacement concepts | `docs/se/interfaces.md`, `docs/se/architecture.md` |
| Handler execution and selective receive | `docs/se/requirements.md`, `docs/se/architecture.md` |
| Compile-time receive checks | `docs/se/requirements.md`, `docs/se/interfaces.md`, `docs/se/verification-plan.md` |
| `SessionId`, `Response<T>`, `HasSessionId` | `docs/se/requirements.md`, `docs/se/interfaces.md`, `docs/se/glossary.md` |
| Synchronous call behavior and out-of-order response matching | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/verification-plan.md` |
| Late one-shot response handling | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/interfaces.md` |
| Streaming calls, batches, end, error, cancellation | `docs/se/requirements.md`, `docs/se/interfaces.md`, `docs/se/validation-scenarios.md` |
| Stream producer behavior and flow control | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/interfaces.md` |
| Future `futures_core::Stream` compatibility constraint | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/interfaces.md` |
| External blocking callers | `docs/se/requirements.md`, `docs/se/interfaces.md`, `docs/se/validation-scenarios.md` |
| Unix signal bridge | `docs/se/requirements.md`, `docs/se/architecture.md`, `docs/se/interfaces.md` |
| Suggested crate structure and implementation phases | `docs/se/architecture.md`, `docs/se/requirements.md` |
| AI/system-engineering workflow | `AGENTS.md`, `docs/agents/`, `docs/se/change-process.md` |

## Updates made during replacement check

The replacement check found that the content coverage was mostly present, but not yet self-contained.

The following issues were corrected:

- `AGENTS.md` no longer names the old message-model document as the source of truth.
- `docs/agents/process.md` now makes `docs/se/` the authoritative baseline.
- Agent role documents no longer require the old message-model document as a normal input.
- `docs/se/stakeholders.md` now says `docs/se/` is authoritative.
- `docs/se/requirements.md` now makes `docs/se/` authoritative and references `docs/se/architecture.md` for implementation phases and crate structure.
- `docs/se/architecture.md` and `docs/se/interfaces.md` no longer describe themselves as merely derived from the old message-model note.
- Requirements were added for late one-shot response handling, producer suspension under flow control, and possible future `futures_core::Stream` compatibility.
- Verification planning was updated for the new requirements.

## Remaining notes

The old message-model document may still be useful as historical design context, but it should not be treated as normative after these changes.

If it is kept, recommended locations are:

- `docs/archive/message-model.md`; or
- `docs/history/message-model.md`.

If it is deleted, the SE baseline should be the only authoritative message-model source.
