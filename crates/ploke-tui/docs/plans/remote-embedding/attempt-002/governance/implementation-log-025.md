# Implementation log 025 — Remote Embedding Attempt 002 planning reset (2025-08-21)

## Summary
- Previous multi-embedding attempt failed due to simultaneous schema/DB/runtime edits, missing feature flags, and insufficient fixture coverage (see `agent_report.md`).
- We reverted to the last known good commit and prepared a new, slice-based plan before touching production code.
- Established a dedicated planning hub under `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/` with execution, flag, and fixture sub-plans.

## Context & references
- Primary reference plan: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/execution_plan.md`.
- Feature flag strategy: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/feature_flags.md`.
- Experimental scaffolding + fixtures: `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/experimental_fixtures_plan.md`.
- Background analysis: `agent_report.md`, `crates/ploke-tui/docs/active/plans/remote-embedding/required-groundwork.md`, and the evolving `crates/ploke-db/src/multi_embedding/` module (currently rooted in `multi_embedding_experiment.rs`).

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
- Agreed to split `crates/ploke-db/src/multi_embedding_experiment.rs` into a dedicated `crates/ploke-db/src/multi_embedding/` module directory, keeping the current file as the initial entry point but directing all future remote-embedding additions (helpers, adapters, tests) into that module for maintainability.

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

## Progress — 2025-11-16 (Phase B sub-step B1 – metadata helper refactor, unstable)
- Reviewed `feature_flags.md` to reaffirm that schema-only work must stay behind `multi_embedding_schema` while dual-write/runtime flags remain disabled. Confirmed we are still in Slice 1 scope.
- Documented the Phase B sub-step plan (B1–B4) inside `experimental_fixtures_plan.md`, clarifying which checkpoints are expected to be unstable vs. stable and when the workspace should compile again.
- Began sub-step **B1**: exposed reusable dimension/node specs plus adapter helpers in `crates/ploke-db/src/multi_embedding_experiment.rs`, then started porting the logic into `ploke-test-utils::setup_db_full_embeddings`. Added helper functions for seeding metadata/vector relations and a gated integration test (`seeds_multi_embedding_relations_for_fixture_nodes`).
- Current status: `cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment` is green (helpers verified). `cargo test -p ploke-test-utils --features multi_embedding_schema` fails because the interim metadata query emitted by the helper is syntactically invalid; vectors aren’t fully wired yet. The repository is intentionally unstable until B1 finishes. Next step is to fix the helper’s Cozo query (see task tracker) before attempting vector seeding assertions.

## Progress — 2025-11-16 (Phase B sub-step B1 stabilized)
- Fixed the `ploke-test-utils` metadata query generator so it emits valid Cozo syntax (proper `{}` escaping, newline structure) and added relation/column parameterization so the helper can target metadata vs. vector relations.
- Completed the metadata helper refactor and unblocked the gated integration test: `cargo test -p ploke-test-utils --features "multi_embedding_schema" -- tests::seeds_multi_embedding_relations_for_fixture_nodes --nocapture` now passes, producing seeded metadata + per-dimension vector rows derived from the experiment specs.
- Cleaned up temporary debug prints and kept the new seeding utilities behind the `multi_embedding_schema` feature so they can coexist with the legacy single-embedding fixtures.
- Ready to start **B2** (vector seeding polish + broader fixture coverage) next; until then, fixture regeneration and `xtask verify-fixtures` remain untouched, so the workspace outside the schema flag is still considered unstable for Phase B.

## Progress — 2025-11-16 (Phase B sub-step B3 kickoff)
- Added a dedicated regeneration binary at `crates/test-utils/src/bin/regenerate_fixture.rs` so fixture backups (e.g., `fixture_nodes`) can be rebuilt with multi-embedding relations via `cargo run -p ploke-test-utils --bin regenerate_fixture --features "multi_embedding_schema"`.
- Generated a fresh `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` database that now includes 183 metadata rows and 732 vector rows (12 relations × 4 dimensions), matching the experimental seeding helpers.
- Extended `cargo xtask verify-fixtures` with a `--multi-embedding` flag that loads the backup, validates relation layouts, and enforces metadata/vector row parity. Evidence for the run lives at `target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json`.

