# Remote Embedding Attempt 002 — Multi-Embedding Module Reorg Progress

_Date:_ 2025-11-16

## Summary
- Completed the structural split of `crates/ploke-db/src/multi_embedding/` into cohesive modules (`schema`, `vectors`, `adapter`, `seeding`, `tests`).
- Added strongly-typed helpers on `ExperimentalRelationSchema` and `ExperimentalVectorRelation` so metadata/vector relations can be lazily created (`ensure_registered`) before inserting rows.
- Relocated the validation/sample builders into the `ploke-db` crate (new `seeding.rs`) so unit tests own their fixtures without round-tripping through `test-utils`.
- Updated `test-utils::seed_multi_embedding_schema` to call the new helpers instead of duplicating relation creation logic, eliminating the `metadata_projection_fields` copy.

## Implementation Notes & Follow-ups
- `ExperimentalNodeRelationSpec` now exposes `metadata_projection_fields()`; downstream code (fixtures, ingest) should prefer this instead of re-implementing column lists.
- The adapter traits (`ExperimentalEmbeddingDbExt`, `ExperimentalEmbeddingDatabaseExt`) remain the single entry point for Cozo access; future Phase 1.5 work can layer production APIs on top without leaking test-only helpers.
- `ensure_relation_registered` currently spins up relations synchronously via `Db::run_script`. When the runtime wiring begins, consider threading these calls through an IoManager façade to respect the “safety-first editing” guardrails.
- `test-utils` still seeds multi-embedding relations via live fixture data. We should revisit fixture regeneration after the Slice 1 evidence gate to ensure the multi-embedding backups contain the new relations and relation-name verifier outputs.
- Known flaky/locked DB tests (`bm25_index::*`, `hnsw::*`, `get_by_id::*`) still fail under concurrent runs; no regressions observed in the new multi-embedding suite.

## Next Steps
1. Thread the new `ensure_*` helpers into ingest/xtask once the schema-gated slices progress to dual-write mode.
2. Extend the adapter tests with negative cases for `ensure_registered` (e.g., deliberately corrupting a relation’s vector column) to surface clearer errors.
3. Once fixtures are regenerated, capture hashes + evidence artifacts under `target/test-output/embedding/` per telemetry plan and reference them from the Slice 1 report.
