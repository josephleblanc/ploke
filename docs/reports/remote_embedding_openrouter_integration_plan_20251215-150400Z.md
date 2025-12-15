# Remote Embedding (OpenRouter) Integration Plan — 2025-12-15

## Context
- This plan ties into the remote embedding roadmap tracked under `crates/ploke-tui/docs/active/plans/remote-embedding/` (see `reference-docs.md` and the blocker notes). It focuses specifically on landing OpenRouter-driven remote embeddings inside the `ploke-embed` pipeline while preserving the safety/test requirements called out in `docs/plans/todos/2025-09-11_todo.md`.
- Current embedding logic still assumes a single 384‑dim local model. New work must keep the stronger typing guarantees from `ploke-llm` (`crates/ploke-llm/src/embeddings/mod.rs`) and make it possible to switch embedding sets at runtime per the activation workflows described in `blocker-02-embedding-set-activation.md`.

## Current system snapshot

| Area | Key artifacts |
| --- | --- |
| Remote API client | `ploke-llm` already exposes strongly-typed embedding plumbing through `HasEmbeddings`/`HasEmbeddingModels`, `EmbeddingRequest`, and the OpenRouter-specific `OpenRouterEmbeddingFields`/`OpenRouterEmbeddingsResponse` (`crates/ploke-llm/src/router_only/openrouter/embed.rs`). Responses carry `OpenRouterEmbeddingVector` (`Float(Vec<f64>)` or `Base64(String)`), request metadata (model id, optional dimensions/provider prefs), and rich error mapping (`OpenRouterEmbeddingError`). |
| Embedding pipeline | `EmbeddingProcessor` and `EmbeddingSource` in `crates/ingest/ploke-embed/src/indexer/mod.rs` currently dispatch to local/HuggingFace/OpenAI/Cozo backends. `generate_embeddings` returns `Vec<Vec<f32>>`, the `process_batch` path retrieves snippets, streams them into BM25 via `DocData`, calls the embedder, enforces `dimensions()`, and finally writes vectors with `Database::update_embeddings_batch`. |
| Database + HNSW | `ploke-db` (feature `multi_embedding_db`) stores schema metadata in `EmbeddingSet` (`crates/ploke-core/src/embeddings.rs`) and exposes helpers in `crates/ploke-db/src/multi_embedding/db_ext.rs` and `hnsw_ext.rs`. `Database::active_embedding_set` controls which relation (`emb_{model}_{dims}`) receives vectors, `update_embeddings_batch` writes via the embedding-set-specific `:put` script, and `HnswExt::create_embedding_index` builds an HNSW relation using `embedding_set.dims()`. Today this is hard-wired to the default local set. |
| Runtime configuration | `ploke-tui/src/user_config.rs` instantiates `EmbeddingProcessor` based on `UserConfig.embedding`. No option exists yet for OpenRouter/`ploke-llm`, nor for switching the `Database.active_embedding_set` when a remote model is selected. |

## Implementation steps

### 1. Bridge `ploke-llm` embeddings into `EmbeddingProcessor`
1. **Client wiring:** Introduce a new `EmbeddingSource::OpenRouter(RemoteEmbeddingBackend)` in `crates/ingest/ploke-embed/src/indexer/mod.rs`. The backend should own the chosen `ModelId`, `EmbeddingSet` metadata (provider slug, dimensions), a reusable `reqwest::Client`, and `OpenRouterEmbeddingFields` builder.
2. **Request composition:** Map batched snippets into `EmbeddingRequest<OpenRouter>` (`EmbeddingInput::Batch`) and set `router.dimensions` to match the selected embedding set. Respect optional fields such as `input_type` for code snippets gathered from `ploke_io`.
3. **Response decoding:** Convert `OpenRouterEmbeddingVector::Float(Vec<f64>)` into `Vec<f32>` (with overflow checks). Treat `Base64` payloads as an error unless/until we add base64 support, surfacing a descriptive `EmbedError::Embedding`. Capture `resp.model`, `usage`, and `request_id` for logging/telemetry so we can satisfy the “evidence of live calls” gate from `blocker-05-telemetry-and-artifacts.md`.
4. **Error mapping:** Translate `OpenRouterEmbeddingError` (rate limits, overload, retry-after) into the existing `EmbedError` variants so callers can decide whether to back off or abort indexing.