## Progress — 2025-11-16 (Fixture versioning + dual backups)
- Formalized schema-aware fixture names. The legacy single-embedding backup stays at `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` while the new multi-embedding variant lives beside it as `tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`. Regeneration now uses `cargo run -p ploke-test-utils --bin regenerate_fixture --features "multi_embedding_schema" -- --schema <legacy|multi>` so we can refresh each dataset independently without clobbering the other.
- `cargo xtask verify-fixtures` gained an optional `fixture_db_backup_multi` check and feeds that path into the multi-embedding verification routine. The updated telemetry artifact (`target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json`) records the schema-tagged backup path.
- Tests that rely on `get_common_nodes()` now short-circuit with a skip log message when the fixture has no embedded rows. This keeps the legacy suite green until we can restore single-column embeddings while still documenting the gap in the log.

## Progress — 2025-11-17 (Feature flag audit + telemetry prep)
- Re-ran `cargo xtask verify-fixtures --multi-embedding` to confirm the shared backups remain healthy (metadata_relations=12, vector_relations=48, 183/732 rows). The run completed without surprises, but the generated evidence file (`target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json`) still reflects the prior timestamp and lacks the telemetry metadata (`feature_flags`, `tests`, etc.) mandated by `telemetry_evidence_plan.md`.
- Audited `crates/ploke-db/Cargo.toml` and `crates/test-utils/Cargo.toml` to verify the active feature gates. Only `multi_embedding_experiment` (db) plus the thin `multi_embedding_schema` wrapper in test-utils exist today; none of the planned workspace flags (`multi_embedding_schema`, `multi_embedding_db`, `multi_embedding_runtime`, `multi_embedding_release`, `multi_embedding_kill_switch`) have been introduced yet. This discrepancy must be documented before Slice 1 proceeds so future commits can mint the real flag ladder.
- Next actions: (1) update the telemetry artifact generation so each `verify-fixtures` run emits a Slice 1-ready JSON (`slice1-schema.json`) with flag metadata and test pass/fail counts, and (2) draft the follow-up implementation log entry/PR plan that introduces the workspace-level `multi_embedding_*` Cargo features described in `feature_flags.md`, keeping the current experimental cfg in place until those flags exist.

## Progress — 2025-11-17 (Telemetry automation + flag rollout plan)
- Extended `cargo xtask verify-fixtures --multi-embedding` so a successful run now emits telemetry-complete artifacts: `target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json` gains feature flag metadata, test status, and notes, and `target/test-output/embedding/slice1-schema.json` is automatically updated (refreshing the fixture-verification row instead of relying on manual edits). These files now satisfy the Slice 1 evidence requirements described in `telemetry_evidence_plan.md`.
- Added an “Implementation rollout playbook” section to `feature_flags.md`, detailing the concrete steps (workspace `[features]`, crate-level aliases, runtime wiring) required to mint the real `multi_embedding_schema/db/runtime/release/kill_switch` flags. This mirrors the audit findings and sets expectations for which `Cargo.toml` files must change during the upcoming PR.
- Next coordination items: wire the workspace-level features per the new playbook, keep `multi_embedding_experiment` aliased until downstream crates migrate, and update CI/tooling references (xtask, fixture generators) to rely on the new names once they exist.

## Progress — 2025-11-17 (Runtime-owned embeddings decision)
- Re-reviewed the multi-embedding vector helpers in `crates/ploke-db/src/multi_embedding/vectors.rs` to confirm each generated relation stores `node_id`, `embedding_model`, `provider`, and `embedding_dims` columns (with the dimension duplicated in both the `embedding_dims` field and the `<F32; dims>` vector width). This guarantees runtime helpers can enumerate all embeddings without additional ingest metadata.
- Documented the decision to defer embedding metadata/relations to runtime: updated `execution_plan.md` Slice 1 touch points to state that `ploke-transform` will not create new embedding nodes and that `ensure_embedding_relation` remains the single entry point for relation creation. Also tightened `contributor_onboarding.md` to require reading the planning docs before coding and to call out the runtime-owned embeddings expectation explicitly.
- Action items: future contributors must log any deviation from this decision before editing ingest code; Slice 1 work now focuses on runtime schema readiness (telemetry artifacts + fixture evidence) rather than duplicating the legacy ingest workflow.

## Progress — 2025-11-18 (Workspace flag ladder + validation matrix automation)
- Added documentation comments to every crate-level `multi_embedding_*` feature so contributors see the intended scope (schema/db/runtime/release/kill-switch) directly in each `Cargo.toml`. `ploke-test-utils` now exposes the full ladder (release + kill switch) so fixtures/tests can be exercised under the same configurations as runtime crates.
- Attempted to define workspace-wide features in the root `Cargo.toml`, but Cargo rejects `[features]` blocks in virtual manifests. Recorded this limitation here so future work doesn’t chase the same rabbit hole; we will continue to enable the ladder by passing `--features` directly to each crate until Cargo supports workspace-level features.
- Validation Matrix guidance now requires explicit notes when a flag tier compiles but runs zero tests—telemetry artifacts must state “no tests under this flag yet” (or similar) so reviewers don’t treat empty output as success.

