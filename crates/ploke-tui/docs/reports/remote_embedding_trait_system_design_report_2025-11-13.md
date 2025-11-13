# Remote Embedding Trait System Design Report (2025-11-13)

## Inputs Reviewed
- `remote_vector_indexing_support_report_2025-02-18.md` documents the outstanding work for remote indexing (dynamic dimensions, provenance metadata, runtime config) across embedder, DB, and TUI layers (`crates/ploke-tui/docs/reports/remote_vector_indexing_support_report_2025-02-18.md:5-35`).
- `deep-research_remote_embedding.md` captures OpenAI/Hugging Face request shapes, batching constraints, and rate limits plus an initial trait sketch for embedding services (`crates/ploke-tui/docs/reports/deep-research_remote_embedding.md:3-125`).
- The existing LLM router traits (`Router`, `HasModels`, `ChatCompRequest`, `WireRequest`) show how we generalize OpenRouter calls to future providers (`crates/ploke-tui/src/llm/router_only/mod.rs:52-315`, `crates/ploke-tui/src/llm/wire.rs:18-48`).
- Current embedding executor code still relies on ad-hoc enums and direct HTTP clients without shared abstractions (`crates/ingest/ploke-embed/src/indexer/mod.rs:32-118`, `crates/ploke-embed/src/providers/{hugging_face,openai}.rs`, `crates/ploke-tui/src/user_config.rs:81-180`).

## Problem Statement
Remote embeddings must (a) talk to multiple providers with different shapes, (b) propagate provider metadata into database schemas/index builders, and (c) let the TUI reconfigure providers at runtime. Today we have fixed 384-dimension schemas, no provenance tracking, single-startup embedder instantiation, and per-provider bespoke HTTP structs, which blocks remote indexing and makes it hard to extend beyond OpenAI/Hugging Face (`crates/ploke-tui/docs/reports/remote_vector_indexing_support_report_2025-02-18.md:20-35`). We need a trait-based system mirroring the chat router layer so that remote embedding support inherits strong typing, config ergonomics, and testing hooks from the agentic roadmap.

## Key Findings From Research
- **API Surface Differences** – Hugging Face encodes the model ID in the URL and expects `{"inputs": [...], "options": {...}}`, while OpenAI uses a fixed `/v1/embeddings` endpoint with `{ "model": "...", "input": [...] }`. Both return vectors but at different lengths (384–3072+) and have strict RPM/TPM quotas that require batching + backoff (`crates/ploke-tui/docs/reports/deep-research_remote_embedding.md:3-117`).
- **Schema + Lifecycle Gaps** – The DB schemas, HNSW builders, and RAG/index runtimes are hard-coded to 384-d vectors, do not store provider/model metadata, and cannot rebuild an embedder in response to config changes (`crates/ploke-tui/docs/reports/remote_vector_indexing_support_report_2025-02-18.md:20-33`).
- **LLM Router Lessons** – The chat stack isolates router-specific fields behind the `Router` trait, flattens strongly typed request bundles into JSON via `ChatCompRequest`, and centralizes HTTP construction in `WireRequest`, making it easy to slot in OpenRouter vs. Anthropic without changing callsites (`crates/ploke-tui/src/llm/router_only/mod.rs:52-315`, `crates/ploke-tui/src/llm/wire.rs:18-48`).

## Proposed Architecture

### Goals
1. **Strongly typed provider boundary** – No raw `serde_json::Value` in application logic; every request/response must be represented by structs + enums with `Serialize`/`Deserialize`.
2. **Router-like extensibility** – Adding Cohere, Azure OpenAI, or managed Cozo endpoints should be “implement a trait + register,” not “fork the embedder.”
3. **First-class metadata** – Model ID, provider slug, dimensions, encoding format, rate-limit policy, and batch caps must be propagated through `EmbeddingProcessor`, persisted in DB metadata, and surfaced in the TUI.
4. **Runtime configurability** – `/embedding …` commands should rebuild embedders and update dependent services without restarting the TUI.
5. **Evidence + safety** – Apply the IoManager hash-checked writes, route remote tests through `target/test-output/...`, and gate live endpoints behind feature flags per AGENTS.md.

### Trait Stack

