## Remote embeddings (OpenRouter) fixture validation test plan: `fixture_tracking_hash`

### Purpose
Validate that the new **OpenRouter remote embedding pipeline** correctly:
- Generates embeddings for a real (small) fixture crate (`tests/fixture_crates/fixture_tracking_hash/`).
- Stores vectors into the correct **multi-embedding** relations for the configured embedding set.
- Builds a valid **HNSW** index for that embedding set.
- Produces consistent results across runs (determinism expectations) and handles ordering/dimension invariants.
- (Optional but ideal) Demonstrates **local vs remote model parity** when the *same* model is available both locally and on OpenRouter.

### Scope
This plan targets the new OpenRouter-backed embedding source in `crates/ingest/ploke-embed`.

It assumes:
- Multi-embedding schema is enabled (feature `multi_embedding_embedder` in `ploke-embed`, `multi_embedding_db` in `ploke-db`).
- Live API calls are explicitly gated (e.g. `#[cfg(feature = "live_api_tests")]` and `OPENROUTER_API_KEY`).

### Inputs
- **Fixture crate**: `tests/fixture_crates/fixture_tracking_hash/`
- **OpenRouter embedding models fixture**: `fixtures/openrouter/embeddings_models.json`

From `embeddings_models.json`, recommended model set for coverage:
- **Parity candidate (384 dims, local-friendly)**:
  - `sentence-transformers/all-minilm-l6-v2` (384)
- **Dimensions-override (grounded in current code)**:
  - `openai/text-embedding-3-small` with requested `dimensions=256` (tests OpenRouter-side truncation)
  - (Optional) `openai/text-embedding-3-small` with requested `dimensions=384` (matches local default dims; useful for parity scaffolding)
  - (Optional) `openai/text-embedding-3-small` with requested `dimensions=512` (bigger than local default, still fully specified)

**Important constraint (current implementation)**: `OpenRouterBackend` currently requires `OpenRouterConfig.dimensions` to be set.
“Infer dims from model id” is not wired yet (see backend error message); therefore, tests must always specify an explicit dimension.

### Live-gate discipline
These tests must not be reported “green” unless:
- `OPENROUTER_API_KEY` is present.
- The code path actually calls the OpenRouter endpoint.
- Artifacts prove the call happened (captured response metadata and/or request/response observation via test logs or recorded output).

If `OPENROUTER_API_KEY` is missing, tests should **explicitly fail fast** (panic) with a message indicating the live gate is not satisfied.
Rationale: Rust tests do not have a first-class “skip” mechanism, and silently passing violates live-gate discipline.

---

## Test layers

### Layer 0: Preconditions / environment sanity

- **Env**:
  - `OPENROUTER_API_KEY` set
  - (Optional) `OPENROUTER_EMBEDDINGS_URL` unset (to ensure we hit the real endpoint)

- **Build flags**:
  - `ploke-embed`: `--features "multi_embedding_embedder live_api_tests"`

- **Fixture gate** (repo discipline):
  - Run `cargo xtask verify-fixtures`.
  - If it fails due to unrelated fixture drift (e.g. pricing fixture, backup drift), document it as a known blocker for CI-style validation, but continue with the targeted `ploke-embed` live tests.

**Expected result**: environment is ready; build succeeds.

---

## Layer 1: Minimal live smoke test (single batch)

### Goal
Prove the OpenRouter embedding adapter works end-to-end for a tiny input set and returns:
- correct vector count
- correct dimensionality

### Test case
**`live_openrouter_embed_two_snippets_smoke`** (new, feature-gated)

- Create `EmbeddingProcessor` with `EmbeddingSource::OpenRouter`.
- Use model `sentence-transformers/all-minilm-l6-v2` and `dimensions=384`.
- Embed 2–3 short strings.

**Assertions**:
- returned `Vec<Vec<f32>>.len() == input_len`
- every vector `len() == 384`
- all floats finite

**Note on ordering semantics**:
The OpenRouter adapter already validates and reorders by `data[i].index` internally.
Live tests cannot observe the raw response order (only the post-processed output).
The raw ordering semantics should remain covered by the existing mocked unit test in
`crates/ingest/ploke-embed/src/providers/openrouter.rs`.

