## Remote embeddings (OpenRouter) integration report for `ploke-embed`

### Goal
Add support for **remote embeddings via OpenRouter** to the embedding pipeline in `crates/ingest/ploke-embed`, using the new `crates/ploke-llm` crate as the HTTP client + typed request/response layer.

This report assumes we already know which embedding model to use (and whether we want to request a reduced `dimensions`).

---

## Current state (what exists today)

### `ploke-llm` (remote embeddings plumbing)
- **Typed request**: `crates/ploke-llm/src/embeddings/mod.rs`
  - `EmbeddingRequest<R>` with:
    - `model: ModelId`
    - `input: EmbeddingInput::{Single,String | Batch,Vec<String>}`
    - `encoding_format: Option<EmbeddingEncodingFormat::{Float,Base64}>`
    - `user: Option<String>`
    - `router: R::EmbeddingFields` (flattened router-specific fields)
  - `HasEmbeddings::fetch_embeddings(&reqwest::Client, &EmbeddingRequest<Self>) -> Future<Result<Response>>`

- **OpenRouter implementation**: `crates/ploke-llm/src/router_only/openrouter/embed.rs`
  - `OpenRouterEmbeddingFields { dimensions: Option<u32>, provider: Option<EmbeddingProviderPrefs>, input_type: Option<String> }`
  - `OpenRouterEmbeddingsResponse { data: Vec<...>, model: EmbeddingModelName, id, usage }`
  - `OpenRouterEmbeddingVector::{Float(Vec<f64>), Base64(String)}`
  - `OpenRouterEmbeddingError` maps non-2xx HTTP codes and captures `Retry-After` for 429.

**Implication**: `ploke-llm` is already the right “typed boundary” to call OpenRouter. The missing work is **adapting `ploke-embed`’s `EmbeddingSource` / `EmbeddingProcessor` to use it**, and ensuring the DB is set up for the chosen (provider, model, dims) embedding set.

---

### `ploke-embed` (batching + DB update loop)
Main entrypoints in `crates/ingest/ploke-embed/src/indexer/mod.rs`:
- `EmbeddingProcessor { source: EmbeddingSource }`
- `EmbeddingSource::{ Local, HuggingFace, OpenAI, Cozo }`
- `EmbeddingProcessor::generate_embeddings(snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError>`
  - Delegates to `backend.compute_batch(snippets).await` for non-local sources.

Embedding persistence happens in `IndexerTask::process_batch(...)`:
- Fetch snippets via `IoManagerHandle::get_snippets_batch`.
- Call `embedding_processor.generate_embeddings(valid_snippets).await?`.
- Validate every returned vector has `embedding.len() == embedding_processor.dimensions()`.
- Persist via `self.db.update_embeddings_batch(updates)?`.

Index creation differs by feature flag:
- Non-multi path: creates per-node-type indices (legacy embedding field)
- `#[cfg(feature = "multi_embedding_embedder")]`: uses the multi-embedding HNSW path (see unit tests).

---

### `ploke-db` (multi-embedding schema + HNSW invariants)
Relevant files:
- Schema helpers: `crates/ploke-db/src/multi_embedding/schema.rs`
- DB methods: `crates/ploke-db/src/multi_embedding/db_ext.rs`
- HNSW methods: `crates/ploke-db/src/multi_embedding/hnsw_ext.rs`

Key data structures:
- `ploke_core::embeddings::EmbeddingSet` (`crates/ploke-core/src/embeddings.rs`)
  - `{ provider: EmbeddingProviderSlug, model: EmbeddingModelId, shape: EmbeddingShape{dimension,dtype}, hash_id: EmbeddingSetId, rel_name: EmbRelName }`
  - `EmbeddingSet::new(...)` **sanitizes** the derived `rel_name` (replaces `-` and `/`) which is important for OpenRouter model IDs like `openai/text-embedding-3-small`.

Key DB behavior:
- `Database::update_embeddings_batch` (`crates/ploke-db/src/database.rs`) converts `Vec<f32>` → `Vec<f64>` and forwards to the multi-embedding implementation using `&self.active_embedding_set`.
  - This makes **`Database.active_embedding_set`** the critical selection point for which embedding relation gets the vectors.

