# Blocker 01 – Embedding Storage Schema Split & HNSW Layout

_Last updated: 2025-11-14_

## Problem statement
- Every primary-node relation defined in `crates/ingest/ploke-transform/src/schema/primary_nodes.rs` bakes the embedding directly into the row via `embedding: "<F32; 384>?"` (see e.g. `FunctionNodeSchema` lines 13-27).
- `Database::update_embeddings_batch` (`crates/ploke-db/src/database.rs:564-650`) writes batches into every primary node, and `index/hnsw.rs` creates indexes that hard-code `dim: 384` for each relation.
- Because embeddings share storage with structural metadata, we cannot:
  - retain multiple embedding models simultaneously,
  - attach provenance (`provider_slug`, `model_id`, dtype) to vectors, or
  - enforce dimensional safety on a per-set basis without rewriting entire node relations.
- The groundwork doc calls for an `embedding_nodes` relation plus dedicated HNSW indexes keyed by `(node_type, embedding_set_id)`, but no schema or migration plan exists.

## Goals
1. Introduce a standalone, strongly typed storage layer for embeddings that supports arbitrary dimensions/dtypes and multiple providers.
2. Preserve the existing node relations during migration (dual writes) to avoid breaking RAG/search consumers mid-flight.
3. Provide deterministic, typed APIs in `ploke-db` for reading/writing embeddings through the new schema while maintaining IoManager safety guarantees.
4. Parameterize HNSW index creation so new embedding sets can be indexed without schema churn.
5. Produce evidence (scripts + tests) that proves the migration retains vectors and indexes.

## Proposed schema additions
### 1. `embedding_sets`
(Defined here for context; activation policies are detailed in Blocker 02.)

```
:put embedding_sets {
    id => Uuid,
    workspace => Uuid,              // matches namespace/WorkspaceId
    provider_slug => String,        // e.g. "openai", "hf"
    model_id => String,             // provider native identifier
    dimension => UInt32,
    dtype => String,                // "F32" for now; future-proof for int8/bfloat16
    metric => String,               // "cosine", "l2", etc.
    encoding => String,             // e.g. "float32-array", "base64"
    created_at => Int,              // unix micros (matches Cozo DateTime storage)
    updated_at => Int,
    status => String,               // "active", "staged", "retired"
    source_hash => Uuid,            // hash of embedder config used for auditing
    note => String?                 // optional human readable tag
}
```

- Unique constraint: `(workspace, provider_slug, model_id, dimension, dtype)`.
- `source_hash` is the IoManager-staged hash of the embedder config (ensures reproducibility).

### 2. `embedding_nodes`
Stores actual vectors detached from node relations.

```
:put embedding_nodes {
    id => Uuid,
    node_id => Uuid,
    node_type => String,            // one of NodeType::relation_str()
    embedding_set_id => Uuid,
    dimension => UInt32,
    dtype => String,
    vector => Bytes,                // little-endian packed f32 row
    norm => Float?,                 // optional cached ||v|| for cosine
    created_at => Int,
    updated_at => Int,
    shard_hint => UInt32?,          // reserved for horizontal partitioning
    FOREIGN KEY embedding_set_id REFERENCES embedding_sets{id}
}
```

Why `Bytes`: Cozo tensor columns require fixed shapes; using `Bytes` keeps schema stable and lets us decode into strongly typed structs in Rust (using `bytemuck` + `Vec<f32>`). `dimension` + `dtype` guard decoding, and a `CHECK` rule enforces `dimension > 0`.

Constraints:
- Primary key: `(embedding_set_id, node_id)` so each set rewrites the same logical node without duplication.
- Unique `(embedding_set_id, node_id)` already ensures single vector per set; `id` is still provided for legacy APIs and to simplify referencing rows from telemetry traces.
- Add covering index on `(node_type, embedding_set_id)` to accelerate HNSW seeding.

### 3. `embedding_nodes_metadata` (optional helper view)
A materialized view (or query helper) that joins `embedding_nodes` ↔ primary node metadata for RAG/resolution functions:
```
?[node_id, node_type, embedding_set_id, name, span, file_path, hashes, namespace] :=
    *embedding_nodes{node_id, node_type, embedding_set_id},
    match node_type {
        "function" => *function{ id: node_id, name, span, file_path, tracking_hash, file_hash, namespace },
        ...
    }
```
Rust helper `Database::embedding_nodes_with_context(...)` will encapsulate this join.

## HNSW layout updates
- Replace relation-specific indexes with per-embedding-set structures: `::hnsw create embedding_nodes:embedding_idx_{node_type}` filtered by `embedding_set_id`.
- `ploke-db/src/index/hnsw.rs` gets new functions:
  - `create_index_for_set(db, ty: NodeType, set: EmbeddingSetMetadata)` which emits:
    ```
    ::hnsw create embedding_nodes:embedding_idx_{ty} {
        fields: [vector],
        dim: $dim,
        dtype: $dtype,
        distance: $metric,
        filter: embedding_set_id = $set_id && node_type = $ty
    }
    ```
  - `replace_index_for_set` and `drop_index_for_set` mirror the API.
