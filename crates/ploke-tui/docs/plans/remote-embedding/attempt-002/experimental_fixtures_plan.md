# Experimental Scaffolding & Fixture Plan

Purpose: ensure the experimental multi-embedding scaffolding (now tracked under `crates/ploke-db/src/multi_embedding/`, which initially re-exports `multi_embedding_experiment.rs` but will be split into smaller files) and our shared fixtures evolve into production-ready assets for attempt 002. We cannot touch production DB helpers until the experiment + fixtures cover every affected node type and `cargo xtask verify-fixtures` can validate the new relations.

## Current assets & gaps

| Asset | Current state | Gap |
| --- | --- | --- |
| `crates/ploke-db/src/multi_embedding/` module (`multi_embedding_experiment.rs` today) | Defines `function_multi_embedding` + `function_embedding_vectors`, Cozo helpers, unit test verifying metadata/vector parity and HNSW search. | Only functions covered; no schemas/tests for structs, enums, modules, impl blocks, etc. Needs generalization + shared helpers for other node types and embedding dims. |
| `ploke_test_utils::setup_db_full_embeddings` (`crates/test-utils/src/lib.rs:180-191`) | Returns `TypedEmbedData` by reading `get_unembedded_node_data` (legacy column). | Needs to seed multi-embedding fixtures for all node types, expose embedding-set metadata, and assert new relations exist when `multi_embedding_schema` is ON. |
| Fixture repo (`fixtures/fixture_nodes`, etc.) | Contains serialized primary node data with single `embedding` columns only. | Must be regenerated with multi-embedding relations + metadata tuples; `cargo xtask verify-fixtures` must gain a `--multi-embedding` mode that validates both legacy and new relations during transition. |

> **Fixture naming convention:** Legacy backups remain at `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`, while multi-embedding backups carry the schema tag in the filename (`tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`). Use `cargo run -p ploke-test-utils --bin regenerate_fixture --features "multi_embedding_schema" -- --schema <legacy|multi>` to refresh the appropriate variant and regenerate the matching `.meta.json`.

## Plan of record

### Phase A – Expand the experimental Cozo module
1. Generalize the schema macro invocations to cover every primary node relation (functions, structs, enums, type aliases, modules, constants). Each relation gets:
   - `<node>_multi_embedding` metadata table with `{node_id, embeddings: [(model, dims)], ...}` mirroring the function schema.
   - Dimension-specific `<node>_embedding_vectors_<dims>` relations keyed by `{node_id, provider, model_id}` where each relation owns a single `<F32; dims>` column. Adopt a shared naming helper so `embedding_vectors_384`, `embedding_vectors_768`, etc. are enforced consistently.
2. Add helper builders to generate synthetic vectors per dimension (384, 768, 1024, 1536) so future providers are easy to test.
3. Extend the unit tests to iterate over all schema pairs, verifying:
   - Metadata tuples match vector rows one-to-one.
   - The correct per-dimension relation receives the vector (no cross-contamination between dimensions).
   - HNSW index creation/search works for each dimension (use filters per schema/dimension).
4. Add a validation helper that queries `::relations`/`::columns` to ensure every `<node>_embedding_vectors_<dims>` relation has the expected suffix + column definition, and fails fast if an unknown dimension is introduced (this becomes the template for the runtime `ensure_embedding_relation` helper).
5. Store reusable fixtures produced by these tests under `target/test-output/embedding/experiment/` for reference.

**Stop & Test:** After Phase A changes, run `cargo test -p ploke-db multi_embedding_experiment` (with `multi_embedding_schema` enabled) and attach the pass/fail summary plus artifact paths to `remote-embedding-slice1-report.md` before touching shared fixtures.

### Phase B – Wire experiment data into shared fixtures
1. Update `ploke_test_utils::setup_db_full_embeddings` so when `multi_embedding_schema` is enabled it seeds the DB with:
   - Metadata rows for each node in the fixture using the experimental schema definitions.
   - Corresponding entries in the vector relations (using real or synthetic vectors depending on fixture coverage).
2. Regenerate the canonical fixtures (those consumed by `setup_db_full`, `setup_db_minimal`, etc.) with the new per-dimension relations present; the fixture documentation should explicitly list which dimension-specific relations were emitted so downstream tooling can audit them.
3. Introduce new fixture files if needed for multi-provider coverage (e.g., `fixture_nodes_multi_embed`). Document them in `fixtures/README.md`.
4. Ensure all fixture regen steps are validated via `cargo xtask verify-fixtures` before committing. Store the fixture hash/evidence in `target/test-output/embedding/fixtures/<timestamp>.json`.
5. **Preserve legacy embedding semantics and test coverage while fixtures evolve.** The legacy single-column `embedding` data must remain populated for all symbols and relations that existing tests depend on (e.g., `ploke-rag` dense/hybrid search tests, legacy HNSW tests) so those tests continue to pass unmodified. Helpers such as `ploke_test_utils::seed_default_legacy_embeddings` (currently seeding 32 rows @ 384 dims) are implementation details for regeneration and scaffolding; they **must not reduce legacy coverage or change which symbols have non-null embeddings** relative to the pre-refactor backups without (a) updating this plan, and (b) explicitly demonstrating that all pre-existing legacy tests still pass against the regenerated fixtures.

