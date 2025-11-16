# Remote Embedding Attempt 002 – Execution Plan

This document sequences the remote-embedding refactor into reviewable slices. It derives its requirements from:

- `crates/ploke-tui/docs/active/plans/remote-embedding/required-groundwork.md`
- `agent_report.md` (postmortem of attempt 001)
- `crates/ploke-db/src/multi_embedding_experiment.rs`
- `crates/ploke-tui/docs/archive/feature/agent-system/agentic_system_plan.md`

Each slice below must link evidence (tests, artifacts, docs) back to this plan when it lands. The slices are phased only to ease validation—once Slice 4 finishes, the single-embedding code paths are removed and multi-embedding becomes the sole supported architecture (no long-term dual path).

## Slice 1 – Schema module + ingest wiring
- **Goal.** Introduce the new embedding relations alongside the existing schema so we can populate multi-embedding data without touching runtime consumers yet.
- **Touch points.**
  - Add `embedding_vectors.rs` (or revive `embeddings.rs`) under `crates/ingest/ploke-transform/src/schema/` with strongly typed structs mirroring the experiment module.
  - Update `schema/mod.rs`, `transform/functions.rs`, and any generators/macros that emit primary node structs so they can emit metadata tuples required by the new relation.
  - Add a migration script (Cozo or xtask subcommand) that creates `embedding_nodes`+`embedding_vectors` per references in the groundwork doc §1.
  - Vector relations move to a per-dimension layout (`embedding_vectors_<dims>`) so each Cozo relation owns a fixed-width `<F32; N>` column. Provide a runtime `ensure_embedding_relation(provider, model, dims)` helper that emits the `:create` script on demand (so new providers/dimensions can be introduced without a recompile) and a verifier that enforces the naming convention via `::relations`.
- **Feature flags / build gating.** Add `multi_embedding_schema` cfg gate that limits use of the new schema wiring to tests + ingest; default OFF for prod so existing runtime ignores new relations until Slice 2.
- **Tests & fixtures.**
  - Unit tests for schema modules proving the new relations create + insert data at least for functions; align with `multi_embedding_experiment.rs` expectations and the scaffolding plan in `experimental_fixtures_plan.md`.
  - Per-dimension vector relations must each expose a single `<F32; dims>` column. Tests should query `::relations`/`::columns` to confirm that only allowed relations exist and that each stores exactly one vector column plus provider/model metadata, even when the relation was created dynamically at runtime.
  - Extend `cargo xtask verify-fixtures` to assert the new relations are present in fixture dumps (see `experimental_fixtures_plan.md` for required coverage).
  - Evidence artifact: `target/test-output/embedding/slice1-schema.json` summarizing migration + fixture verification.
- **Doc/report updates.** Annotate this plan and `required-groundwork.md` §1 with commit references; open an implementation log entry capturing design decisions.
- **Telemetry artifacts.** Follow `telemetry_evidence_plan.md` for artifact layout (`slice1-schema.json`, fixture hashes) before claiming readiness.

## Phase 1.5 – Embedding DB adapter + API cleanup
- **Goal.** Refine Phase 1 assets by consolidating experimental Cozo scripts behind a strongly typed adapter trait implemented on `Database`, improving maintainability before Slice 2 dual-write work begins.
- **Touch points.**
  - Add a `multi_embedding_experiment`-gated trait (e.g., `ExperimentalEmbeddingDatabaseExt`) within `crates/ploke-db/src/multi_embedding_experiment.rs` or nearby, implemented for `Database`, exposing helper methods such as `create_idx`, `search_embeddings_hnsw`, `vector_rows`, and `vector_metadata_rows`.
  - Replace ad-hoc in-test query construction with calls to the adapter methods so Cozo snippets live in one place and every call site benefits from typed `DbError` propagation.
  - Extend `DbError` docs/tests if additional error variants are needed while keeping the existing feature flag coverage.
- **Tests & evidence.**
  - Add targeted unit/integration tests that cover happy-path + failure-path behavior for each adapter method (creation failures, missing relations, malformed query results) plus edge cases (e.g., querying nodes with no embeddings or multiple hits for HNSW).
  - Update the experimental test module to call the adapter trait and ensure no remaining `should_panic` tests exist; failure cases should assert on the returned `DbError`.
  - Record results under `target/test-output/embedding/slice1-schema.json` (or a follow-up artifact noted in the telemetry plan) to show the adapter coverage before unlocking Slice 2.
- **Docs/report updates.** Note this sub-phase in the implementation log (`governance/implementation-log-025.md`) and annotate `telemetry_evidence_plan.md`/reports if additional artifacts are produced.
- **Exit criteria.** Trait consumed across the experiment module, new tests green with feature flag enabled, and governance docs updated so Slice 2 can leverage the centralized adapter.

## Slice 2 – Database dual write/read helpers
- **Goal.** Teach `ploke-db` to dual-write into the new embedding relations and dual-read when the flag is ON, while keeping legacy columns for compatibility.
- **Touch points.**
  - `crates/ploke-db/src/database.rs` helpers: `update_embeddings_batch`, `get_unembedded_node_data`, `count_pending_embeddings`, `get_nodes_ordered`, etc.
  - `crates/ploke-db/src/index/hnsw.rs` filters + builders so HNSW queries receive an embedding-set identifier and select the correct per-dimension relation.
  - Test utilities/fixtures (e.g., `setup_db_full_embeddings`).