| Trait | Purpose | Notes |
|-------|---------|-------|
| `EmbeddingRouter` | Analogue to `Router`: defines associated request/response types, API constants, and model ID encoding. | e.g. `const BASE_URL`, `const EMBEDDING_URL`, `const MODELS_URL`, `const API_KEY_ENV`. |
| `HasEmbeddingModels` | Fetch + normalize provider model catalogs to `EmbeddingModelRecord`, similar to `HasModels`. | Returns strongly typed metadata (dimensions, context length, pricing, TASK tags). |
| `EmbeddingRequest<R>` | Generic request builder parameterized by `R: EmbeddingRouter`; flattens common batch fields (`inputs`, `encoding`, `user`) and router-specific envelopes. | Mirrors `ChatCompRequest<R>` pattern with builder helpers. |
| `EmbeddingParameters` | Shared knobs (batch_size, max_rps, wait_for_model, encoding_format). | Lives in `crates/ploke-tui/src/embedding/types/params.rs` to parallel `LLMParameters`. |
| `EmbeddingWireRequest` | HTTP envelope (URL, headers, body, evidence metadata) reused by background tasks. | Extends `WireRequest` with `expected_dimensions`, `encoding_format`, `cost_hint`. |
| `EmbeddingProvider` | Runtime object stored in `EmbeddingProcessor`. Wraps `Arc<dyn EmbeddingService>` produced from config + registry metadata, exposing `async fn embed(batch)`. | Enables hot-swapping by reconstructing from `EmbeddingConfig`. |

These traits live in a new `crates/ploke-tui/src/embedding/` module so we can co-locate registry data, configs, and tests. We can progressively migrate `ploke-embed` to consume these traits by having `EmbeddingProcessor` depend on `dyn EmbeddingService` rather than a large enum, preserving backward compatibility until the old variants are deleted.

### Type System + Metadata
- **Model Identity** – Introduce `EmbeddingModelId` and `EmbeddingProviderSlug` newtypes mirroring `ModelId`/`ProviderSlug` so we can re-use hashing, display, and parsing logic. These feed into `EmbeddingRouter::RouterModelId` for URL-safe formatting just like `OpenRouterModelId` does for chat (`crates/ploke-tui/src/llm/router_only/openrouter/mod.rs:70-137`).
- **Shape Tracking** – Define `EmbeddingShape { dimension: u32, dtype: EmbeddingDType, encoder: EncodingFormat }` and thread it through `EmbeddingRequest`, DB metadata rows, and `EmbeddingProcessor::dimensions()`. This solves the “fixed 384” gap noted in the remote indexing report by making schema builders parameterized instead of literal constants.
- **Config + Registry** – Extend `UserConfig::embedding` to include a `router` field (OpenRouter-style aggregator vs. direct provider) plus per-provider structs that derive `Serialize`/`Deserialize` for strong typing (`crates/ploke-tui/src/user_config.rs:81-180`). Add an `EmbeddingRegistry` file (parallel to `llm::registry`) that caches provider catalogs, filters models by tooling requirements (e.g., JSON output), and surfaces defaults to `/embedding status`.
- **Error + Rate Limit Types** – Expand `EmbedError` with structured variants (`RateLimited { retry_after, quota }`, `DimensionsMismatch { expected, got }`) so the TUI can give actionable messages and the indexing HUD can surface remote failures.

### Request / Response Flow
1. `EmbeddingCommand` (new TUI command) resolves the desired provider and model via the registry, producing an `EmbeddingProviderConfig`.
2. `EmbeddingProviderFactory` builds the router-specific struct (e.g., `HuggingFaceRouter`) → `EmbeddingRequestBuilder::new(router).with_inputs(snippets)…`.
3. The builder produces an `EmbeddingWireRequest` with the right headers (HF bearer token vs. OpenAI `Authorization`), request body, and `expected_dimensions`.
4. `EmbeddingProcessor::generate_embeddings` simply forwards to `dyn EmbeddingService`, which orchestrates retries/backoff (leveraging rate-limit data from `EmbeddingParameters` and the `deep-research` report).
5. Responses parse into typed structs (`OpenAiEmbeddingResponse`, `HuggingFaceEmbeddingResponse`), normalize to `Vec<EmbeddingVector>`, and annotate each batch with provenance.
6. The DB layer consumes `(EmbeddingShape, EmbeddingProvenance)` to build HNSW indexes with the correct `dim`/`dtype` and persist metadata rows so mismatches are caught before writes.

