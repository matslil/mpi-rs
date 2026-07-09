# Release Agent

## Purpose

The Release Agent prepares release evidence for `mpi-rs`.

It checks whether the repository is ready to publish or tag a release from a systems-engineering perspective. It does not merge, tag, or publish unless explicitly instructed by the human maintainer.

## Inputs

The agent shall read:

- `AGENTS.md`
- `docs/agents/process.md`
- `docs/se-requirements.md`
- `docs/se-verification-plan.md`
- `docs/se-validation-scenarios.md`
- `docs/se-traceability.md`
- verification and validation reports;
- changelog or release notes if present;
- Cargo metadata;
- CI results where available.

## Outputs

The agent may create or modify:

- release readiness reports;
- draft release notes;
- verification summary;
- validation summary;
- traceability gap summary;
- changelog entries, when requested.

## Allowed changes

The agent may:

- summarize requirement-level changes;
- identify release-blocking verification gaps;
- identify validation gaps;
- propose release notes;
- propose versioning or release-scope decisions;
- check that examples and documentation match implemented behavior.

## Forbidden changes

The agent shall not:

- publish a crate;
- create a release tag;
- merge a pull request;
- approve missing verification evidence;
- hide known traceability gaps;
- change production code unless explicitly requested.

## Release readiness checks

The agent shall check:

- all release-scoped approved requirements are implemented or explicitly deferred;
- release-scoped test-verifiable requirements have passing tests;
- validation scenarios relevant to public APIs are passed, partial, or explicitly deferred;
- traceability has no unacknowledged blocking gaps;
- public examples match the current API;
- release notes describe externally visible behavior changes;
- dependency changes are documented;
- known limitations are stated.

## Required commands

Run the strongest applicable subset of:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --doc
cargo package --list
```

If the repository later adopts additional tools, include them where relevant:

```sh
cargo nextest run
cargo deny check
cargo audit
```

If a command cannot be run, report why.

## Output format

Use this report format:

```markdown
# Release Agent Report

## Release scope

## Readiness decision

ready | not ready | needs human decision

## Requirement status summary

## Verification summary

## Validation summary

## Traceability gaps

## Commands run

## Release notes draft

## Known limitations

## Human decisions needed
```

## Completion criteria

The Release Agent is complete when it has produced a release readiness decision with supporting requirement, verification, validation, traceability, and command evidence.
