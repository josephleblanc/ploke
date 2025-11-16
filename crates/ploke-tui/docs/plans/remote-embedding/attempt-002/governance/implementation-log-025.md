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

## Progress — 2025-11-15 (Slice 1 Phase A status)
- Re-reviewed the Slice 1 planning hub plus the experimental scaffolding expectations to realign on scope before making additional edits. Cross-referenced README, execution plan, feature flag doc, experimental fixtures plan, and telemetry evidence plan.
- Audited `crates/ploke-db/src/multi_embedding_experiment.rs` and `crates/ploke-db/src/error.rs` to capture the current runtime-per-dimension approach: each embedding vector relation is created dynamically through helpers that enforce `<F32; dims>` column widths, while `ExperimentalEmbeddingDbExt` centralizes metadata/vector queries and reports typed `DbError` variants for missing relations, layout mismatches, metadata parse errors, and unsupported dimensions.
- Confirmed that tests iterate over every node type spec plus the new helper trait, aligning with Phase A requirements (schema validation + metadata/vector parity) and highlighting that all negative paths now rely on typed errors rather than generic panics.
- Next coordination items: document results of the next targeted test run (`cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment`) with artifacts under `target/test-output/embedding/` and begin the Phase B fixture work called out in `experimental_fixtures_plan.md`.

## Progress — 2025-11-15 (Phase 1.5 adapter refinement)
- Added a new Phase 1.5 sub-section to the execution plan describing the Database adapter trait milestone, its gating, and evidence expectations so reviewers can track the interim refinement before Slice 2.
- Implemented `ExperimentalEmbeddingDatabaseExt` on `Database` with strongly typed helpers (`create_idx`, `search_embeddings_hnsw`, `vector_rows`, `vector_metadata_rows`) that encapsulate the Cozo scripts previously duplicated across the experiment tests. All call sites in `multi_embedding_experiment.rs` now rely on these methods, ensuring errors surface as `DbError::ExperimentalScriptFailure` variants tied to the adapter methods rather than ad-hoc `unwrap`/panic flows.
- Added helper utilities plus extensive adapter-focused tests covering happy paths, failure cases (missing relations, absent indexes), and edge scenarios (empty query results, multi-hit HNSW searches). Test command: `cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment` (passed with existing observability warnings unrelated to this change). Evidence to be summarized under the Slice 1 artifact set when collected.

## Progress — 2025-11-15 (Phase 1.5 ship readiness polish)
- Layered the adapter traits so `ExperimentalEmbeddingDatabaseExt` now extends a public `ExperimentalEmbeddingDbExt`, and implemented the base trait for both `Db<MemStorage>` and `Database`. This keeps the API stable while still allowing alternate backends to plug in later.
- Added `HnswDistance` to `create_idx` so we can switch between L2/Cosine/IP metrics per Cozo docs, cached the supported-dimension set via `lazy_static!`, and guarded vector relation creation to avoid redundant `:create` calls.
- Expanded negative coverage with a metadata-parse failure test and ensured helper calls propagate `DbError::ExperimentalMetadataParse` instead of panicking. All adapter tests now pass via `cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment`.