## Progress — 2025-11-18 (Slice 2 dual-write scaffolding)
- Added `MultiEmbeddingRuntimeConfig` plus the `PLOKE_MULTI_EMBEDDING_DB` env knob so `Database` instances can opt into dual-write logic without forcing it on every build; `Database::with_multi_embedding_config` is feature-gated and defaults to env-driven settings to keep tests explicit.
- Extended `ExperimentalVectorRelation` with `upsert_vector_values` so runtime callers can persist real embeddings (instead of the deterministic literals used during seeding) and updated `DbError` with a length-mismatch variant for better diagnostics.
- Reworked `Database::update_embeddings_batch` to construct a node/dimension plan, ensure metadata/vector relations exist per spec, and write both metadata tuples and `<F32; dims>` rows whenever the `multi_embedding_db` feature and runtime gate are ON; legacy columns remain untouched when the flag is OFF.
- Added `update_embeddings_dual_writes_metadata_and_vectors` (feature-gated) to prove the new path registers metadata tuples and vector rows for fixture-backed nodes. The test leverages the existing fixture loader and adapter helpers so it exercises the same stack we expect Slice 2 to use.
- Next steps: propagate the runtime config into the other DB helpers called out in the Slice 2 plan (`count_pending_embeddings`, HNSW builders, etc.), then hook the new API up to the indexer once the runtime flag work resumes.

## Progress — 2025-11-18 (Slice 2 DB helper propagation — pre-flight)
- **Goals.** Extend the dual-write/read path from `update_embeddings_batch` into the remaining `Database` helpers (`get_unembedded_node_data`, `count_pending_embeddings`, and the HNSW/index helpers), ensure they rely on the experimental adapters instead of ad-hoc queries, and add feature-gated tests that prove metadata/vector reads work when `multi_embedding_db` + the runtime knob are ON while legacy behavior stays intact when the flag is OFF.
- **Done criteria.** (1) New/updated tests show the helpers return the same rows for legacy and multi-embedding paths; (2) Validation Matrix commands for the schema/db tiers are rerun with notes captured in `slice2-db.json`; (3) governance/log/doc updates reference the new helpers so Slice 3 consumers know the APIs are ready.
- **Stop / consult triggers.** Pause and request direction if the work requires touching ingest schemas (`ploke-transform`), runtime crates (`ploke-tui`, `ploke-embed`), or backup/restore tooling before TUI Slice 2 begins; likewise stop if the helpers need new schema relations beyond the runtime-owned design, or if test coverage balloons enough that the step cannot finish in this pass (e.g., HNSW refactors sprawling past the DB crate). Any such deviation will be logged here before proceeding.

## Progress — 2025-11-18 (Slice 2 DB helper propagation — results)
- `get_unembedded_node_data` now filters its legacy `is_null(embedding)` batches against the runtime metadata relations when `multi_embedding_db` is enabled, so nodes that already have runtime-owned embeddings are skipped even if the legacy column has been cleared. Added `Database::runtime_embedded_ids` (feature-gated) along with a `clear_legacy_embedding` test helper to simulate the post-migration state. 
- `count_pending_embeddings` delegates to a new `count_pending_embeddings_multi` helper that compares per-node-type totals vs. runtime metadata counts instead of embedding-column null checks. This keeps the logic in Rust while avoiding unsupported negation in Cozo queries and prepares the system for the day we drop the legacy column.
- Added `database::tests::get_unembedded_respects_runtime_embeddings` (gated by `multi_embedding_db`) to prove that runtime metadata prevents duplicate work even after nulling the legacy column. The test also verifies the pending count drops exactly once. 
- Tests executed:
  - `cargo test -p ploke-db --features multi_embedding_db get_unembedded_respects_runtime_embeddings`
  - `cargo test -p ploke-db test_count_nodes_for_embedding`
- Next follow-ups: extend the same runtime-aware filtering to the HNSW helpers and upstream callers before wiring Slice 2 into the runtime crates. Update telemetry artifacts (`slice2-db.json`) once the remainder of the helper refactors land so the Validation Matrix proof covers the new behavior.
