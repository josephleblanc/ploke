# Remote Vector Indexing Support – Research Report (2025-02-18)

> Related planning references: `README.md` feature list (`README.md:264`) and the active agentic-system plans under `crates/ploke-tui/docs/feature/agent-system/agentic_system_plan.md`.

## Context
- The top-level roadmap calls out “Support for remote vector indexing + user-selected vector indexing models” as a priority feature, but no work has landed yet (`README.md:264`).
- Remote indexing touches every layer of the pipeline: vector generation (`crates/ingest/ploke-embed`), storage/indexing (`crates/ploke-db`, `crates/ingest/ploke-transform`), and the user entry points in `ploke-tui`.
- This report surveys the current implementation and outlines the gaps and concrete steps required to ship the feature.

## Current Implementation Snapshot

### Embedding pipeline (`ploke-embed`)
- `EmbeddingSource` already enumerates `Local`, `HuggingFace`, `OpenAI`, and a placeholder `Cozo` backend, and `EmbeddingProcessor::generate_embeddings` simply delegates to the selected backend (`crates/ingest/ploke-embed/src/indexer/mod.rs:32-96`).
- Backend configs live in `config.rs`, but only `HuggingFaceConfig` carries an explicit `dimensions` field; `OpenAIConfig` only stores `api_key` and `model` (`crates/ingest/ploke-embed/src/config.rs:3-24`).
- `OpenAIBackend::new` hard-codes `dimensions` to 1536 (text-embedding-ada-002) regardless of the configured model, so any attempt to select a newer embedding model (e.g., `text-embedding-3-large`, 3072 dims) would corrupt downstream invariants (`crates/ingest/ploke-embed/src/providers/openai.rs:8-55`).
- `HuggingFaceBackend` expects the user to supply the correct dimensionality manually, but there is no validation against the remote response (`crates/ingest/ploke-embed/src/providers/hugging_face.rs:24-65`).
- The `CozoBackend` is a stub that always returns `EmbedError::NotImplemented`, so it cannot act as a remote indexing target yet (`crates/ingest/ploke-embed/src/indexer/mod.rs:100-120`).
- Error handling for remote backends is minimal: HTTP status/body are surfaced, but there is no rate-limit backoff, retry budget, or batching strategy beyond “send the entire snippet batch in one POST”.

### Storage and index schema (`ploke-db`, `ploke-transform`)
- Every primary schema entry in `ploke-transform` stores embeddings as `<F32; 384>?`, so the dimension is effectively baked into the database schema (`crates/ingest/ploke-transform/src/schema/primary_nodes.rs:13-80`).
- HNSW index creation mirrors that assumption: `create_index`, `create_index_warn`, and `replace_index_warn` always emit `dim: 384`, `dtype: F32` when creating per-relation indexes (`crates/ploke-db/src/index/hnsw.rs:411-485`).
- `Database::index_embeddings` technically accepts a `dim` parameter, but that path is unused by the TUI, and the schema still constrains the stored vectors (`crates/ploke-db/src/database.rs:526-555`).
- There is no per-workspace metadata that records which embedding model populated the vectors or what dimension the HNSW structures expect, so switching models mid-stream risks corrupting the index silently.

### TUI integration & configuration (`ploke-tui`)
- `UserConfig::embedding` allows picking exactly one backend (local, Hugging Face, OpenAI, Cozo), and `UserConfig::load_embedding_processor` builds the embedder accordingly (`crates/ploke-tui/src/user_config.rs:80-180`).
- At startup `try_main` instantiates a single `EmbeddingProcessor`, wires it into both the `IndexerTask` and the RAG service, and stores it inside `AppState` (`crates/ploke-tui/src/lib.rs:171-217`).
- There is no runtime command, overlay, or UI surface to edit embedding settings. The only option is to edit `~/.config/ploke/config.toml` manually and run `/model load`. Even then, the executor warns that “restart [is] recommended” because the embedder cannot be hot-swapped (`crates/ploke-tui/src/app/commands/exec.rs:80-120`).
- The command parser/executor has zero embedding-specific commands (`crates/ploke-tui/src/app/commands/parser.rs:17-118`), and the indexing handler simply reuses whatever embedder `AppState` was constructed with (`crates/ploke-tui/src/app_state/handlers/indexing.rs:14-86`).
- `StateCommand::IndexWorkspace` has no flags for selecting a backend or model per run, and there is no feedback about which embedding provider is active when `/index start …` executes.

## Gap Analysis
1. **Fixed 384-dimension schema** – All schema definitions and HNSW builders assume 384-d vectors. Remote providers routinely return 512/768/1536/3072 dims, so supporting “user-selected indexing models” requires schema changes and automatic re-indexing.
2. **No metadata about embedding provenance** – Without recording the model/provider that produced stored vectors, switching providers leaves mixed embeddings in the same tables and invalid HNSW indexes.
3. **Backend configuration UX** – Users cannot switch between local/remote backends or select remote models from within the TUI; manual config edits plus app restarts are the only option.
4. **Runtime embedder lifecycle** – `ploke-tui` creates the embedder once and shares it between indexing and query-time retrieval. Remote support demands the ability to rebuild the embedder (and `IndexerTask`/`RagService`) after a configuration change.
5. **Remote resilience** – Remote providers need rate limiting, retries, cost tracking, and smaller batch sizes. None of those concerns are addressed in the current `compute_batch` implementations.
6. **Docs/tests** – There is no user-facing documentation for setting up remote embeddings, nor integration tests that exercise remote backends, even behind feature flags (`docs/plans/embedding_pipeline/short_plan.md:1-63` still lists documentation/test TODOs).