### 2. Config + runtime selection flow
1. **User config schema:** Extend `ploke_embed::config::EmbeddingConfig` (`crates/ingest/ploke-embed/src/config.rs`) and `ploke-tui/src/user_config.rs` to accept an `openrouter` block containing the chosen model id, optional provider prefs, and target dimension override. Reuse `ploke-llm` types for strongly typed model ids instead of raw strings wherever possible.
2. **Database activation:** Add a way to swap `Database.active_embedding_set` (setter method or builder) before indexing starts. When a remote model is selected, construct an `EmbeddingSet` with `EmbeddingProviderSlug::new_from_str("openrouter")`, `EmbeddingModelId` from the config, and `EmbeddingShape::new_dims_default(dim)`. Call `create_embedding_set_relation`, `put_embedding_set`, and `ensure_vector_embedding_relation` (`crates/ploke-db/src/multi_embedding/db_ext.rs`) to register the relation if needed. Persist metadata so future runs can reuse the same hash id (per `EmbeddingSetId::from_components`).
3. **Indexer boot:** Thread the active `EmbeddingSet` into `EmbeddingProcessor` (for dimension checking) and `IndexerTask` so `process_batch` can emit updates with the matching set. This keeps `Database::update_embeddings_batch` (`crates/ploke-db/src/database.rs`) aligned with the remote backend, preventing writes to the default 384‑dim relation.
4. **UI/runtime commands:** Per `blocker-04-runtime-reconfiguration.md`, plan for a follow-up CLI command (`/embedding use`) that swaps embedding sets without restarting. For this milestone it is enough to expose the OpenRouter option in config and document the restart requirement.

### 3. HNSW + storage implications
1. **Dimension-aware indexes:** Before inserting remote vectors, ensure the new relation has a matching HNSW index. Invoke `HnswExt::ensure_embedding_relation` and `create_embedding_index` (`crates/ploke-db/src/multi_embedding/hnsw_ext.rs`) with the remote `EmbeddingSet`. The script uses `dim: {embedding_set.dims()}`, so calling it with e.g. a 3072‑dim model will create a distinct HNSW relation (`emb_{model}_{dim}:hnsw_idx`).
2. **Batch updates:** `Database::update_embeddings_batch` already converts `Vec<f32>` to `Vec<f64>` and records the embedding-set id. Confirm the new backend enforces `embedding.len() == embedding_set.dims()` before invoking it (mirroring the existing check in `IndexerTask::process_batch`).
3. **Metadata + observability:** Record the active embedding set, remote provider, and hnsw registration status in the planning logs under `crates/ploke-tui/docs/plans/agentic-system-plan/` so future agents can audit which relations belong to which provider.

### 4. Downstream adjustments
1. **BM25 path:** `process_batch` sends `DocData` to `bm25_service` (same file). No changes are required, but we should confirm the service keeps up with potentially larger remote batches since remote latency may push us to bigger batch sizes.
2. **Cost/usage tracking:** Plumb `OpenRouterEmbeddingsResponse.usage` through the indexing telemetry so we can later satisfy the cost modeling work in `blocker-07-cost-and-safety-modeling.md`. At minimum, log prompt/total tokens and per-request ids.
3. **Safety-first IO:** Remote embeddings should still respect the staging rules in `IoManagerHandle`. Confirm that `process_batch` continues to use `IoManagerHandle::get_snippets_batch` before any writes, and consider tagging each batch with the embedding-set hash for traceability.

