# Remote Embedding Attempt 002 – Planning Hub

This directory is the single entrypoint for all planning, governance, and evidence artifacts related to the remote-embedding refactor. New agents should start here to understand the current state before editing code.

## Core references

| Doc | Purpose |
| --- | --- |
| `execution_plan.md` | Slice-by-slice breakdown (schema → DB → runtime → cleanup) with file touch points, tests, and telemetry obligations. |
| `feature_flags.md` | Defines temporary cfg/features, dependencies, runtime knobs, and end-state expectations (full migration). |
| `experimental_fixtures_plan.md` | Describes how to extend the Cozo experiment, regenerate fixtures, and gate Slice 1 with stop-and-test checkpoints. |
| `telemetry_evidence_plan.md` | Specifies artifact formats, tests, and live gate requirements per slice. |
| `contributor_onboarding.md` | Quick-reference onboarding guide listing the exact files, commands, and evidence to review before editing code. |

## Governance & history

| Doc | Purpose |
| --- | --- |
| `governance/implementation-log-025.md` | Latest implementation log entry describing the planning reset and references. Future slice entries should also live in `governance/`. |
| `governance/decisions_required_remote_embedding.md` | Active decision queue (e.g., kill-switch policy, storage caps). Mirror resolved decisions back to the global agent-system log after closure. |
| `../reports/remote-embedding-slice<N>-report.md` (planned) | Slice summaries with links to telemetry artifacts and test evidence. |

## Directory layout

```
crates/ploke-tui/docs/plans/remote-embedding/attempt-002/
├── README.md  (this file)
├── execution_plan.md
├── feature_flags.md
├── experimental_fixtures_plan.md
├── telemetry_evidence_plan.md
└── governance/
    ├── implementation-log-025.md
    └── decisions_required_remote_embedding.md
```

## How to use this hub
1. Read `execution_plan.md` to understand the current slice and prerequisites.
2. Check `feature_flags.md` for the gating state you must use while testing.
3. Follow `experimental_fixtures_plan.md` before altering schemas/fixtures.
4. Produce artifacts as specified in `telemetry_evidence_plan.md` and store them under `target/test-output/embedding/`.
5. Update the implementation log + decisions file in `governance/` whenever plans change or new questions arise.

Maintaining this directory keeps all planning documents discoverable and prevents stale references elsewhere in the repo. Any new planning/governance doc for remote embeddings should be added here and linked from this README.
