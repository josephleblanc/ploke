# Embedding Pipeline Test Survey

Scope: quick inventory of the `ploke-embed` pipeline, what is currently exercised by tests, gaps/risks, and a proposed strategy for expanding coverage. No tests were run for this report.

## Pipeline Snapshot
- Entry points: `IndexerTask::run` drives batch fetch (Cozo via `ploke-db`), snippet fetch (`ploke-io`), BM25 indexing, and embedding generation via `EmbeddingRuntime` → `EmbeddingProcessor`.
- Backends: `EmbeddingProcessor` supports `LocalEmbedder`, `HuggingFaceBackend`, `OpenAIBackend`, `OpenRouterBackend`, and a placeholder `CozoBackend` (unimplemented).
- Supporting pieces: cancellation tokens, configuration structs (`config.rs`), BM25 integration, and partial indexing scaffolding (`partial` module is mostly TODO).

## Current Test Inventory
- **Core utilities**
  - `cancel_token.rs` covers basic cancel/listener behavior.
  - `runtime.rs` exercises activating an embedding set and relation creation.
  - `config.rs` asserts default OpenRouter config safety.
- **Indexer pipeline (local, fixture-driven)**
  - `src/indexer/unit_tests.rs`: fixture-backed end-to-end runs using `LocalEmbedder`, BM25 service, callback handling, and progress broadcast. Individual fixtures are gated per-test via `PLOKE_EMBED_RUN_<TEST>`; most tests are skipped by default. Some `#[ignore]` due to syn parser gaps (`file_dir_detection`, `spp_edge_cases_*`). Coverage includes path resolution, generics/macros, tracking hashes, and duplicate name handling. Tests also seed multi-embedding/HNSW via `setup_db_full_*`, but pause/resume/cancel control paths are unused.
- **Vector storage/search**
  - `tests/test_vector_functionality.rs`: Cozo vector column CRUD, HNSW index creation, graph traversal, and similarity search (including higher-dimension code embeddings).
- **OpenRouter backend**
  - `providers/openrouter.rs` unit tests (mocked via `httpmock`) validate response ordering, type enforcement (float vs base64), index presence/range, and basic retry/backoff plumbing (single-attempt config).
- **Live OpenRouter e2e (feature `live_api_tests`)**
  - `tests/openrouter_live_fixture_tracking_hash.rs`: requires real `OPENROUTER_API_KEY` and forbids overriding the endpoint; runs `IndexerTask` against fixtures, asserts vector dims, HNSW creation, and cosine parity against local vectors. Writes artifacts under `target/test-output/...`.
  - `tests/openrouter_live_fixture_nodes_e2e.rs`: similar live gate; indexes fixture nodes end-to-end and records artifacts.
- **Helpers**
  - `tests/test_helpers.rs` provides in-memory Cozo setup and debug printers used by vector tests.

## Coverage Gaps / Risks
- Default CI coverage is shallow: most indexer fixture tests are env-gated; live OpenRouter tests require feature flag + API key; ignored tests mean cfg-heavy parsing remains unvalidated.
- No tests for `HuggingFaceBackend` or `OpenAIBackend` request/response handling; `EmbeddingProcessor` error paths for these sources are unexercised.
- `EmbeddingRuntime` lacks concurrency/active-set switching tests beyond a single activation case.
- `IndexerTask` control flow for pause/resume/cancel, cursor persistence, and failure recovery (I/O or DB errors) is not covered.
- Partial/incremental indexing (`partial` module, `update_embeds` flag) is unimplemented and untested.
- Local embedder only covered indirectly; no targeted checks for device selection, dimension mismatches, or token-length limits.
- Error handling/backoff for OpenRouter is partially covered; retry logic with rate limiting/provider overload is not asserted.

## Strategy to Add Tests
1) **Raise default offline coverage (CI-friendly)**
   - Un-gate one small `IndexerTask` smoke test using an ultra-light fixture (single file) to exercise the full loop with `LocalEmbedder`, BM25, and callback handling; assert progress sequencing, vector length, and DB row counts. Keep heavier fixtures behind env flags.
   - Add `EmbeddingRuntime` unit tests for switching active sets and verifying `dimensions()` tracks the active processor.
   - Add cancellation-focused tests: feed `CancellationListener` into `EmbeddingProcessor::generate_embeddings_with_cancel` for OpenRouter mock + Local embedder to ensure early exit and no vector writes.
2) **Provider contract tests (mocked)**
   - Mirror the OpenRouter mock pattern for `OpenAIBackend` and `HuggingFaceBackend`: validate request payloads (model, dimensions/input_type), index ordering, float/base64 validation, and retry/backoff branches.
   - Expand OpenRouter mock tests to cover rate-limit and provider-overload retries (assert backoff sequencing without sleeping by injecting a test scheduler).
3) **DB/vector behavior**
   - Extend `test_vector_functionality` with round-trips for multi-embedding sets and dimension-mismatch errors to guard HNSW registration code paths.
   - Add regression tests for tracking-hash updates without live API by stubbing embeddings (use `EmbeddingProcessor::new_mock()` once results are meaningful or add a deterministic mock source).
4) **Live gates discipline**
   - Keep live OpenRouter tests under `live_api_tests`; document expected artifacts under `target/test-output/...` and add a minimal matrix (2 models × 1 fixture) to verify dims override and cosine sanity. Run `cargo xtask verify-fixtures` before live runs.
5) **Future (once partial indexing lands)**
   - Table-driven tests for `partial/index.rs` covering insert/update/delete flows keyed by tracking hash, plus failure injection on DB writes to confirm rollback behavior.

## Invariants Observed (early)
- Embedding set activation expects database relations registered before swapping the active set; `EmbeddingRuntime::activate` enforces ordering (ensure relation, persist set, ensure vector relation, set active).
- OpenRouter backend requires embeddings to include `index` for reordering and to return float vectors of expected dimension; duplicates/missing indices are treated as hard errors.
- Cancellation listeners must be created before triggering the handle to propagate cancellation to OpenRouter requests.

## Progress Log
- v0.1: Baseline survey captured; identified provider/runtime gaps and mapped existing tests. Plan to add offline provider contract tests, runtime active-set checks, and cancellation coverage next.
- v0.2: Added OpenRouter cancellation test (`cancels_before_sending_request`) and runtime active-set swap test (`tests/runtime_active_set.rs`) that exercises embedder swaps via mocked OpenRouter responses. Tests run: `cargo test -p ploke-embed cancels_before_sending_request`, `cargo test -p ploke-embed runtime_swaps_active_set_and_embedder_dimensions` (both passing; existing warnings unrelated).

Execution notes: prefer typed structs/enums for any test helpers touching OpenRouter/OpenAI/HF payloads; avoid stringly typed JSON. Use static dispatch and mocks over trait objects. Keep new fixtures small to maintain runtime, and gate long e2e runs via env flags instead of `#[ignore]` where possible.