## Recommendations

### 1. ploke-embed
- **Introduce typed backend registry**: Model remote backends via a trait (`EmbeddingService`) that exposes metadata (model id, dimension, max batch) and enrich `EmbeddingProcessor` with dynamic dispatch over boxed trait objects. That will simplify adding additional providers (Cohere, custom HTTP) later.
- **Make backend configs strongly typed**: Extend `OpenAIConfig` with explicit `dimensions`, `endpoint`, and optional `encoding_format`; add validation that compares configured dims against the actual response shape before persisting embeddings.
- **Add rate limiting and retries**: Wrap remote calls with a lightweight limiter (e.g., `tokio::sync::Semaphore`) and exponential backoff for HTTP 429/5xx responses. Surface retry counts via `IndexingStatus.errors`.
- **Surface provider metadata**: Augment `EmbeddingProcessor` with a `provider_id()` accessor and pass it through to the DB layer so the model id can be persisted alongside each embedding batch.
- **Cozo backend**: Decide whether to implement the placeholder as a remote inference endpoint or remove it until it exists; the current stub raises `NotImplemented` and complicates config validation.

### 2. ploke-db & schema
- **Parameterize embedding dimension**: Replace the `<F32; 384>` literals in `ploke-transform` schemas with a symbolic dimension (e.g., store vectors as dynamic `Bytes` blobs or use multiple relations keyed by dimension). At minimum, generate schema strings from a runtime `EmbeddingShape` struct so dimension changes propagate consistently (`crates/ingest/ploke-transform/src/schema/primary_nodes.rs:13-80`).
- **Dynamic HNSW creation**: Rewrite `create_index*_` helpers to accept an `EmbeddingShape` (dim + metric) and emit the correct `::hnsw create` script instead of hard-coding `dim: 384` (`crates/ploke-db/src/index/hnsw.rs:411-485`). Keep `Database::index_embeddings` as the single entry point so the TUI can rebuild indexes when the backend changes.
- **Persist embedding metadata**: Add a small table (e.g., `embedding_metadata { workspace: String => model: String, dimension: UInt32, updated_at: Int }`) that records which backend populated the current vectors. Index rebuild commands can consult that table to detect mismatches before writing.
- **Migration path**: Because schema changes are breaking, document a migration command that drops embeddings/HNSW indexes and reindexes with the new backend. For developer builds, gating remote support behind a feature flag until migrations stabilize may be prudent.

### 3. ploke-tui UX & commands
- **Config UI**: Add `/embedding status`, `/embedding use <local|openai|hf|cozo>`, and `/embedding model <id>` commands so users can switch providers without editing files. These commands should update `state.config`, persist via `UserConfig::save_to_path`, and trigger a controlled embedder rebuild.
- **Embedder lifecycle management**: When the embedding config changes, tear down the existing `IndexerTask`, build a new `EmbeddingProcessor`, and rehydrate dependent services (RAG, indexing control channels). This requires moving `EmbeddingProcessor` construction out of `try_main` into a restartable component and storing factories rather than raw `Arc`s.
- **Per-run overrides**: Extend `/index start` to accept flags such as `--backend openai --model text-embedding-3-large`. If provided, clone the runtime config, resolve env vars (API keys), and spin up a temporary embedder for that run.
- **Status reporting**: Include the active backend/model/dimension in the indexing progress HUD so users know whether a remote provider is being used (`crates/ploke-tui/src/app_state/handlers/indexing.rs:14-86`). Surface remote errors (HTTP codes, retries) through `IndexingStatus.errors` and the chat panel.

### 4. Testing, docs, and ops
- **Docs**: Update `crates/ploke-tui/README.md` and add a provider setup guide under `crates/ploke-tui/docs/` that covers API key management, sample `config.toml` snippets, and expected costs/limits.
- **Tests**: Introduce mocked remote backends (e.g., via `wiremock`) so unit tests can validate error handling without live API calls. Gate optional live tests behind a `cfg(feature = "live_api_tests")`, as already planned in the agentic-system TODO list.
- **Telemetry**: For remote runs, emit per-batch timing and cost estimates to `target/test-output/...` artifacts so future status reports can prove readiness, in line with the project’s evidence-based change policy.

## Suggested Next Steps
1. **Schema/dimension groundwork** – Implement dynamic embedding shapes in `ploke-transform` and `ploke-db`, and add a CLI command to drop/rebuild embeddings so future runs can cleanly switch providers.
2. **Backend configuration plumbing** – Extend `OpenAIConfig`/`HuggingFaceConfig`, add validation, and expose the settings through new `/embedding …` commands. Ensure `AppState` can rebuild the embedder without a process restart.
3. **Remote robustness** – Add retry/limiter logic to remote providers and enrich `IndexingStatus.errors` with actionable diagnostics so the UX remains predictable under network failures.
4. **Docs + samples** – Write a provider setup guide and ship a sample `config.toml` with commented remote embedding options so early adopters can experiment.

Addressing these items will unlock the “remote vector indexing” feature end-to-end and lay the foundation for additional providers or managed indexing services later. Once the dynamic schema and command UX exist, extending to other embedding APIs becomes mostly a matter of plugging in new backend structs and updating the configuration registry.
