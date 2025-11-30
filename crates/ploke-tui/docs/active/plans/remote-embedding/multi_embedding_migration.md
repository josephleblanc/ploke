# Multi-Embedding Migration Status

_Last touched: 2025-11-15_

## Experiment Validation Review

- The gated prototype in `crates/ploke-db/src/multi_embedding_experiment.rs` already provisions two relations:
  - `function_multi_embedding` with an `embeddings: "[(String, Int)]"` payload (`crates/ploke-db/src/multi_embedding_experiment.rs:127-141`).
  - `function_embedding_vectors` with keyed rows per `(node_id, embedding_model, provider)` and dedicated `vector_dim384` / `vector_dim1536` columns (`crates/ploke-db/src/multi_embedding_experiment.rs:151-159`).
- `sample_params()` currently records metadata entries such as `"local/all-MiniLM-L6-v2"` and `"remote/ada-002"` (`crates/ploke-db/src/multi_embedding_experiment.rs:236-275`) while the vector rows use the canonical model names (`"sentence-transformers/all-MiniLM-L6-v2"` and `"text-embedding-ada-002"`, lines 190-202 & 205-219). Because the strings differ, we cannot yet join metadata to vectors.
- The existing test only asserts that the metadata list has length two and that two vector rows exist; tuple shapes and per-dimension invariants are not exercised (`crates/ploke-db/src/multi_embedding_experiment.rs:278-343`).
- The experimental schema macro copies the `ID_KEYWORDS` / `ID_VAL_KEYWORDS` arrays from `ploke-transform`, so it already understands `node_id`, `embedding_model`, and `provider`. The production macro in `crates/ingest/ploke-transform/src/schema/mod.rs:106-121` still lacks those identifiers, which would break `script_identity()` once we port the relations out of the experiment.

### Validation gaps to cover before propagating

1. **Metadata/vector join proof:** Query `function_multi_embedding.embeddings` and `function_embedding_vectors` together, keyed by a canonical tuple `(embedding_model, provider, embedding_dims)`. Update `sample_params()` to store the canonical model name so the join succeeds.
2. **Tuple-shape assertions:** Decode each `embeddings` entry inside the test and assert `(String, Int)` types plus dimension ↔ provider expectations.
3. **Vector-column exclusivity:** Verify that `vector_dim384` rows keep `vector_dim1536 == null` (and vice versa) so we do not pay HNSW costs for oversized vectors.
4. **Schema macro parity:** Extend the production `define_schema!` macro to understand `node_id`, `embedding_model`, and `provider` as identity fields, mirroring the experimental constants. Add a smoke-test that instantiates a list-of-tuples schema via `define_schema!` so we cannot regress support for `[(String, Int)]`.

## Propagation Plan

### Phase 1 – Harden the experiment

- Align metadata + vectors by storing canonical `(model, provider, dims)` strings everywhere.
- Add the join/assertion tests listed above and gate them behind `cargo test -p ploke-db --features multi_embedding_experiment`.
- Document expected tuple layouts inside the module so future agents can see the target shape without reading the full test.

### Phase 2 – Port schema definitions into ingestion (`ploke-transform`)

1. **Macro prerequisites:** Update `ID_KEYWORDS` / `ID_VAL_KEYWORDS` in `crates/ingest/ploke-transform/src/schema/mod.rs` and re-export them through `ploke-db`’s copy so `NodeType::keys/vals` stay in sync.
2. **Primary node schema:** Split `FunctionNodeSchema`’s `embedding: "<F32; 384>?"` field into:
   - `embeddings: "[(String, Int)]"` metadata stored inside the `function` relation.
   - Removal (or deprecation) of the old monolithic `embedding` column.
3. **New relations:** Add non-experimental schema structs for `function_multi_embedding` and `function_embedding_vectors`. Wire their `create_and_insert_schema()` calls into `create_schema_all()` so fixture ingestion populates them by default.
4. **Transform updates:** Adjust `transform_functions()` (and the `common_fields!` macro) so functions insert empty metadata lists rather than `embedding: null`. Create helper builders for `EmbeddingMetadata` entries to keep serialization code out of the Cozo scripts.

### Phase 3 – Database runtime (`ploke-db`)

1. **Node metadata queries:** Anywhere that currently filters by `!is_null(embedding)` (e.g., `helpers.rs:resolve_nodes_by_canon_in_file`, `get_by_id::COMMON_FIELDS_EMBEDDED`, `database::count_pending_embeddings`) must switch to joining `function_multi_embedding` and/or `function_embedding_vectors`.
2. **Update pipeline:** Replace `Database::update_embeddings_batch` with a strongly typed batch (e.g., `Vec<EmbeddingInsert>`) that includes `node_id`, `provider`, `embedding_model`, `embedding_dims`, and the vector payload. The implementation becomes:
   - Upsert metadata tuple(s) into `function_multi_embedding`.
   - Write rows into `function_embedding_vectors`, ensuring only the matching vector column is populated.
3. **Index maintenance:** Rework `index_embeddings`, `search_similar`, and `index::hnsw` to operate on the new relation and drive two HNSW indices keyed by dimension filters (mirroring the test flow). Capture reusable specs so we can extend to other dimensions later.
4. **Query builder support:** Plumb the new schema definitions into `NodeType` or a sibling enum so we can reuse `script_identity()` / field listings when generating Cozo scripts.

### Phase 4 – Embedding producers & downstream crates

1. **`ploke-embed`:** Teach the indexer to request multiple embeddings per node, likely by iterating configured providers and emitting `EmbeddingInsert` structs. This requires surfacing provider/model/dimension metadata from the orchestrator rather than assuming a single 384-d vector.
2. **`ploke-rag` / `ploke-tui` tooling:** Wherever we currently assume “has embedding” == single dense vector, swap to “has at least one vector row.” Update any UI surfaces (model pickers, edit tools) to read from `function_multi_embedding`.
3. **`bm25_index` and other code paths** that filter on `embedding` should be audited; most only need to know whether a node has snippets, so they should join against metadata instead of the old column.

### Phase 5 – Verification workflow

- Before every heavy run, execute `cargo xtask verify-fixtures` to make sure Cozo scripts and fixture DBs include the new relations.
- Extend the ploke-db integration tests so `cargo test -p ploke-db` covers both the legacy `embedding` path and the `multi_embedding_experiment` feature until we can flip the default.
- Capture evidence under `target/test-output/...` covering:
  - Schema creation scripts for both relations.
  - Batch update logs showing metadata + vector inserts.
  - HNSW search traces (one per dimension) that demonstrate end-to-end vector retrieval.

## Next Concrete Actions

1. Patch the experiment (metadata alignment, join validation, tuple-shape asserts).
2. Mirror the experimental schema into `ploke-transform`, updating macros + ingestion to recognize the new fields.
3. Design the `EmbeddingInsert` API and refactor the DB helper stack (`update_embeddings_batch`, `count_pending_embeddings`, `search_similar`) to consume it.
4. Upgrade the embedding indexer to emit multi-provider batches and prove the flow with `cargo test -p ploke-db --features multi_embedding_experiment`.