HNSW invariant:
- `create_embedding_index(&EmbeddingSet)` in `crates/ploke-db/src/multi_embedding/hnsw_ext.rs` builds a `::hnsw create` script that requires:
  - `dim: embedding_set.dims()`
  - and creates an index relation named `embedding_set.hnsw_rel_name()`

**Implication**: remote embedding integration must ensure `active_embedding_set.shape.dimension` matches the vector length we get back from OpenRouter (including any OpenRouter-side `dimensions` override), otherwise:
- `ploke-embed` will reject vectors with `EmbedError::DimensionMismatch`
- and/or Cozo HNSW creation will be configured for the wrong dimension.

---

## Implementation steps (what to change)

### 1) Add a new embedding source for OpenRouter (via `ploke-llm`)
**Files**:
- `crates/ingest/ploke-embed/src/indexer/mod.rs` (or split into a `providers/openrouter.rs` like existing providers)
- `crates/ingest/ploke-embed/Cargo.toml` (dependency on `ploke-llm`)

**Changes**:
- Extend `EmbeddingSource` with something like:
  - `EmbeddingSource::OpenRouter(OpenRouterBackend)`

- Implement `OpenRouterBackend::compute_batch(snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError>` by adapting `ploke-llm`:
  - Build `ploke_llm::embeddings::EmbeddingRequest<ploke_llm::router_only::openrouter::OpenRouter>`
  - Set:
    - `model` to the chosen OpenRouter model (`ploke_llm::ModelId`)
    - `input = EmbeddingInput::Batch(snippets)`
    - `encoding_format = Some(EmbeddingEncodingFormat::Float)`
    - `router.dimensions = Some(dims)` **iff** we are requesting a reduced embedding size
    - (optional) `router.input_type = Some("code-snippet".into())` (OpenRouter-specific hint)
  - Call `<OpenRouter as HasEmbeddings>::fetch_embeddings(&reqwest_client, &request).await`

**Validation in the adapter (must do):**
- Ensure `resp.data.len()` equals request batch length.
- Reject `OpenRouterEmbeddingVector::Base64` unless we explicitly requested Base64.
- Ensure all returned vectors have the same length.
- Ensure returned vector length equals the pipeline’s configured dimension.
- Validate `data[i].index` and treat it as required: ensure `index == i` (or at minimum the set `{0..n-1}`) so we never mis-associate vectors with nodes if OpenRouter reorders or omits entries.
- (optional but recommended) check `resp.model.matches_request(&request.model)`.

**Type conversion choice:**
- `ploke-llm` returns `Vec<f64>`, while `ploke-embed` currently operates on `Vec<f32>`.
  - The least disruptive change is: **convert `f64 -> f32` in the OpenRouter backend**, return `Vec<Vec<f32>>`, and keep DB write path unchanged.

---

### 2) Thread the chosen embedding model/dims into `Database.active_embedding_set`
**Files**:
- Likely `crates/ploke-db/src/database.rs` (where `active_embedding_set` is established)
- The `ploke-embed` setup / CLI / config path that constructs `Database` and `EmbeddingProcessor`

**Required behavior:**
- The embedding pipeline must set `Database.active_embedding_set` from embedding configuration (single source of truth), and use the same config to construct `EmbeddingSource::OpenRouter(...)`.
- Dims handling is resolved as follows:
  - **Preferred**: specify `<dims>` in embedding config (and optionally request OpenRouter `dimensions = Some(dims)` if the model supports it).
  - **Fallback (inference)**: if dims are not configured, infer dims from the first successful OpenRouter embedding response vector length, then initialize the `EmbeddingSet`/DB relations **before** starting the indexing loop that writes vectors.
    - Practical note: because `IndexerTask` holds `db: Arc<Database>` and `Database::update_embeddings_batch` writes via `self.active_embedding_set`, dims inference must happen *prior to* the first call that persists embeddings, not mid-run.

**DB setup to ensure schema readiness:**
Before writing any vectors, ensure:
- `db.ensure_embedding_set_relation()`
- `db.put_embedding_set(&db.active_embedding_set)` if not already present
- `db.ensure_embedding_relation(&db.active_embedding_set)` (creates the vector relation with `<F32; dims>`)

Notes:
- The `EmbeddingSet::new` sanitization is critical for OpenRouter model IDs containing `/`.
- `EmbeddingSetId` is derived from provider+model+shape; changing dims creates a *different* set ID and thus a distinct relation/index.

