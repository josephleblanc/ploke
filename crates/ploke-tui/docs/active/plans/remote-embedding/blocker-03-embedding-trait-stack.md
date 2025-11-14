# Blocker 03 – Embedding Router Trait Stack & Registry

_Last updated: 2025-11-14_

## Problem statement
- `EmbeddingProcessor` holds an enum of concrete backends (`EmbeddingSource`) and the provider crates (`ploke-embed/src/providers/*.rs`) hand-roll HTTP payloads with ad-hoc JSON.
- There is no shared registry of providers/models for embeddings, so `/embedding list` cannot surface model metadata, and we cannot enforce strong typing or reuse OpenRouter-like tooling.
- Earlier research (remote embedding trait report) identified the need for router-style traits mirroring the LLM stack, but open questions remained about naming, shared wire plumbing, and exposing pricing metadata.

## Goals
1. Mirror the LLM router pattern with embedding-specific traits so new providers are “implement trait + register.”
2. Enforce strongly typed request/response structs (no `serde_json::Value`), including serialization for tool schemas per AGENTS.md.
3. Centralize provider/model metadata (dimensions, context length, rate limits, cost) in a registry that the TUI + embedder pipeline share.
4. Share as much infrastructure (`WireRequest`, `IoManager` telemetry hooks, builder macros) with the LLM stack as practical.
5. Provide upgrade paths for `ploke-embed` so it depends on trait objects rather than concrete backends.

## Module layout
```
crates/ploke-tui/src/embedding/
├── mod.rs                  // re-exports, EmbeddingManager hooks (Blocker 04)
├── router.rs               // EmbeddingRouter trait + provider declares
├── request.rs              // EmbeddingRequest<R>, builder, serialization
├── wire.rs                 // generic WireRequest<T>, HTTP envelope shared with LLM stack
├── registry/
│   ├── mod.rs              // EmbeddingRegistry facade
│   ├── catalog.rs          // provider/model metadata persistence
│   ├── pricing.rs          // integrates all_pricing_parsed.json slices
│   └── user_prefs.rs       // mirrors llm::registry::user_prefs for defaults
├── service.rs              // EmbeddingService trait + dyn dispatch for ploke-embed
├── types.rs                // EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape, metrics
└── tests/                  // unit tests w/ mock routers
```

`ploke-embed` consumes the `EmbeddingService` trait from this module via a new crate feature (or `ploke-core` shared crate if we want to avoid a TUI dependency).

## Core traits & structs
### `EmbeddingRouter`
```rust
pub trait EmbeddingRouter:
    Copy + Clone + Serialize + DeserializeOwned + Default + Eq
{
    type RouterModelId: RouterModelId;
    type RequestFields: ApiRoute + Serialize + Default; // router-specific fields
    type ResponseBody: for<'a> Deserialize<'a> + EmbeddingVectorPayload;

    const BASE_URL: &'static str;
    const EMBEDDING_URL: &'static str;
    const MODELS_URL: &'static str;
    const API_KEY_ENV: &'static str;

    fn resolve_api_key() -> Result<String, VarError> { ... }
}
```
- `RouterModelId` replicates `llm::RouterModelId` semantics (helpers for slug formatting & alias conversion).
- `EmbeddingVectorPayload` trait exposes `fn embeddings(&self) -> &[Vec<f32>]` so consumers stay agnostic of provider-specific JSON shapes.

### `EmbeddingRequest<R: EmbeddingRouter>`
```rust
pub struct EmbeddingRequest<R: EmbeddingRouter> {
    pub router: R,
    pub model: R::RouterModelId,
    pub inputs: Vec<EmbeddingSnippet>,
    pub parameters: EmbeddingParameters,
    pub router_fields: R::RequestFields,
}
```

Builder ensures batch size + encoding_format are validated before serialization. `.into_wire()` returns `EmbeddingWireRequest` with URL, headers, JSON bytes, and telemetry context.

### `EmbeddingWireRequest`
- Refactors `llm::wire::WireRequest` into `net::wire::WireRequest<Purpose>` in `crates/ploke-tui/src/net/wire.rs`. Both chat + embedding use the shared struct, but we supply a `WirePurpose` enum to differentiate for logging/telemetry.
- Holds `expected_status: RangeInclusive<StatusCode>`, `redacted_body`, `evidence_hook: EvidenceEmitter` so AGENTS.md requirements on telemetry are easier to satisfy.