**Artifacts**:
Write `target/test-output/openrouter_embed_smoke/<timestamp>.json` with:
- model id
- dims
- batch size
- first 8 floats of each vector (for basic inspection)

---

## Layer 2: Fixture crate indexing run (multi-embedding)

### Goal
Prove that indexing `fixture_tracking_hash` with remote embeddings:
- populates the correct embedding-set relation
- updates pending counts to zero for that set
- builds an HNSW index with matching dims

### Test case
**`live_openrouter_index_fixture_tracking_hash_builds_vectors_and_hnsw`** (new, feature-gated)

**Setup**:
- Create DB via existing fixtures (recommended):
  - `ploke_test_utils::setup_db_full_multi_embedding("fixture_tracking_hash")`
- Set `db.active_embedding_set` to:
  - provider: `openrouter`
  - model: chosen model id
  - shape: `<F32; dims>`
  - **Important**: set `active_embedding_set` *before* wrapping the DB in `Arc` and handing it to the indexer.
    Do not mutate `active_embedding_set` after the indexer starts.

**Run**:
- Instantiate `IoManagerHandle`.
- Instantiate `IndexerTask` using:
  - `EmbeddingProcessor(EmbeddingSource::OpenRouter(...))`
  - small batch size (e.g. 4 or 8)
- Run the indexer until completion.
- Then create HNSW via `ploke_db::multi_embedding::hnsw_ext::HnswExt::create_embedding_index(&active_set)`.

**Assertions**:
- `db.count_pending_embeddings() == 0` (active set pending should reach zero)
- `count_embeddings_for_set(&active_set) > 0` (multi-embedding count helper)
- embedding relation exists and name is Cozo-safe:
  - `active_set.rel_name()` contains `_slash_` substitutions where expected
- HNSW index exists for the active set:
  - `is_hnsw_index_registered(&active_set) == true`

**Artifacts**:
Write `target/test-output/openrouter_fixture_tracking_hash/<model>/<timestamp>.json`:
- embedding set identity (provider/model/dims/hash_id/rel_name)
- before/after counts (pending, embedded)
- HNSW index creation result
- a small sample of node ids that received embeddings

---

## Layer 3: Multi-model matrix against the same fixture

### Goal
Prove the pipeline is robust across different model families and dimensions.

### Test case
**`live_openrouter_matrix_fixture_tracking_hash`** (new, feature-gated)

Run Layer 2 multiple times against the **same fixture crate**, but use a **fresh DB per run**.

Reason: the current codebase relies on `Database.active_embedding_set` for read/write selection of embeddings.
Switching `active_embedding_set` on a shared `Database` is unsafe and/or infeasible once it is in `Arc` and used by tasks.

Recommended matrix:
- `sentence-transformers/all-minilm-l6-v2` @ 384 (parity candidate)
- `openai/text-embedding-3-small` @ 256 (dimensions override path)
- (Optional) `openai/text-embedding-3-small` @ 384 (parity scaffolding)
- (Optional) `openai/text-embedding-3-small` @ 512 (additional coverage)

**Key invariant**: “one embedding set per run” is preserved by constructing a fresh DB per run and setting
`db.active_embedding_set` once, prior to running the indexer.

**Assertions** per run:
- counts go to 0 for that set
- HNSW builds successfully
- relation names do not collide (different rel_name/hash_id per model/dims)

**Artifacts**:
Write a single summary JSON:
`target/test-output/openrouter_fixture_tracking_hash/matrix/<timestamp>.json`
containing per-model results:
- dims
- embedded count
- elapsed time
- elapsed time
- retry occurrences (if any; only if exposed via logs/metrics)

---

## Layer 4 (Ideal): Local vs remote parity test (same model)

### Why this is feasible
The OpenRouter model fixture includes `sentence-transformers/all-minilm-l6-v2` (384 dims), which is close to Ploke’s default local model (`sentence-transformers/all-MiniLM-L6-v2`). Even if casing differs, this still targets the *same family* and is the best parity candidate available in-repo.

### Goal
Validate that local and remote embeddings for the same input text are **nearly identical**.

### Test cases

#### A) Single-vector cosine similarity
**`live_openrouter_vs_local_all_minilm_l6_v2_cosine_similarity`** (new, feature-gated)