- Index names follow `embedding_nodes:embedding_idx_{node_type}_{set_short_id}` to avoid collisions; `set_short_id` is the first 8 chars of `embedding_set_id`.
- Searching uses `::hnsw query embedding_nodes:embedding_idx_{ty}_{set_short_id}` with `query: $vector` and returns `node_id` keyed rows; the caller then joins metadata via helper query.

## Migration plan
1. **Schema extension (phase 0)**
   - Add `embedding_sets` + `embedding_nodes` definitions to `ploke-transform` macros.
   - Extend `create_schema_all` to create both relations.
   - Update `ploke-db::Database::init_with_schema` tests (`crates/ploke-db/tests/unit/database_init_tests.rs`) to expect the new relations + HNSW index placeholders.

2. **Dual-write enablement (phase 1)**
   - Introduce `Database::upsert_embedding_vectors(set_id, vec<EmbeddingInsert>)` that writes packed vectors into `embedding_nodes` (with `Bytes` encoding) **and** keeps calling existing `update_embeddings_batch` so consumers keep reading from legacy columns.
   - Embedder pipeline (`ploke-embed/src/indexer/mod.rs:388-620`) switches to the new API but retains old commit for now.
   - Add instrumentation to compare row counts between `embedding_nodes` and legacy columns to catch drift early (persist results under `target/test-output/embedding/migration_phase1.json`).

3. **Backfill historical data (phase 1b)**
   - CLI command (likely `ploke-tui /embedding backfill --set <id>`) triggers a Cozo script:
     ```
     ?[node_id, node_type, embedding] := *function{...}
     :put embedding_nodes { node_id, node_type: 'function', embedding_set_id: $set, vector: encode_bytes(embedding) }
     ```
   - Use IoManager to stage the generated script(s) with hash verification before applying.
   - Validation: run Cozo query comparing `count_non_null(legacy.embedding)` vs. `count(embedding_nodes where embedding_set_id = $current_set)` per node type.

4. **Read-path migration (phase 2)**
   - Update `Database::get_nodes_ordered`, `search_similar`, and helpers to read from `embedding_nodes` (via join) but keep a feature flag `legacy_embedding_columns` in case rollback is needed.
   - `QueryResult::to_embedding_nodes` now expects columns from the join helper instead of legacy fields; add new typed struct `EmbeddingVectorRow { node_id, set_id, node_type }` if needed.

5. **Drop legacy columns (phase 3)**
   - After parity proof (artifact referencing counts + sample spot checks) is checked into `crates/ploke-tui/docs/reports/embedding_migration_status.md`, remove `embedding` columns from all primary/assoc schemas and delete obsolete Cozo scripts.
   - Retire `Database::update_embeddings_batch` and replace with typed methods targeting the new relation exclusively.

## Code touch points
- `crates/ingest/ploke-transform/src/schema/{primary_nodes.rs, assoc_nodes.rs}`: remove legacy columns only in phase 3.
- `crates/ploke-db/src/database.rs`: add new write/read helpers and restructure query builders to parameterize by `embedding_set_id`.
- `crates/ploke-db/src/index/hnsw.rs`: new per-set builders + queries; remove 384-dim literals.
- `crates/ploke-embed/src/indexer/mod.rs`: call new APIs, pass explicit `EmbeddingShape` metadata per set.
- `crates/ploke-rag/src/core/mod.rs` + `ploke-tui/src/app_state/database.rs`: update search APIs to supply `embedding_set_id` when requesting vectors.

## Evidence & validation
- **Unit tests**: Add coverage in `ploke-db/tests/unit` that demonstrates writing to `embedding_nodes`, generating HNSW indexes with arbitrary dims, and reading them back.
- **Migration harness**: `cargo xtask embedding-migration --verify` runs the backfill comparison queries, writes JSON summaries to `target/test-output/embedding/migration_phaseN.json`, and fails if mismatches occur.
- **Live gate**: When feature-flagged live tests (cfg `live_api_tests`) run indexing against remote providers, assert that `embedding_nodes` rows grow while legacy columns remain untouched (post phase 3 they should be absent).

## Outstanding questions for review
1. _Vector encoding_: `Bytes` packing keeps the schema simple but requires conversions in every consumer. Alternative is to store `[Num]` lists but they are slower for HNSW ingestion. Need confirmation that `Bytes` is acceptable for Cozo-based HNSW.
2. _Per-node-type partitions_: The plan above uses a single `embedding_nodes` relation keyed by `node_type`. If Cozo storage amplification becomes a problem, we may instead emit per-type relations (e.g., `function_embeddings`). Feedback requested.
3. _Namespace/workspace scoping_: For multi-workspace environments we may need `(workspace, node_id, set_id)` uniqueness instead of assuming global `Uuid`s. Validate this assumption before implementation.

Once these decisions are ratified we can unblock Blocker 02 (embedding set activation) and start landing the migration scaffolding.
