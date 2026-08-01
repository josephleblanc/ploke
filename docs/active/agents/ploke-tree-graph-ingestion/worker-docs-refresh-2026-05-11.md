# Docs Refresh Worker Report - 2026-05-11

Task: `docs-refresh-graph-ingestion-staleness`

Changed file:

- `docs/active/agents/ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md`

Reported changes:

- Marked `messages/child-plan/node-*.json` as loaded passive child-plan
  evidence with summary-only graph evidence.
- Marked `run-profile.toml` and `run-profile.commitment.json` as loaded
  passive run-profile evidence.
- Added `PassiveEvidence.child_plans` and `PassiveEvidence.run_profile` rows
  to current loader coverage.
- Updated type ownership rows to `ChildPlanRecord`, `RunProfileRecord`, and
  `RunProfileCommitmentRecord`.
- Removed child-plan and run-profile rows from missing/unresolved loader rows.
- Updated the bottom-line summary to say these attach as typed evidence, not
  missing inputs.

Verification reported by worker:

- `rg` check for stale child-plan/run-profile missing/not-loaded/no-passive-owner
  wording: no matches.
- Targeted source search confirmed current names.
- No `crates/` files edited.

Follow-up: none from this docs-only refresh.