**Setup**:
- Choose a fixed set of 10 short snippets (Rust-ish) + 5 plain-English sentences.
- Generate embeddings locally using `LocalEmbedder` configured with:
  - `model_id = "sentence-transformers/all-MiniLM-L6-v2"`
- Generate embeddings remotely using OpenRouter:
  - `model = "sentence-transformers/all-minilm-l6-v2"`
  - `dimensions = 384`

**Compute**:
- Normalize both vectors to unit length.
- Cosine similarity per pair.

**Assertions** (suggested thresholds; tune after first run):
- `p50(sim) >= 0.99`
- `min(sim) >= 0.97`
- If any similarity is below threshold, store the text + similarity + small vector excerpt in artifacts.

**Stability note**:
These parity tests are expected to be flaky or expensive (local model download/init, remote provider variations).
Treat as **non-blocking** initially (e.g., `#[ignore]` or separate feature like `parity_live_tests`) until baselined.

**Artifacts**:
`target/test-output/openrouter_parity/all_minilm_l6_v2/<timestamp>.json`:
- per-snippet cosine
- summary stats
- model ids (local and remote)

#### B) Retrieval parity (neighbor overlap)
**`live_openrouter_vs_local_all_minilm_l6_v2_neighbor_overlap`** (new, feature-gated)

**Approach**:
- Run indexing twice on the same fixture DB, producing two embedding sets:
  - Set A: local embedding set (provider `local`, model `sentence-transformers/all-MiniLM-L6-v2`, dims 384)
  - Set B: openrouter embedding set (provider `openrouter`, model `sentence-transformers/all-minilm-l6-v2`, dims 384)
- Build HNSW for both sets.
- For a few query nodes/snippets, perform `top_k=10` similarity search in both sets.

**Assertions**:
- Jaccard overlap of top-10 IDs is high (suggested >= 0.7 for most queries).
- Rank correlation is “reasonable” (optional; Spearman on overlapping IDs).

**Artifacts**:
`target/test-output/openrouter_parity/all_minilm_l6_v2_retrieval/<timestamp>.json`:
- queries
- top_k results for each set
- overlap metrics

---

## Layer 5: OpenRouter `dimensions` override correctness

### Goal
Prove that requesting reduced dimensions yields:
- vectors of the requested size
- DB schema uses `<F32; dims>` matching the reduced size
- HNSW dim matches

### Test case
**`live_openrouter_dimensions_override_text_embedding_3_small_256`** (new, feature-gated)

- Model: `openai/text-embedding-3-small`
- Requested `dimensions = 256`
- Run the same fixture indexing flow.

**Assertions**:
- all vectors are length 256
- embedding-set relation name includes the requested dims in the relation name (via `EmbeddingSet::new` naming),
  e.g. ends with `_256` and uses `_slash_` for any `/` in the model id
- HNSW created with dim 256

**Artifacts**:
`target/test-output/openrouter_dimensions_override/<timestamp>.json`

---

## Failure diagnostics checklist

If a test fails, capture:
- The chosen embedding set (provider/model/dims/hash_id/rel_name)
- The DB counts before/after
- Raw OpenRouter error mapped (status/body/retry-after)
- Whether response `data[i].index` mapping was valid
- Whether the index was created, and under what relation name

---

## Suggested commands

### Local/offline (adapter) tests
```bash
cargo test -p ploke-embed
```

### Live API tests (only when key present)
```bash
OPENROUTER_API_KEY=... cargo test -p ploke-embed --features live_api_tests
```

### Fixture crate focus (if a dedicated test runner is introduced)
```bash
OPENROUTER_API_KEY=... cargo test -p ploke-embed --features live_api_tests -- --nocapture
```

---

## Notes / open questions

- **Parity thresholds**: 0.99 cosine similarity may be realistic for identical model + identical preprocessing, but may need adjustment if OpenRouter’s hosting differs (normalization, tokenization, pooling). The plan is designed to *measure and record* the distribution first, then set stable thresholds.
- **Casing/ID differences**: local model id uses `all-MiniLM-L6-v2` while OpenRouter fixture uses lowercase `all-minilm-l6-v2`. Treat as the same intended model but verify empirically.
- **Fixture drift gate**: `cargo xtask verify-fixtures` currently fails in this repo due to missing pricing fixture and backup drift. This plan’s live tests should still be runnable and produce artifacts, but CI-readiness requires fixing those fixture issues separately.