**Phase B working notes / unstable sub-steps:** Because this fixture refactor touches multiple crates, we will explicitly work through smaller checkpoints before expecting the workspace to compile with all flags enabled. During these checkpoints we will keep feature flags (`multi_embedding_schema`, future db/runtime flags) OFF by default and note any unstable commits in the governance log so they can be rolled back if needed. Any temporary divergence from the legacy embedding distribution that causes pre-existing tests to fail (for example, RAG dense search tests that previously passed) must be treated as a regression and tracked as such until we restore the original behavior.

| Sub-step | Description | Exit signal | Expected build state |
| --- | --- | --- | --- |
| **B1 – metadata helpers** | Refactor the `ploke-db::multi_embedding` module (`multi_embedding_experiment.rs` initially) to expose reusable specs + add `ploke-test-utils` helpers that seed metadata rows only (no vectors). Harden unit tests for adapter traits. | `cargo test -p ploke-db multi_embedding_experiment --features multi_embedding_experiment` green; `ploke-test-utils` metadata helper compiles but vectors still TODO. | Unstable: `ploke-test-utils` tests may fail under `multi_embedding_schema`. |
| **B2 – vector seeding + tests** | Extend helpers to write vectors per dimension, ensure `setup_db_full_embeddings` exposes seeded rows, and add integration tests (`seeds_multi_embedding_relations_for_fixture_nodes`). | `cargo test -p ploke-test-utils --features multi_embedding_schema` passes locally; document evidence path. | Stable under `multi_embedding_schema`; still need fixture regen + xtask work. |
| **B3 – fixture regeneration & verify command** | Regenerate fixture backups (per docs) via `cargo run -p ploke-test-utils --bin regenerate_fixture --features "multi_embedding_schema"`, update metadata/README, and extend `cargo xtask verify-fixtures --multi-embedding` to assert the new relations. | `cargo xtask verify-fixtures --multi-embedding` green locally; new fixture hashes recorded under `target/test-output/embedding/fixtures/`. | Build should be stable with schema flag ON. |
| **B4 – telemetry + documentation** | Capture Phase B telemetry artifact (`target/test-output/embedding/fixtures/<run>.json`) and summarize in governance docs + slice report before unlocking Slice 2. | Telemetry artifact referenced in `remote-embedding-slice1-report.md`; implementation log updated. | Stable (expected to match pre-refactor tests). |
| **B5 – test review & strictness audit** | Re-review every test touched while dual fixtures were introduced (see `governance/test_changes_review.md`). Restore any assertions that were temporarily skipped/relaxed (e.g., legacy helpers) so we continue catching regressions in both legacy and multi-embedding paths. Ensure parity tests assert on specific error variants/results before proceeding to Slice 2. | Explicit reviewer sign-off recorded in implementation log; warnings cleared from `test_changes_review.md`. | Stable; all tests run under both fixture variants without skips. |

Agents should only attempt to run the broader fixture/test matrix after completing a sub-step that is marked “stable.” If a detour is required (e.g., unexpected fixture blast radius), add a note to the governance log with the new sub-step identifier (B1a, B2-detour, etc.) so reviewers can follow the progression.

**Stop & Test:** Before moving to Phase C, run `cargo xtask verify-fixtures --multi-embedding` (with schema flag ON) and `cargo test -p test-utils setup_db_full_embeddings` to confirm fixtures and helpers behave as expected. Document results + hashes in the slice report.

### Phase C – Enhance `cargo xtask verify-fixtures`
1. Add `--multi-embedding` flag (and config default) so CI can enforce that fixture dumps contain:
   - All `<node>_multi_embedding` relations plus the required `<node>_embedding_vectors_<dims>` relations.
   - Matching counts between metadata tuples and vector rows, broken down per dimension.
   - Expected provider/model/dimension tuples for canonical fixtures.
2. When the flag is enabled (now available via `cargo xtask verify-fixtures --multi-embedding`), the command must fail if any relation is missing, mis-specified, or if sample vectors are absent.
3. Pipe the verification summary (pass/fail counts, fixture hash, flag state) to `target/test-output/embedding/fixtures/verify-fixtures.json` for documentation per AGENTS guidelines.

**Stop & Test:** After implementing the flag, run the command both with and without `--multi-embedding` to ensure failure modes are correct. Capture both outcomes in the report before enabling Slice 2 work.

### Phase D – Document & gate usage
1. Reference this plan from the execution plan Slice 1 + Slice 2 sections so reviewers know fixture coverage is a prerequisite.
2. Update `required-groundwork.md` (schema split section) with pointers to the experiment module and fixture requirements.
3. Add a checklist to the Slice 1 implementation log entry capturing:
   - Experiment module updated.
   - Fixtures regenerated.
   - `verify-fixtures --multi-embedding` evidence attached.
4. Only after this checklist is complete may Slice 2 code begin (dual-write in ploke-db).

**Stop & Test:** Verify that all evidence artifacts (experiment tests, fixture hashes, verify-fixtures outputs) are linked in the Slice 1 implementation log and `remote-embedding-slice1-report.md`. Obtain reviewer sign-off before lifting the gate on Slice 2.

### Ownership & timeline
- **Primary owner:** database + ingest pairing (same folks responsible for Slice 1).
- **Due before:** any PR that toggles `multi_embedding_db` defaults.
- **Evidence:** Attach links to the experiment module tests, fixture hashes, and verify-fixtures output in `remote-embedding-slice1-report.md`.

This plan should be reviewed alongside the execution plan so future contributors know the scaffolding expectations before modifying production code.