---

### 3) Ensure HNSW index creation happens for the chosen embedding set
**Files**:
- `crates/ingest/ploke-embed/src/indexer/mod.rs` (end-of-run index creation)
- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs`

**Behavior:**
- After embeddings are inserted, call `db.create_embedding_index(&db.active_embedding_set)` (or the existing wrapper invoked by `ploke_db::create_index_warn(&db)` under `multi_embedding_embedder`).

**Invariant:**
- HNSW `dim` must equal the stored vectors’ length (`EmbeddingSet::dims()`).

---

### 4) Gate/feature alignment
Remote embeddings + multi-embedding schema should be treated as the “correct” path.

At minimum, ensure the build configuration used for remote embedding has:
- `ploke-db`: feature `multi_embedding_db`
- `ploke-embed`: feature `multi_embedding_embedder` (so index creation follows the embedding-set-aware HNSW path)

---

## Invariants to preserve

- **I1: One embedding set per run** (for now): the embedding pipeline must use a single `(provider, model, dims, dtype)` for all vectors it writes. Mixing sets in one run would require plumbing an `EmbeddingSet` through `process_batch` and DB writes explicitly.

- **I2: Vector length must equal configured dims**:
  - `ploke-embed` validates this today (`EmbedError::DimensionMismatch`).
  - `ploke-db`’s HNSW creation requires `EmbeddingSet::dims()`.

- **I3: DB relation naming must be Cozo-safe**:
  - OpenRouter model IDs contain `/`; `EmbeddingSet::new` already replaces `/` with `_slash_` and `-` with `_`.

- **I4: No empty vectors**:
  - `EmbeddingVector::validate_embedding_vec()` enforces non-empty vectors before DB insertion.

- **I5: Batch alignment**:
  - Response `data.len()` must match request batch size (otherwise we risk associating vectors with wrong nodes).

---

## Decisions (resolved) → concrete action items

### 1) Model selection lives in embedding config (single source of truth)
- Implement/configure a single embedding config object used to construct both:
  - `EmbeddingSource::OpenRouter(OpenRouterBackend { model, dims, ... })`
  - `Database.active_embedding_set: EmbeddingSet` (provider=`openrouter`, model=`<chosen>`, shape=`<dims, f32>`)

### 2) Dims strategy: accept/derive from response length (with preflight) and enforce consistency
- Treat **vector length as authoritative** for correctness, but preserve the “one embedding set per run” invariant:
  - If config specifies dims, enforce response vectors match exactly.
  - If config does *not* specify dims, run a **preflight** (single-snippet embedding call) to discover dims and then lock in `EmbeddingSet(shape.dimension = inferred)` for the run.
- If we request OpenRouter `router.dimensions`, the config dims must match the requested dims and the response must match too.

### 3) Retry/backoff: bounded, cancellation-aware
- Implement bounded retry for OpenRouter 429/529 in the OpenRouter backend:
  - Respect `Retry-After` (already parsed by `ploke-llm` for 429).
  - Use exponential backoff with a hard cap (max attempts / max elapsed).
  - Ensure retries stop promptly when `CancellationToken` is triggered.

### 4) Batch sizing + rate limiting: make it configurable, and add a semaphore-based limiter
- Ensure remote embedding has a config knob for `batch_size` (caller chooses per provider/model).
  - Longer term: `ploke-tui` can use OpenRouter model list metadata (throughput/latency hints, provider preferences) to suggest defaults.
- Add a concurrency/rate limiting mechanism in the OpenRouter backend:
  - Use a Tokio `Semaphore` (and optionally a time-based limiter) so we can reliably tune rate limits per provider/model.
  - Make limiter parameters configurable (max in-flight requests, optional requests/sec cap).

### 5) Float precision: keep `F32` end-to-end for now
- Continue storing vectors as `<F32; dims>` and keep `ploke-embed` vectors as `Vec<f32>`.
- Convert OpenRouter `f64` → `f32` in the backend, and rely on the existing DB write conversion path (`f32` → `f64`) that feeds Cozo `F32`.
- Add a TODO near the DB storage boundary to re-evaluate the extra conversions and any Cozo conversion quirks, but treat it as out-of-scope for the initial remote embedding integration.

---

## Testing & verification plan

### Unit tests (offline)
Add tests in `crates/ingest/ploke-embed` that do not require a real API key:

1) **OpenRouter adapter: parses float vectors**
- Use `httpmock` to simulate `/v1/embeddings`.
- Verify:
  - returns `Vec<Vec<f32>>` with expected batch length
  - rejects `Base64` vectors when `Float` requested (this is a runtime safety check in the `ploke-embed` adapter even though `ploke-llm` is strongly typed)
  - validates `index` fields are present and form a valid 0..n-1 mapping

2) **Dimension mismatch**
- Mock response vectors with wrong length.
- Verify `EmbeddingProcessor::process_batch` returns `EmbedError::DimensionMismatch`.

3) **DB writes are scoped to embedding set**
- With `multi_embedding_embedder` enabled, create a DB, set `db.active_embedding_set` to a non-default embedding set (provider/model/dims), ensure relations, insert a small batch, then assert:
  - `db.count_embeddings_for_set(&db.active_embedding_set)` increments
  - `db.create_embedding_index(&db.active_embedding_set)` succeeds

### Integration tests (feature-gated live)
- Add a `#[cfg(feature = "live_api_tests")]` + `#[tokio::test]` in `ploke-embed` that:
  - requires `OPENROUTER_API_KEY`
  - embeds 2–3 small snippets
  - stores them into an in-memory DB fixture
  - creates HNSW and runs one neighbor query to prove index correctness