- **Feature flags.** Introduce `#[cfg(feature = "multi_embedding_db")]` (depends on Slice 1 flag) that enables dual-write/dual-read. Add runtime config knob or env var so tests can enable it explicitly. Define exit criteria for removing legacy columns.
- **Tests & evidence.**
  - Repurpose/extend `multi_embedding_experiment.rs` into integration tests that validate metadata tuples align with vector rows across node types.
  - End-to-end DB tests verifying dual-write parity and HNSW search invariants for both 384 and 1536 dimensions.
  - Introduce a runtime registry (`EmbeddingDimensionRegistry` or similar) that maps a dimension value to its backing relation/table name and calls `ensure_embedding_relation` before writing. Add tests proving unknown dimensions trigger on-demand relation creation and that invalid names are rejected.
  - Artifact: `target/test-output/embedding/slice2-db.json` capturing pass/fail counts plus fixture hashes.
- **Docs.** Update this execution plan + `required-groundwork.md` with concrete helper names; add a decisions entry if schema naming conflicts arise.
- **Telemetry artifacts.** Produce `slice2-db.json` (and optional live artifacts) per `telemetry_evidence_plan.md`.

## Slice 3 – Indexer + runtime writers
- **Goal.** Update `ploke-embed` indexer, embedding processor, and ingest pipeline to write/read through the new DB helpers behind the established flag.
- **Touch points.**
  - `crates/ingest/ploke-embed/src/indexer/mod.rs`, provider implementations, and `EmbeddingProcessor` to emit `EmbeddingShape` + embedding-set IDs per batch, then dispatch vectors to the per-dimension relation reported by the registry (creating it first if missing).
  - `ploke-rag` consumers and TUI handlers that fetch embeddings (`app_state/handlers/embedding.rs`) so they request a specific embedding set.
  - Runtime configuration for `/embedding use` commands (as described in required-groundwork §3).
- **Feature flags.** Use `multi_embedding_runtime` (depends on db flag). Must ensure live API tests remain behind `cfg(feature = "live_api_tests")`.
- **Tests & evidence.**
  - Unit tests on indexer tasks verifying provenance, dimension enforcement, and telemetry spans.
  - Offline integration test invoking TEST_APP harness with `multi_embedding_runtime` enabled and verifying both local + remote providers populate separate sets.
  - Artifact: `target/test-output/embedding/slice3-runtime.json` summarizing job counts, provider latencies, and active set IDs.
- **Docs.** Extend implementation log and `/docs/reports/` with run evidence and open questions.
- **Telemetry artifacts.** Capture offline + live runtime evidence per `telemetry_evidence_plan.md`.

## Slice 4 – Cleanup + enablement
- **Goal.** Complete the migration by removing legacy single-embedding columns/paths, flipping the multi-embedding flags ON by default, and preparing to delete the temporary flags once soak testing completes.
- **Touch points.**
  - Database migrations dropping legacy columns + indexes.
  - `cargo xtask` commands and `/embedding` CLI verbs for pruning/listing sets.
  - Telemetry + artifacts under `target/test-output/embedding/live/` ensuring live gates capture provider traces.
- **Feature flags.** Flip `multi_embedding_schema`, `multi_embedding_db`, and `multi_embedding_runtime` ON by default as part of this slice. After soak, delete those flags (and the kill switch) entirely so the multi-embedding architecture is the only supported path. Keep the kill switch available only during the soak/testing period documented in the decisions log.
- **Tests & evidence.**
  - Full test suite with feature flags ON, plus live API gate (`cfg(feature = "live_api_tests")`), demonstrating real provider calls with artifact references.
  - Regression tests proving `/embedding list|use|drop|prune` operate on the new relations only.
  - Artifact: `target/test-output/embedding/slice4-release.json` plus live artifacts per provider.
- **Docs.** Update `required-groundwork.md`, this plan, and the agentic implementation log with completion evidence and follow-up tasks.
  - When removing transitional feature flags, ensure `crates/ploke-db/src/error.rs` (which holds the `#[cfg(feature = "multi_embedding_experiment")]` error variants) is updated so the DbError enum no longer depends on the temporary cfg.
- **Telemetry artifacts.** Generate slice 4 offline/live reports (including soak results) per `telemetry_evidence_plan.md`.

## Tracking & Reporting
- Remote-embedding governance artifacts (implementation logs, decision queue) now live under `crates/ploke-tui/docs/plans/remote-embedding/attempt-002/governance/`. Every slice gets its own log entry referencing this execution plan.
- Summaries of resolved decisions should still be mirrored back to the global agent-system docs after closure, but remote-specific discussions live in the governance subfolder noted above.
- Progress summaries + telemetry evidence should be stored under `crates/ploke-tui/docs/reports/` with filenames `remote-embedding-slice<N>-report.md` to keep the history linear.