### `EmbeddingService`
Located in `service.rs`, consumed by `ploke-embed`:
```rust
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    fn provider(&self) -> EmbeddingProviderSlug;
    fn model_id(&self) -> EmbeddingModelId;
    fn shape(&self) -> EmbeddingShape;
    fn batch_limits(&self) -> BatchLimits;
    async fn embed(&self, inputs: Vec<EmbeddingSnippet>) -> Result<Vec<Vec<f32>>, EmbedError>;
}
```

Implementations wrap a router + reqwest client configured with retry policies. They also emit telemetry via the shared `EmbeddingWireRequest` instrumentation.

## Registry architecture
- **Catalog data** combines three sources:
  1. Provider-supplied `/v1/models` (for OpenAI, HF Inference, etc.), cached under `~/.cache/ploke/embedding_models/<provider>.json` with IoManager-hash verification.
  2. Static overrides from `crates/ploke-tui/data/models/all_pricing_parsed.json` (reused from LLM stack) to annotate cost per 1K tokens/vectors.
  3. Local workspace overrides (user-provided `embedding_registry.toml`) for self-hosted endpoints.
- `EmbeddingRegistry` exposes APIs:
  - `fn providers(&self) -> &[EmbeddingProviderRecord]`
  - `fn models(&self, provider: EmbeddingProviderSlug) -> &[EmbeddingModelRecord]`
  - `fn resolve(&self, alias: &str) -> Option<EmbeddingModelRecord>`
  - `fn best_batch_for(&self, provider, dimension) -> u32`
- `ploke-tui` loads registry at startup via `RegistryLoader`, similar to LLM `RegistryPrefs`. Embedding-specific preferences (favorites, pinned providers) live beside the LLM config (`UserConfig::embedding_preferences`).
- CLI overlay uses the registry to show available models and to validate `/embedding rebuild --provider ... --model ...` arguments.

## Integrating with ploke-embed
- Replace `EmbeddingProcessor::source: EmbeddingSource` with `Arc<dyn EmbeddingService>` plus metadata struct `ActiveEmbeddingContext { set_id, shape, provider_slug }`.
- Provider crates implement `EmbeddingService` by using the router stack. Example: `struct OpenAIService { router: routers::openai::OpenAI, http: reqwest::Client, params: EmbeddingParameters }`.
- Local embeddings continue implementing the trait but skip HTTP (they can return `EmbeddingWireRequest::Offline`).

## Pricing + telemetry hooks
- Registry merges pricing data → `EmbeddingModelRecord.cost_per_1k_tokens` or per vector, enabling `/embedding status` to surface estimated costs and warn when a provider is missing numbers.
- `EmbeddingWireRequest` captures request/response metadata and writes compressed JSON to `target/test-output/embedding/wire_logs/<timestamp>.json`. The schema includes provider slug, model id, batch size, latency, HTTP status, and whether tool calls were invoked (for live gate compliance).

## Tests & validation
1. **Unit tests** under `embedding/tests/router_tests.rs` verifying serialization for HuggingFace + OpenAI requests, including options like `wait_for_model` and `encoding_format`.
2. **Registry tests** that load canned provider catalogs and ensure dimension/type metadata is parsed correctly.
3. **Integration tests** in `ploke-embed` that instantiate `Arc<dyn EmbeddingService>` via the registry and feed them into `EmbeddingProcessor`, ensuring we can swap providers without rebuilding binaries.
4. **Live gated tests** (cfg `live_api_tests`) hitting HF + OpenAI endpoints, recording telemetry artifacts and verifying `EmbeddingWireRequest` redacts secrets.

## Decisions captured
- **Naming**: Keep `Router` terminology (`EmbeddingRouter`) for parity with LLM stack; alias `EmbeddingProvider` in user-facing docs for clarity.
- **Wire layer**: Extract existing `llm::wire::WireRequest` into shared `net::wire::WireRequest` with typed purpose. Both chat + embedding share it to avoid drift.
- **Pricing metadata**: Registry integrates `all_pricing_parsed.json` from day one; no more ad-hoc lookups in CLI.

## Follow-ups / dependencies
- Blocker 04 (runtime reconfiguration) will use `EmbeddingServiceFactory` derived from this stack to rebuild embedders when `/embedding use` changes sets.
- Need to confirm where to host provider-specific auth notes (docs vs. registry). Suggest storing env var hints in `EmbeddingProviderRecord` so overlays can say “set OPENAI_API_KEY.”

With this blueprint we can implement the trait stack without revisiting design questions, unblocking provider expansion and reuse of typed tooling.
