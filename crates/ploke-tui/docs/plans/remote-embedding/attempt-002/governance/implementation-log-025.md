# Implementation log 025 — Remote Embedding Attempt 002 planning reset (2025-08-21)

## Summary
- Previous multi-embedding attempt failed due to simultaneous schema/DB/runtime edits, missing feature flags, and insufficient fixture coverage (see `agent_report.md`).
- We reverted to the last known good commit and prepared a new, slice-based plan before touching production code.
- Established a dedicated planning hub under `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/` with execution, flag, and fixture sub-plans.

## Context & references
- Primary reference plan: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/execution_plan.md`.
- Feature flag strategy: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/feature_flags.md`.
- Experimental scaffolding + fixtures: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/experimental_fixtures_plan.md`.
- Background analysis: `agent_report.md`, `crates/ploke-tui/docs/active/plans/remote-embedding/required-groundwork.md`, `crates/ploke-db/src/multi_embedding_experiment.rs`.

## Decisions / guidance captured
- Remote embeddings will fully replace the legacy single-embedding path once Slice 4 completes; feature flags are temporary validation gates.
- Every slice must produce evidence artifacts under `target/test-output/embedding/` with explicit flag states.
- Slice 1 cannot proceed to Slice 2 until the experimental scaffolding checklist (Phase A–D) plus stop-and-test checkpoints pass.

## Work completed in this log entry
1. Authored the Slice-by-slice execution plan referencing the groundwork doc and postmortem insights.
2. Documented the feature flag/kill-switch strategy plus end-state expectations.
3. Produced the experimental scaffolding + fixture plan with mandatory test checkpoints.
4. Centralized these docs under a new planning directory for attempt 002 to keep future updates localized.

## Risks / blockers
- Fixture regeneration requires consensus on canonical multi-embedding sample data; pending until Phase B owners propose data format.
- Decisions needed on: (a) kill-switch rollout/rollback procedure, (b) storage sizing expectations when multiple embedding sets are retained. Added to `decisions_required.md` (see entry IDs REM-EMB-001 and REM-EMB-002).

## Next steps
- Update `crates/ploke-tui/docs/active/plans/remote-embedding/required-groundwork.md` to reference the new planning hub (pending).
- Execute Phase A of the experimental scaffolding plan before editing production schemas.
- Start drafting telemetry/evidence expectations document (per planning checklist).
- Maintain this implementation log by appending new entries per slice, referencing artifacts and decisions as work proceeds.

## Plan-adjustment guardrails
- While reviewing work-in-progress slices, capture potential plan changes here (e.g., switching to per-dimension vector relations, additional verification gates, fixture ownership clarifications). Treat this section as a discussion queue rather than an auto-merge policy.
- Any proposed tweak must include (a) impact summary, (b) affected docs/files, (c) blocking reason if we postpone it. This keeps future agents aware of emerging risks without silently mutating the plan.
- **Do not update the core planning docs/flag references unless a human explicitly requests the change.** Use this guardrail to document hypotheses and surface them during review, then wait for user approval before editing the authoritative plans.

## Recent updates
- Confirmed user-approved shift to runtime-created per-dimension vector relations (create the `<F32; dims>` relation on demand, validate via `::relations`/`::columns`). Execution + fixture plans now mention the `ensure_embedding_relation` helper so future slices follow the same approach.