### Resiliency + Observability
- **Batching + Backoff** – Include a `BatchPolicy` on each router to enforce provider quotas (e.g., ≤8191 tokens per request, 200 RPM for HF free tier per `deep-research_remote_embedding.md:15-21`). Integrate with `tokio::Semaphore` inside the provider implementation.
- **Retry Budget** – On 429/5xx, apply exponential backoff up to `max_attempts`, log to `IndexingStatus.errors`, and write structured spans so we can reference them in evidence artifacts.
- **Telemetry Artifacts** – Each indexing run writes a `target/test-output/embedding/<timestamp>.json` blob summarizing provider name, model, batch counts, error tallies, and average latency to align with the “evidence-based changes” principle in AGENTS.md.
- **Config Hot Reload** – Use an `EmbeddingManager` similar to `llm::manager` so `/embedding use openai --model text-embedding-3-small` tears down the existing processor, rebuilds dependencies, and informs the UI banner which provider is active.

## Implementation Plan
1. **Module + Trait Scaffolding**
   - Create `crates/ploke-tui/src/embedding/{mod.rs,router.rs,types,wire.rs}` with the trait stack above plus unit tests that serialize/deserialize representative requests.
   - Mirror the structure of `llm::router_only` so future contributors recognize the pattern.
2. **Provider Implementations**
   - Port the existing `HuggingFaceBackend` and `OpenAIBackend` into router implementations that satisfy `EmbeddingRouter + EmbeddingService`, ensuring they report true dimensions instead of hard-coded 384/1536 (`crates/ploke-embed/src/providers/{hugging_face,openai}.rs`).
   - Add validation against live responses (optional `cfg(feature = "live_api_tests")`) and offline fixtures for deterministic tests.
3. **Processor + Config Integration**
   - Refactor `EmbeddingProcessor` to store `Arc<dyn EmbeddingService>` plus metadata so we no longer pattern-match on `EmbeddingSource` (`crates/ingest/ploke-embed/src/indexer/mod.rs:32-118`).
   - Extend `UserConfig::embedding` with router/provider selections and add `/embedding status|use|model` commands so the runtime can rebuild processors without restarts (`crates/ploke-tui/src/user_config.rs:81-180`).
4. **Database + Schema Updates**
   - Introduce `EmbeddingShape`-aware schemas in `ploke-transform` and parameterize the HNSW builders to remove the hard-coded 384 dimension noted in the remote indexing report.
   - Persist embedding provenance records so mismatches are detected before writes (aligns with report recommendations).
5. **Testing + Evidence**
   - Create fixture-backed unit tests for each provider (`embedding/tests/…`) and integration tests that simulate rate limits.
   - Add `target/test-output/embedding/*.json` artifacts capturing pass/fail counts, live-gate coverage, and observed dimensions for every CI run.

## Validation Strategy
- **Offline** – Run new unit + integration tests (mocked HTTP) on CI; attach summaries (pass/fail counts, feature coverage) to `target/test-output/embedding/offline_summary.json`.
- **Live (gated)** – Behind `--features live_api_tests`, execute smoke tests against HF/OpenAI to ensure request/response structs stay in sync; only mark “green” when tool-call traces confirm real embeddings flowed through the trait system, per AGENTS.md live-gate guidance.
- **Documentation** – Update `crates/ploke-tui/docs/openrouter/request_structure.md` with an embedding section and add a new `docs/feature/embedding/` note that references this report so future agents can find the plan.

## Open Questions / Follow-Ups
1. **Router vs. direct provider naming** – Should we reuse the `Router` terminology even though HF/OpenAI are direct APIs? (Leaning yes for symmetry, but we can alias to `EmbeddingProvider` if the naming proves confusing.)
2. **Cozo integration** – The existing placeholder backend returns `NotImplemented`; do we flesh it out as a managed remote endpoint or remove it until a real API exists?
3. **Shared wire layer** – `llm::wire::WireRequest` is almost identical to what we need; evaluate whether to generalize it (e.g., `net::wire::WireRequest<T>`) to avoid duplication.
4. **Cost tracking** – Deep research highlighted major cost variability between OpenAI models; decide whether to compute per-batch cost estimates upfront (may require exposing pricing data in the registry).

This report should guide the upcoming remote embedding implementation so it aligns with the agentic system roadmap, keeps traits + typing consistent with the chat stack, and unblocks remote vector indexing end to end.
