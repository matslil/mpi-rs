# Traceability Matrix

This document provides the initial traceability baseline for `mpi-rs`.

The matrix intentionally starts with many implementation and test entries marked `pending`, because this branch establishes the process and systems-engineering baseline rather than implementing runtime behavior.

## Traceability status values

Use the status values from `docs/agents/process.md`:

- proposed;
- approved;
- implemented;
- verified;
- validated;
- deferred;
- obsolete.

## High-level traceability

| Need | Requirements | Architecture | Interfaces | Validation |
|---|---|---|---|---|
| SN-001 | REQ-001, REQ-002, REQ-003 | process | AGENTS.md, docs/agents/* | VAL-014 |
| SN-002 | REQ-002 | process | docs/agents/* | VAL-014 |
| SN-003 | REQ-003 | process | docs/agents/process.md | VAL-014 |
| SN-004 | REQ-001 | process | AGENTS.md | VAL-014 |
| SN-010 | REQ-050, REQ-101, REQ-120 | ARCH-001 | INT-001, INT-010, INT-090 | VAL-001, VAL-007, VAL-011 |
| SN-011 | REQ-025, REQ-026, REQ-070, REQ-071 | ARCH-011, CMP-011 | INT-040, INT-041, INT-042 | VAL-001, VAL-006 |
| SN-012 | REQ-027, REQ-028, REQ-051 | CMP-002, CMP-012 | INT-020, INT-021, INT-022 | VAL-001, VAL-003 |
| SN-013 | REQ-021, REQ-022, REQ-023, REQ-024, REQ-090, REQ-100 | CMP-009, CMP-010 | INT-013, INT-014, INT-015 | VAL-004, VAL-007 |
| SN-014 | REQ-012, REQ-013, REQ-014 | CMP-003, ARCH-020..ARCH-024 | INT-021, INT-100 | VAL-003 |
| SN-015 | REQ-060, REQ-061, REQ-062, REQ-121 | CMP-006, CMP-007 | INT-004, INT-025, INT-092 | VAL-004, VAL-011 |
| SN-016 | REQ-100..REQ-105, REQ-112 | ARCH-070..ARCH-076 | INT-070..INT-081 | VAL-007, VAL-009 |
| SN-017 | REQ-106, REQ-107 | ARCH-073, ARCH-074 | INT-073, INT-074 | VAL-008 |
| SN-020 | REQ-020..REQ-024 | glossary | docs/se/glossary.md | documentation review |
| SN-021 | REQ-051, REQ-110, REQ-151 | CMP-001..CMP-014 | INT-010..INT-111 | review evidence |
| SN-022 | REQ-150, REQ-151 | implementation phases | process | review evidence |
| SN-023 | REQ-070, REQ-071, REQ-072 | CMP-011 | INT-040..INT-042 | VAL-006 |
| SN-030 | VER-001..VER-005 | verification plan | docs/agents/test-agent.md | VAL-014 |
| SN-031 | validation scenarios | validation plan | docs/agents/validation-agent.md | VAL-001..VAL-014 |
| SN-032 | traceability matrix | process | docs/agents/traceability-agent.md | VAL-014 |
| SN-040 | REQ-030..REQ-035 | ARCH-020..ARCH-025 | INT-030..INT-032 | VAL-010 |
| SN-041 | REQ-040..REQ-043, REQ-053 | ARCH-030..ARCH-032 | INT-012, INT-017 | VAL-002 |
| SN-042 | REQ-080..REQ-092 | ARCH-050..ARCH-063 | INT-050..INT-062 | VAL-004, VAL-005 |
| SN-043 | REQ-100..REQ-112 | ARCH-070..ARCH-076 | INT-070..INT-081 | VAL-008, VAL-009 |
| SN-044 | REQ-130, REQ-131 | ARCH-090..ARCH-092 | INT-110, INT-111 | VAL-012 |
| SN-045 | REQ-082, REQ-140 | diagnostics | diagnostics interfaces pending | VAL-013 |

## Requirement-level traceability

| Requirement | Source need | Architecture | Interface | Implementation | Verification | Validation | Status |
|---|---|---|---|---|---|---|---|
| REQ-001 | SN-004 | process | AGENTS.md | AGENTS.md | inspection pending | VAL-014 | approved |
| REQ-002 | SN-002 | process | docs/agents/* | docs/agents/* | inspection pending | VAL-014 | approved |
| REQ-003 | SN-003 | process | docs/agents/process.md | pending | inspection pending | VAL-014 | approved |
| REQ-010 | SN-010, SN-040 | CMP-001 | task API pending | pending | test pending | VAL-001 | approved |
| REQ-011 | SN-040 | ARCH-010 | task API pending | pending | inspection pending | VAL-001 | approved |
| REQ-012 | SN-014 | ARCH-023 | INT-011 | pending | test pending | VAL-003 | approved |
| REQ-013 | SN-014 | ARCH-023 | queue API pending | pending | test pending | VAL-003 | approved |
| REQ-014 | SN-014 | CMP-003 | INT-021, INT-100 | pending | test pending | VAL-003 | approved |
| REQ-020 | SN-020 | glossary | glossary | docs/se/glossary.md | inspection pending | documentation review | approved |
| REQ-021 | SN-013 | message categories | INT-013..INT-015 | pending | inspection pending | VAL-003, VAL-004, VAL-007 | approved |
| REQ-022 | SN-013 | message categories | INT-013 | pending | inspection pending | VAL-003 | approved |
| REQ-023 | SN-013, SN-042 | CMP-009 | INT-014 | pending | test pending | VAL-004 | approved |
| REQ-024 | SN-013, SN-016 | CMP-010 | INT-015 | pending | test pending | VAL-007 | approved |
| REQ-025 | SN-011 | ARCH-011 | INT-040..INT-042 | pending | inspection pending | VAL-001, VAL-006 | approved |
| REQ-026 | SN-011 | ARCH-011 | INT-030 | pending | inspection pending | VAL-001 | approved |
| REQ-027 | SN-012 | CMP-002 | INT-020 | pending | demonstration pending | VAL-003 | approved |
| REQ-028 | SN-012 | CMP-002, CMP-003 | INT-022 | pending | inspection pending | VAL-003 | approved |
| REQ-030 | SN-040 | ARCH-024, ARCH-025 | INT-016, INT-031 | pending | inspection pending | VAL-010 | approved |
| REQ-032 | SN-040 | ARCH-020 | queue API pending | pending | test pending | VAL-010 | approved |
| REQ-033 | SN-040 | ARCH-021 | queue API pending | pending | test pending | VAL-010 | approved |
| REQ-034 | SN-040 | ARCH-022 | queue API pending | pending | test pending | VAL-010 | approved |
| REQ-040 | SN-041 | ARCH-030..ARCH-032 | INT-012 | pending | test pending | VAL-002 | approved |
| REQ-041 | SN-041 | ARCH-030 | INT-017 | pending | test pending | VAL-002 | approved |
| REQ-042 | SN-041 | ARCH-031, ARCH-032 | task spawn API pending | pending | test pending | VAL-002 | approved |
| REQ-050 | SN-010 | CMP-012 | INT-010 | pending | inspection pending | VAL-001 | approved |
| REQ-051 | SN-012, SN-021 | CMP-012 | INT-010..INT-025 | pending | inspection pending | VAL-001 | approved |
| REQ-052 | SN-012 | CMP-006 | generated dispatch | pending | test pending | VAL-003 | approved |
| REQ-060 | SN-015 | CMP-007 | async handlers | pending | demonstration pending | VAL-004 | approved |
| REQ-061 | SN-015 | ARCH-040, ARCH-041 | context receive API | pending | test pending | VAL-004 | approved |
| REQ-062 | SN-015 | ARCH-040..ARCH-043 | context receive API | pending | test pending | VAL-004 | approved |
| REQ-063 | SN-042 | ARCH-043 | HasSessionId | pending | test pending | VAL-005 | approved |
| REQ-070 | SN-023 | CMP-011 | INT-040 | pending | compile-fail test pending | VAL-006 | approved |
| REQ-071 | SN-023 | CMP-011 | INT-041 | pending | compile-fail test pending | VAL-006 | approved |
| REQ-072 | SN-023 | CMP-011 | INT-040..INT-042 | pending | inspection pending | VAL-006 | approved |
| REQ-080 | SN-042 | ARCH-050 | INT-050 | pending | inspection pending | VAL-004, VAL-005 | approved |
| REQ-081 | SN-042 | ARCH-051 | INT-050 | pending | inspection pending | VAL-004, VAL-005 | approved |
| REQ-082 | SN-042, SN-045 | ARCH-050 | INT-050, INT-052 | pending | inspection pending | VAL-004, VAL-005, VAL-013 | approved |
| REQ-083 | SN-042 | ARCH-052 | INT-050 | pending | inspection pending | VAL-005 | approved |
| REQ-084 | SN-042 | ARCH-053 | context API pending | pending | test pending | VAL-005 | approved |
| REQ-090 | SN-013, SN-042 | ARCH-061 | INT-060 | pending | inspection pending | VAL-004 | approved |
| REQ-091 | SN-013, SN-042 | ARCH-060..ARCH-063 | INT-060..INT-062 | pending | test pending | VAL-004 | approved |
| REQ-092 | SN-042 | ARCH-063 | INT-050..INT-062 | pending | test pending | VAL-005 | approved |
| REQ-100 | SN-016, SN-043 | ARCH-071 | INT-070 | pending | inspection pending | VAL-007 | approved |
| REQ-101 | SN-010, SN-016 | ARCH-072 | INT-070 | pending | demonstration pending | VAL-007 | approved |
| REQ-102 | SN-016 | ARCH-072 | INT-071 | pending | test pending | VAL-009 | approved |
| REQ-103 | SN-016 | ARCH-072 | INT-072 | pending | test pending | VAL-007 | approved |
| REQ-104 | SN-016, SN-043 | ARCH-071 | stream API | pending | test pending | VAL-007 | approved |
| REQ-105 | SN-016, SN-043 | ARCH-071 | stream API | pending | test pending | VAL-007 | approved |
| REQ-106 | SN-017, SN-043 | ARCH-073 | INT-073, INT-074 | pending | test pending | VAL-008 | approved |
| REQ-107 | SN-017, SN-043 | ARCH-074 | stream cancel API | pending | inspection pending | VAL-008 | approved |
| REQ-108 | SN-043 | late stream handling | stream receive API | pending | test pending | VAL-008 | approved |
| REQ-109 | SN-040, SN-043 | receive loop | receive API | pending | test pending | VAL-008 | approved |
| REQ-110 | SN-021, SN-043 | ARCH-075 | INT-075 | pending | inspection pending | VAL-007 | approved |
| REQ-111 | SN-016, SN-043 | stream runtime | INT-075 | pending | test pending | VAL-007 | approved |
| REQ-112 | SN-016, SN-043 | ARCH-076 | INT-080, INT-081 | pending | test pending | VAL-009 | approved |
| REQ-120 | SN-010, SN-015 | ARCH-080 | INT-090, INT-091 | pending | demonstration pending | VAL-011 | approved |
| REQ-121 | SN-015 | ARCH-081, ARCH-082 | INT-092 | pending | inspection pending | VAL-011 | approved |
| REQ-130 | SN-044 | ARCH-090 | INT-110, INT-111 | pending | inspection pending | VAL-012 | approved |
| REQ-131 | SN-044 | ARCH-091, ARCH-092 | INT-110, INT-111 | pending | inspection pending | VAL-012 | approved |
| REQ-140 | SN-045 | diagnostics | diagnostics pending | pending | inspection pending | VAL-013 | approved |
| REQ-150 | SN-022 | implementation phases | process | pending | inspection pending | review evidence | approved |
| REQ-151 | SN-021, SN-022 | crate structure | process | pending | inspection pending | review evidence | approved |

## Known initial gaps

GAP-001: Implementation links are mostly pending because this baseline adds process and engineering documents, not runtime code.

GAP-002: Test artifacts are pending until implementation phases begin.

GAP-003: Compile-fail test framework is not yet selected.

GAP-004: Diagnostics interfaces are intentionally deferred beyond the initial message model baseline.

GAP-005: Unix signal support is later-phase work and remains deferred for validation.

## Traceability maintenance rules

- Add rows when requirements are added.
- Do not remove rows merely because implementation is missing.
- Mark obsolete rows explicitly before removal.
- Keep requirement IDs stable once used by tests or code comments.
- Link tests to requirement IDs.
- Link examples to validation scenario IDs.