## Invariants and risks
- **Dimension consistency:** `EmbeddingProcessor::dimensions()` (crates/ingest/ploke-embed/src/indexer/mod.rs:88-103) must equal `EmbeddingSet::dims()` (crates/ploke-core/src/embeddings.rs:57-82). Violations will corrupt both the vector relation and HNSW index creation.
- **Single-writer per embedding set:** All calls to `Database::update_embeddings_batch` (`crates/ploke-db/src/database.rs:919-939`) implicitly use `active_embedding_set`. Switching providers mid-run without updating this field will mix different vector spaces inside one relation. Introduce guards to prevent that.
- **Remote response shape:** `OpenRouterEmbeddingVector` may return `Base64` if `encoding_format` is set to `base64`. Until we intentionally request that mode, error loudly when the variant is not `Float`.
- **Error propagation:** Rate-limit (HTTP 429) and provider-overload (529) errors arrive with optional `Retry-After` (`OpenRouterEmbeddingError`). Decide whether `EmbeddingProcessor` retries automatically or surfaces the `Duration` to the caller so indexing can pause/resume.
- **Test gating:** Remote tests must only be marked green when the live call path is exercised, per the live-gate guidance in `AGENTS.md`. Never count skipped live tests as passes.

## Testing strategy
1. **Fixtures + unit tests (offline):**
   - Extend `crates/ploke-llm/src/router_only/openrouter/embed.rs` tests to cover the new backend plumbing (serialization of `EmbeddingRequest` with provider prefs, conversion to `Vec<f32>`, error mapping).
   - Add module tests in `crates/ingest/ploke-embed/src/indexer/mod.rs` that feed mocked `OpenRouterEmbeddingsResponse` values into the backend and verify `process_batch` rejects dimension mismatches.
   - Keep `cargo xtask verify-fixtures` in CI before hitting the network so we know fixtures like the embedding model list are fresh.
2. **Database/HNSW tests:**
   - Use the in-memory `Database::new_init()` + `setup_db_full` helpers to verify `create_embedding_set_relation`, `put_embedding_set`, and `create_embedding_index` register separate relations per dimension (write outputs under `target/test-output/embedding/schema/...` as evidence).
   - Add regression tests proving `Database.active_embedding_set` can be swapped and `get_rel_with_cursor` respects the supplied embedding-set id (see `crates/ploke-db/src/multi_embedding/db_ext.rs` queries).
3. **Live API tests (gated by `cfg(feature = "live_api_tests")`):**
   - Mirror the existing live tests in `crates/ploke-llm/src/router_only/openrouter/embed.rs:640-870` but route them through the new backend to ensure the pipeline exercises `TEST_APP` end‑to‑end.
   - Emit artifacts (e.g., `target/test-output/embedding/live/openrouter/<timestamp>.json`) capturing request ids, response models, dimensions, and tool-call traces so we can prove live coverage when toggling the feature flag.
4. **Indexer integration tests:**
   - Spin up an in-process `IndexerTask` with a mocked `IoManagerHandle` and remote backend to confirm `process_batch` updates the DB, BM25 channel, and progress reporting. These tests can stay offline by mocking the HTTP client.

## Open decisions / follow-ups
1. **Credential sourcing:** Should the OpenRouter API key be read from the existing router env resolution (`OpenRouter::resolve_api_key`) or from a dedicated embedding-specific secret? Tie into blocker-06 (provider credentials governance) before shipping.
2. **Model metadata cache:** We currently assume the operator “knows” the target model/dimensions. Decide whether `ploke-embed` should call `HasEmbeddingModels::fetch_embedding_models` to validate the model id and fetch its native dimension before creating the embedding set.
3. **Batch sizing & rate limits:** Remote embeddings introduce latency; we need heuristics (or provider prefs like `min_throughput` / `max_latency`) to size batches. Determine whether to expose these knobs in config or derive them from the OpenRouter model metadata.
4. **Usage accounting:** Where should we persist the per-request `usage` payload (tokens, cost)? Options include logging, a new telemetry table, or augmenting the embedding_set metadata. This ties into blocker-07 and needs alignment with observability goals.
5. **Runtime switching UX:** The current plan assumes restarting the TUI to pick up a remote embedder. Long term we need the `/embedding use` workflow outlined in `blocker-02`, which affects how we cache `EmbeddingProcessor` instances and how we stage IoManager writes.

Delivering the steps above will let `ploke-embed` call OpenRouter for embeddings, store the results in per-model relations, and build the required HNSW indexes, all while preserving the strong typing and safety requirements documented in the remote embedding plans.
