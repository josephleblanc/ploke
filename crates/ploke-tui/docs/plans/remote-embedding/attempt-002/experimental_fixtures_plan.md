# Experimental Scaffolding & Fixture Plan

Purpose: ensure the experimental multi-embedding scaffolding (currently `crates/ploke-db/src/multi_embedding_experiment.rs`) and our shared fixtures evolve into production-ready assets for attempt 002. We cannot touch production DB helpers until the experiment + fixtures cover every affected node type and `cargo xtask verify-fixtures` can validate the new relations.

## Current assets & gaps

| Asset | Current state | Gap |
| --- | --- | --- |
| `crates/ploke-db/src/multi_embedding_experiment.rs` | Defines `function_multi_embedding` + `function_embedding_vectors`, Cozo helpers, unit test verifying metadata/vector parity and HNSW search. | Only functions covered; no schemas/tests for structs, enums, modules, impl blocks, etc. Needs generalization + shared helpers for other node types and embedding dims. |
| `ploke_test_utils::setup_db_full_embeddings` (`crates/test-utils/src/lib.rs:180-191`) | Returns `TypedEmbedData` by reading `get_unembedded_node_data` (legacy column). | Needs to seed multi-embedding fixtures for all node types, expose embedding-set metadata, and assert new relations exist when `multi_embedding_schema` is ON. |
| Fixture repo (`fixtures/fixture_nodes`, etc.) | Contains serialized primary node data with single `embedding` columns only. | Must be regenerated with multi-embedding relations + metadata tuples; `cargo xtask verify-fixtures` must gain a `--multi-embedding` mode that validates both legacy and new relations during transition. |

## Plan of record

### Phase A – Expand the experimental Cozo module
1. Generalize the schema macro invocations to cover every primary node relation (functions, structs, enums, type aliases, modules, constants). Each relation gets:
   - `<node>_multi_embedding` metadata table with `{node_id, embeddings: [(model, dims)], ...}` mirroring the function schema.
   - `<node>_embedding_vectors` relation keyed by `{node_id, provider, model_id, embedding_dims}` plus vectors per supported dimension.
2. Add helper builders to generate synthetic vectors per dimension (384, 768, 1024, 1536) so future providers are easy to test.
3. Extend the unit tests to iterate over all schema pairs, verifying:
   - Metadata tuples match vector rows one-to-one.
   - Dimension-specific vector columns are mutually exclusive (only the matching dimension slot is populated).
   - HNSW index creation/search works for each dimension (use filters per schema/dimension).
4. Store reusable fixtures produced by these tests under `target/test-output/embedding/experiment/` for reference.

**Stop & Test:** After Phase A changes, run `cargo test -p ploke-db multi_embedding_experiment` (with `multi_embedding_schema` enabled) and attach the pass/fail summary plus artifact paths to `remote-embedding-slice1-report.md` before touching shared fixtures.

### Phase B – Wire experiment data into shared fixtures
1. Update `ploke_test_utils::setup_db_full_embeddings` so when `multi_embedding_schema` is enabled it seeds the DB with:
   - Metadata rows for each node in the fixture using the experimental schema definitions.
   - Corresponding entries in the vector relations (using real or synthetic vectors depending on fixture coverage).
2. Regenerate the canonical fixtures (those consumed by `setup_db_full`, `setup_db_minimal`, etc.) with the new relations present.
3. Introduce new fixture files if needed for multi-provider coverage (e.g., `fixture_nodes_multi_embed`). Document them in `fixtures/README.md`.
4. Ensure all fixture regen steps are validated via `cargo xtask verify-fixtures` before committing. Store the fixture hash/evidence in `target/test-output/embedding/fixtures/<timestamp>.json`.

**Stop & Test:** Before moving to Phase C, run `cargo xtask verify-fixtures --multi-embedding` (with schema flag ON) and `cargo test -p test-utils setup_db_full_embeddings` to confirm fixtures and helpers behave as expected. Document results + hashes in the slice report.

### Phase C – Enhance `cargo xtask verify-fixtures`
1. Add `--multi-embedding` flag (and config default) so CI can enforce that fixture dumps contain:
   - All `<node>_multi_embedding` and `<node>_embedding_vectors` relations.
   - Matching counts between metadata tuples and vector rows.
   - Expected provider/model/dimension tuples for canonical fixtures.
2. When the flag is enabled, `verify-fixtures` must fail if any relation is missing, mis-specified, or if sample vectors are absent.
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