Note (dev ergonomics): consider enabling `live_api_tests` in `ploke-embed` default features temporarily so IDE linting covers the live-gated code paths. If we keep it defaulted, ensure tests remain explicitly gated on `OPENROUTER_API_KEY` and do not run unintentionally in CI.

### Pre-run fixture gate
- Follow repo discipline: run `cargo xtask verify-fixtures` before longer test suites that depend on fixtures.

Note: also run targeted tests for the crates touched (`ploke-llm`, `ploke-db`, `ploke-embed`) regularly while iterating, not just the fixture gate.

---

## Actionable checklist (files/structs to touch)

- `crates/ingest/ploke-embed/src/indexer/mod.rs`
  - Add `EmbeddingSource::OpenRouter` variant
  - Implement backend bridging to `ploke-llm` (batch request + response parsing)
  - Ensure dims + batch alignment invariants

- `crates/ingest/ploke-embed/Cargo.toml`
  - Add dependency on `ploke-llm`

- `crates/ploke-db/src/database.rs`
  - Ensure there is an intentional/obvious way to configure `active_embedding_set` for a non-default provider/model/dims (may already exist elsewhere; confirm during implementation).

- `crates/ploke-db/src/multi_embedding/db_ext.rs`
  - Use `ensure_embedding_set_relation`, `put_embedding_set`, `ensure_embedding_relation` during pipeline setup for remote embedding.

- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs`
  - Use `create_embedding_index(&EmbeddingSet)` once vectors are inserted.

---

## Risks / edge cases to handle

- **Rate limiting / overload (429 / 529)**: without retries, indexing will fail mid-run; with retries, ensure they are bounded and respect cancellation.
  - Action: config-driven limiter (Tokio `Semaphore`) + bounded retry/backoff, so different providers/models can be tuned.
- **Partial responses** (missing items or reordered items): must not mis-associate vectors with nodes; validate response length and (if available) index fields.
  - Action: treat `data[i].index` as required; validate it is present and maps each response vector back to the correct request position.
- **Model aliasing in responses**: OpenRouter may return only slug; `EmbeddingModelName::matches_request` already handles common forms.
- **Timeouts**: use a shared `reqwest::Client` with a configured timeout; `ploke-llm` already has defaults in `EmbClientConfig` for model listing, and OpenRouter embedding fetch supports URL override.

---

## Recommendation (minimal, safe path)
- Implement a dedicated `OpenRouterBackend` in `ploke-embed` that uses `ploke-llm`’s typed request/response.
- Enable and rely on the multi-embedding DB path (`multi_embedding_db` + `multi_embedding_embedder`).
- Set `db.active_embedding_set` from the same config that chooses the OpenRouter model/dims.
- Add offline httpmock tests for the adapter + an optional live-gated end-to-end smoke test.
