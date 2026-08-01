# Evidence Ledger

- last_updated: 2026-04-09
- owning_branch: `refactor/tool-calls`
- review_cadence: daily at 3:00 p.m. America/Los_Angeles local time
- update_trigger: update after any formal run, reviewed postmortem, or belief change
- id_conventions: [id-conventions.md](../../workflow/id-conventions.md)

## Update Policy

- Add or revise a belief only when it is backed by an artifact, a replay, or a reviewed postmortem.
- Link the supporting artifact directly.
- Keep each entry short: belief, evidence, consequence.
- Revise an existing belief when the new evidence sharpens it; do not create near-duplicate belief IDs.

## Entries

### BEL-001: Current run provenance is split across multiple artifacts

- belief:
  `run.json` is not yet the full immutable manifest described in the design doc.
- evidence:
  current runs store provider selection in `execution-log.json`, turn outcome details in `agent-turn-summary.json`, and repo/snapshot status in separate JSON files.
- consequence:
  the workflow should treat [run-manifest.v0.draft.json](../../workflow/run-manifest.v0.draft.json) as the target schema, not a claim about current state.

### BEL-002: Setup and provider failures need their own visibility before H0 claims are credible

- belief:
  recent runs can fail before a meaningful patch attempt for reasons that are not about model capability.
- evidence:
  the active eval design and postmortem materials emphasize setup reliability, provider reliability, and artifact ambiguity as blocking concerns.
- consequence:
  readiness gates and the failure taxonomy must keep these classes visible instead of collapsing them into generic task failure.


