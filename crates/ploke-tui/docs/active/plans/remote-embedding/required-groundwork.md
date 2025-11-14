# Remote Embedding Required Groundwork

 - Added the requested survey + impact analysis to
 crates/ploke-tui/docs/active/ plans/remote-embedding/required-groundwork.md:1,
 covering the REVIEW TODO plus the surrounding plan areas (trait stack,
 metadata/runtime controls, telemetry). The doc now captures Q&A, affected
 modules, proposed commands (/embedding list| use|drop|prune), migration plans,
 and testing evidence expectations.

## REVIEW Q Answers

  - Model switching currently just warns on /model load because the embedder is
  constructed once and tightly coupled to the config (crates/ploke-tui/src/app/
  commands/exec.rs:92-120, crates/ingest/ploke-embed/src/indexer/mod.rs:58-97).
  Switching to another provider/dimension forces a full re-embed; the new doc
  recommends versioned embedding sets keyed by provider/model/dimension so
  cached vectors can be retained and toggled without rewriting the DB every
  time.
  - Embeddings are stored as nullable fields on every primary node relation
  (crates/ ingest/ploke-transform/src/schema/primary_nodes.rs:12-200) and
  written via Database::update_embeddings_batch
  (crates/ploke-db/src/database.rs:564-640). No separate node type exists
  today, so we need a new embedding_nodes relation plus joins/view helpers
  before multi-model support is possible.

## Impact Highlights

  - Section 1 of the new doc details how to split embeddings out of node rows,
  migrate data, and add CLI/database commands for listing/removing/activating
  embedding sets while keeping HNSW indexes per (node_type, model, dimension).
  - Section 2 maps out the trait stack modernization: introducing
  EmbeddingRouter/ EmbeddingRequest modules in ploke-tui, refactoring
  EmbeddingProcessor to hold Arc<dyn EmbeddingService>, and adapting provider
  crates (crates/ingest/ploke- embed/src/providers/*.rs) plus downstream
  consumers (ploke-rag, TUI handlers).
  - Section 3 covers metadata propagation (EmbeddingModelId, EmbeddingShape),
  an EmbeddingManager for hot reloads, and the new /embedding … commands that
  reuse the existing indexing event flow.
  - Section 4 outlines telemetry + gating expectations (structured tracing,
  artifacts under target/test-output/embedding/, and live tests guarded by
  cfg(feature = "live_api_tests")).

  No tests were run (documentation-only change). Suggested next steps:
  prioritize the schema split (dual-write and migration tooling), then land the
  trait stack + registry so runtime command work can build on a stable API
  surface.

## REVIEW Q Responses

### Model switching & dimensionality changes
- `crates/ploke-tui/src/app/commands/exec.rs:92-118` only detects that the embedding backend changed when `/model load` overwrites `config.toml`; the UI warns the user to restart because we cannot swap embedders at runtime.
- `crates/ingest/ploke-embed/src/indexer/mod.rs:58-97` exposes a single `EmbeddingProcessor::generate_embeddings` and `dimensions()` pair per backend. The processor owns whichever provider was picked at startup; switching models requires rebuilding the process.
- `crates/ploke-db/src/database.rs:528-630` and `crates/ploke-db/src/index/hnsw.rs:1-220` assume that every row in a given node relation shares identical dimensionality because the schema hard-codes `<F32; 384>?` for the `embedding` column.
- Result: when the user switches to a new provider/model with dimension `Y`, we overwrite the single `embedding` column and must drop/rebuild the HNSW index for that node type. There is no caching of the old vectors.
- Recommendation: introduce versioned embedding sets that are keyed by `(provider_slug, model_id, dimension, dtype)` so we can keep MODEL_ONE (dimension X) active while staging MODEL_TWO (dimension Y). Switching models should only toggle the active embedding set pointer; re-embedding is only necessary when we want to populate a set that does not exist locally.

### Storage layout of embeddings today
- Every primary node schema declares an `embedding: "<F32; 384>?"` column (`crates/ingest/ploke-transform/src/schema/primary_nodes.rs:12-200`). Secondary/assoc schemas do not expose their own dense vectors.
- `Database::update_embeddings_batch` (`crates/ploke-db/src/database.rs:564-640`) writes vectors directly into each node relation and recreates the `:put` statements for every type.
- Retrieval helpers such as `get_unembed_rel`/`get_nodes_ordered` (`crates/ploke-db/src/database.rs:820-930`) and `search_similar` (`crates/ploke-db/src/index/hnsw.rs:123-220`) all query `embedding` fields on the function/struct/etc. relations rather than a dedicated embedding table.
- Therefore embeddings are currently embedded fields, not separate nodes. The proposed refactor should migrate these floats into dedicated relations (see analysis below) so we can store multiple versions per code item without mutating the original node rows.

## Impact Analyses

### 1. Split embeddings out of primary nodes (addresses REVIEW TODO)
**Why**: We need independent storage per embedding model/dimension, provenance metadata, eviction commands, and the ability to re-use cached vectors when the user switches providers. Keeping a single `<F32; 384>` column on every node forces wholesale rewrites and prevents parallel embed sets.

**Current touch points**
- Schema: all entries in `crates/ingest/ploke-transform/src/schema/primary_nodes.rs` and downstream generated structs include the `embedding` field.
- Database ingest + queries: `ploke-db/src/database.rs` (batch updates, `get_unembed_rel`, `get_nodes_ordered`, `count_unembedded_*`, etc.) and `ploke-db/src/index/hnsw.rs` (index creation/search) hard-code the `embedding` columns.
- Runtime consumers: `ploke-rag/src/core/mod.rs:130-220` and `crates/ploke-tui/src/app_state/handlers/embedding.rs:10-69` build search queries that expect `embedding` to be part of each node row.

**Proposed direction**
1. **New relation** `embedding_nodes` (or `embedding_vectors`) with fields `{ id: Uuid, node_id: Uuid, provider_slug: String, model_id: String, dimension: u32, dtype: String, created_at: DateTime, embedding: <F32; ?> }`. Connect it to primary nodes via `node_id` foreign key and enforce uniqueness on `(node_id, provider_slug, model_id, dimension)`.
2. **Edges/views**: expose helper views `active_embedding(node_id)` and `embeddings_by_model(model_id)` so existing queries can join instead of selecting columns directly.
3. **HNSW indices**: create one index per `(node_type, provider_slug, model_id)` combination instead of per relation. The builder in `ploke-db/src/index/hnsw.rs:409-482` should accept an embedding set identifier and filter `embedding_nodes` accordingly.
4. **Version-aware commands**: add CLI verbs under `/embedding`:
   - `embedding list` → lists cached sets with model/dim metadata, hydration status.
   - `embedding use <provider>/<model> [--dimension <n>]` → marks a set as active (no re-embed if already present).
   - `embedding drop <provider>/<model> [--older-than <ts>]` → deletes stale vectors + HNSW indices.
   - `embedding prune --max <n>` → removes the least-recently-used sets to save space.
5. **DB APIs**: add `Database::list_embedding_sets`, `Database::activate_embedding_set`, `Database::delete_embedding_set`, and change `update_embeddings_batch` to write into the new relation (with validation against `dimension`). Node rows become smaller and no longer carry raw floats.
6. **RAG + search**: `search_similar` and `get_nodes_ordered` join against the active embedding set for the requested provider/model. Query arguments now include the target embedding set id rather than inferring solely by node type.

**Migration plan & tests**
- Phase 1: extend the schema to add `embedding_nodes` and backfill data by selecting `(node_id, embedding)` from each primary node relation. Keep the legacy columns until the backfill is done to avoid downtime.
- Phase 2: introduce dual-write logic in `update_embeddings_batch` (write new relation + legacy column). After verifying parity via `cozo` queries and new smoke tests, drop the legacy columns and update schema macros.
- Testing: unit tests covering schema migrations, integration tests that embed the same code base twice with different models and verify that both embedding sets remain accessible, and HNSW tests verifying each set enforces dimension consistency. Add CLI approval tests that exercise `embedding list`/`drop`/`use` and confirm evidence artifacts under `target/test-output/embedding/`.

### 2. Trait stack + provider routing
**Goal**: Replace ad-hoc providers with a router-style trait system (`EmbeddingRouter`, `EmbeddingRequest<R>`, `EmbeddingWireRequest`, etc.) so remote APIs share typed request/response structs.

**Affected code**
- Provider implementations: `crates/ingest/ploke-embed/src/providers/{hugging_face.rs,openai.rs}` currently build primitive HTTP requests with minimal error typing.
- Processor: `EmbeddingProcessor` (`crates/ingest/ploke-embed/src/indexer/mod.rs:25-97`) owns an enum of concrete backends; there is no trait object or registry.
- Config + registry: `crates/ploke-tui/src/user_config.rs:81-180` and `crates/ploke-tui/src/llm/router_only` handle LLM routing but have no embedding equivalent.
- Callers: `ploke-rag/src/core/mod.rs:130-220` and `crates/ploke-tui/src/app_state/handlers/embedding.rs:10-69` hold `Arc<EmbeddingProcessor>` and will need to depend on the new abstractions.

**Implementation steps**
1. Create `crates/ploke-tui/src/embedding/{mod.rs,router.rs,types.rs,wire.rs}` containing the trait stack proposed in the report. Mirror `llm::router_only` patterns for discoverability.
2. Refactor `EmbeddingProcessor` into a thin orchestrator holding `Arc<dyn EmbeddingService>`. The enum `EmbeddingSource` becomes an adapter that constructs router-specific services from config metadata.
3. Introduce `EmbeddingRegistry` (parallel to `llm::registry`) that normalizes provider catalogs, persists defaults in config, and surfaces constraints such as max batch sizes.
4. Update provider crates (`hugging_face`, `openai`, future Cohere) to implement the shared traits. Encode provider-specific endpoints (URL layout, headers) via associated constants and builder hooks rather than inlined strings.
5. Ensure rate-limit/backoff logic is centralized in the service implementation so `EmbeddingProcessor::generate_embeddings` is provider-agnostic.

**Testing**
- Unit tests verifying serialization/deserialization of request/response structs per provider.
- Integration tests using mocked HTTP servers to assert correct headers, body shape, and retry policies.
- `#[cfg(feature = "live_api_tests")]` smoke tests hitting OpenAI/Hugging Face to catch schema drift. Persist diagnostics under `target/test-output/embedding/live/` and only mark live gates green when tool-call traces confirm real executions (per AGENTS.md).

### 3. Metadata propagation & runtime configurability
**Why**: We need first-class types (`EmbeddingModelId`, `EmbeddingShape`, provenance structs) and runtime commands to rebuild embedders without restarting the TUI.

**Touch points**
- User config & runtime config: `crates/ploke-tui/src/user_config.rs:47-190` and `crates/ploke-tui/src/app_state/core.rs:1-120` currently store only a static `EmbeddingConfig` with mutually exclusive options.
- Command surface: `crates/ploke-tui/src/app/commands/parser.rs:22-140` lacks any `/embedding` verbs; everything flows through `/model load`.
- AppState + Rag: `AppState` holds `Arc<EmbeddingProcessor>` and `Option<Arc<RagService>>` built once at startup (`crates/ploke-tui/src/app_state/core.rs:1-80`). Changing providers requires restarting the entire app so the shared `EmbeddingProcessor` can be rebuilt.
- Indexing: `IndexerTask` (`crates/ingest/ploke-embed/src/indexer/mod.rs:159-759`) consumes `Arc<EmbeddingProcessor>` and assumes a single dimension throughout an indexing run.

**Plan**
1. Add newtypes `EmbeddingModelId`, `EmbeddingProviderSlug`, and `EmbeddingShape` (dimension + dtype + encoding) in a shared crate (likely `ploke-core` alongside `EmbeddingData`). Thread these through config structs, DB metadata, and runtime events so mismatches are caught early.
2. Introduce an `EmbeddingManager` in the TUI (parallel to `llm::manager`) that watches config changes, rebuilds `Arc<dyn EmbeddingService>` instances, and updates the `AppState::embedder` pointer atomically. The manager emits events so the RAG subsystem and indexing tasks can re-bind without restarting.
3. Extend the parser/executor with `/embedding status`, `/embedding use …`, `/embedding rebuild`, and `/embedding drop` commands that hook into the manager + DB APIs from impact area #1. These commands should stage work via the existing `IndexerTask`/`IndexingStatus` events instead of inventing new event types (per the REVIEW NOTE in the design report).
4. Update `IndexerTask` to accept an `EmbeddingShape` + embedding set id per run. The task should refuse to mix snippets encoded with different dimensions within a batch and record provenance in the DB.
5. Add UI affordances (status line, logs) that indicate the active embedding provider/model/dimension so users know when they are using cached vectors vs. running re-indexing jobs.

**Validation**
- Unit tests for the manager (config change triggers embedder rebuild; stale references are swapped safely).
- Integration tests that run `/embedding use` followed by RAG queries to ensure the new vectors are used.
- Regression tests verifying that switching back to a previously cached embedding set does not re-trigger a long indexing job (should only change the active pointer).

### 4. Resiliency, telemetry, and evidence capture
**Expectations**: The report calls for telemetry artifacts (`target/test-output/embedding/<timestamp>.json`), structured tracing, and feature-gated live tests.

**Updates required**
- Instrument `EmbeddingProcessor::generate_embeddings` and the provider implementations (`crates/ingest/ploke-embed/src/indexer/mod.rs:58-96`, `providers/*.rs`) with `tracing` spans that record provider slug, model id, batch size, latency, and HTTP status/rate-limit metadata. Guard sensitive payloads.
- Extend `IndexerTask::run` (`crates/ingest/ploke-embed/src/indexer/mod.rs:171-759`) to emit progress + error rows into deterministic JSON artifacts under `target/test-output/embedding/` (mirroring the OpenRouter live test harness).
- Update RAG handlers (`crates/ploke-tui/src/app_state/handlers/embedding.rs:10-69`) to tag retrieved contexts with the embedding set id/version so live gate assertions can prove the correct path was exercised.
- Introduce new integration tests (offline + `live_api_tests`) that assert artifact files exist, include pass/fail counts, and document whether live provider calls occurred. These tests should re-use the gating discipline described in `crates/ploke-tui/docs/reports/remote_vector_indexing_support_report_2025-02-18.md:20-65` and AGENTS.md.

**Artifacts & gating**
- Offline runs: write compact summaries (success counts, provider/model/dimension, average latency) to `target/test-output/embedding/offline_summary.json` after each CI job.
- Live runs: under `cfg(feature = "live_api_tests")`, persist one JSON file per provider invocation to `target/test-output/embedding/live/<timestamp>.json` that includes request metadata (minus secrets), observed rate-limit headers, and tool-call traces. Only mark the gate “ready” when these artifacts exist and verify the scenario (model switch, multi-dim search, etc.).

---

These analyses cover the outstanding REVIEW TODO item plus the surrounding architectural changes (traits, metadata, runtime controls, and telemetry) required to land remote embeddings safely without regressing existing functionality. The next step is to agree on the sequencing (schema split → trait stack → CLI/runtime work → telemetry) so implementation can proceed in controlled, testable increments.
