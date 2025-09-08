LLM Module Refactor: Router/Provider/Model Architecture (07-09-2025)

Purpose
- Define an extensible, strongly typed, and performant LLM stack that generalizes beyond OpenRouter while removing duplication across the current `src/llm` and new `src/llm2` modules.
- Align with engineering principles: strong typing, safety-first editing, evidence-based changes, static dispatch, macro-based extensibility, and compile-time guarantees where possible.

Scope
- Inventory and de-duplication plan for: request types, tool plumbing, endpoint/catalog types, and request building.
- Target architecture: Router → Provider → Model → Endpoint (canonical keys), a pluggable importer/store registry, and a unified wire layer that builds requests with static dispatch.
- Migration path and validation strategy (tests + live gates) to ensure regressions are caught with evidence.

Current State (Findings)

Code hotspots reviewed
- Old module: `src/llm`
  - Request/session orchestration: `src/llm/mod.rs`, `src/llm/session.rs`
  - OpenRouter shapes: `src/llm/openrouter/{openrouter_catalog.rs, provider_endpoints.rs, model_provider.rs, providers.rs}`
  - JSON analysis helpers: `src/llm/openrouter/json_visitor.rs`
  - Defaults: `src/llm/registry.rs`
- New module: `src/llm2`
  - Typed keys and transport: `src/llm2/newtypes.rs`
  - Chat messages and high-level params: `src/llm2/chat_msg.rs`
  - Request types (CompReq) and markers: `src/llm2/request/{mod.rs, completion.rs, marker.rs, endpoint.rs}`
  - Wire builder: `src/llm2/wire.rs`

Primary duplication and drift
- Message and roles
  - `llm::RequestMessage` + `llm::Role` vs `llm2::chat_msg::RequestMessage` + `Role` are functionally the same.
- Provider preferences
  - `llm::ProviderPreferences` vs `llm2::ProviderPreferences` differ only in Option vs empty-vec defaults and visibility.
- Completion request
  - `llm/openrouter/model_provider::CompReq` vs `llm2/request::CompReq` essentially mirror each other.
- Tool choice + markers
  - Duplicated in old (`model_provider.rs`) and new (`llm2/request/endpoint.rs`).
- Endpoint/catalog shapes
  - `openrouter_catalog::{fetch_models, fetch_model_endpoints}` + `provider_endpoints.rs` types overlap with `llm2/request/endpoint.rs` equivalents.
- Request building
  - `session::build_comp_req` builds payloads directly, while `llm2/wire.rs::build_openrouter_request` provides a cleaner, typed builder around the same idea.

Design Goals
- Single source of truth for:
  - Canonical keys: `ModelKey`, `ProviderKey`, `EndpointKey` with typed wrappers (Author, Slug, ProviderSlug, Quant).
  - Request DTOs: CompReq, ToolChoice, JsonObjMarker, ProviderPreferences.
  - Messages: RequestMessage, Role.
  - Endpoint/catalog shapes used for normalization.
- Router → Provider → Model → Endpoint model
  - Router = transport (OpenRouter, DirectOAI, …), Provider = operator slug/name, Model = author/slug, Endpoint = concrete serving option (quant, limits, price).
- Importers + Store
  - Treat external APIs as importers that normalize into canonical records. UI/business logic uses the canonical store only.
- Static dispatch
  - Prefer enums + generics over trait objects. Macros to reduce boilerplate where static dispatch causes repetition.
- Strong typing at boundaries
  - serde numeric unions (string-or-number) resolved into numeric fields; enums for variants; early validation of required invariants.
- Safety-first file IO
  - Writes (logs, cache) go through IoManager with staged-hash verification and atomic apply.

Target Architecture

1) Canonical Domain Types (keep in `llm2/newtypes.rs`)
- Keys and wrappers (already present):
  - `ModelKey { author: Author, slug: Slug }` → `.id()` returns `author/slug`.
  - `ProviderKey { slug: ProviderSlug }` and `EndpointKey { model: ModelKey, provider: ProviderKey, quant: Option<Quant> }`.
  - `ProviderConfig { key, name, transport }` with `Transport::{OpenRouter{base, allow, api_key_env}, DirectOAI{…}}`.
  - Keep `ArcStr`-backed wrappers to minimize cloning costs while remaining owned and Send/Sync.

2) Canonical Records (normalized facts)
- ModelRecord (facts from `/models`): name, created, description, architecture, context hint, supported params hint, last_refreshed.
- EndpointRecord (facts from `/models/:author/:slug/endpoints`): provider_display, pricing, supported_parameters, authoritative limits, uptime, status, last_refreshed.
- ProviderRecord: provider identity metadata. 
- Note: these structs can live in `llm2/domain.rs` (new) or near newtypes.rs. They represent the truth store shape, independent from any provider.

3) Importer + Store interfaces (static dispatch)
- Traits
  - `CatalogImporter` (OpenRouter today; OpenAI/Groq/Anthropic later):
    - `async fn refresh_models(&self) -> Result<Vec<ModelRecord>>`
    - `async fn refresh_endpoints(&self, model: &ModelKey) -> Result<Vec<EndpointRecord>>`
  - `CatalogStore` (sled/sqlite/cozo/in-mem):
    - read/write models and endpoints; inject `now_s()` for TTL logic.
- Implementation
  - OpenRouterImporter maps `ModelsEndpointsData`/`Endpoint` → canonical records.
  - Keep deserialization types for OpenRouter in an `llm2/importer/openrouter/*` module to decouple from store/domain.

4) RegistryService façade
- Holds importer + store + user prefs:
  - `RegistryPrefs` (TOML-backed): per-model profiles, provider allow/deny, selected endpoints, strictness, global default profile.
  - Methods: `search_models`, `ensure_model_endpoints(model)`, `route_plan(model)`, `set_default_profile`, `save_profile`, `select_endpoints`.
- RoutePlan → CompReq builder
  - Compute eligible endpoints by intersecting store facts with prefs (tools-required, allow/deny, quant, etc.).
  - Build request with `models: [model.id()]` and `provider: { allow: … }`; set `model: Some(model.id())` as the default strategy.

5) Unified Wire Layer
- Use `llm2/wire::WireRequest { url: Url, authorization_env: ApiKeyEnv, body: Value, content_type: &str }`.
- For OpenRouter: `build_openrouter_request(ModelKey, ProviderConfig, messages/prompt, LLMParameters)` returns a fully-formed request.
- For Direct providers (future): implement parallel builders. Keep static dispatch via `Transport` enum + match, or a generic `TransportAdapter<P>` implemented for each variant (no dyn).

6) Request/Message DTO consolidation
- Make `llm2` the single owner of:
  - `RequestMessage` + `Role` (move from `llm/mod.rs` → `llm2/chat_msg.rs`).
  - `ProviderPreferences` (adopt `llm2` version; provide From/Into for old type during migration).
  - `CompReq`, `ToolChoice`, `JsonObjMarker` (use `llm2/request/*`).
- Old modules re-export the new types to avoid broad ripples during migration.

7) Traits and Macros for Extensibility
- TransportAdapter (optional):
  - `trait TransportAdapter { fn build(&self, model: &ModelKey, prov: &ProviderConfig, msgs: Option<Vec<RequestMessage>>, prompt: Option<String>, params: &LLMParameters) -> Result<WireRequest>; }`
  - Provide impls per transport with static dispatch.
- provider_transport! macro:
  - Declaratively define a provider family with base URL, env key, and default allowlist.
  - Expands to `ProviderConfig` constructors and possibly registry defaults.
- serde helpers macro (optional):
  - Reduce boilerplate for numeric unions (string-or-number) using a local helper macro around `#[serde(deserialize_with = …)]`.

8) Performance & Compile-time Guarantees
- Static dispatch: prefer enums/generics over trait objects; use macros to avoid repetition.
- Minimize allocations:
  - Keep `ArcStr` in DTOs; keep copies owned only where persistence requires.
- GhostData/phantoms for policy states (optional advanced):
  - Example: `ToolPolicy<ToolsRequired>` phantom markers for compile-time gating of tool enforcement in builders; or typed states in `RequestSession`.
- GAT and zero-copy deserialization (targeted):
  - Consider borrowed-deserialize for importer fixtures to cut copies; normalize into owned canonical records before storing.

9) Safety-first Editing (IoManager)
- All file writes (logs, cache, golden files) go through an IoManager:
  - Stage writes with source hash → verify target hash → apply atomically.
  - Never write on hash mismatch; include error with actionable message.
- Persist test artifacts under `target/test-output/...` and reference them in summaries.

10) Testing & Live Gates
- Offline fixtures:
  - Save `/models` and `/endpoints` snapshots in `tests/fixtures/openrouter/…`.
  - Unit-test importer normalization: assert token units are non-negative, tool support recognized, and limits parsed correctly.
- Request snapshot tests:
  - For `WireRequest` JSON, validate payloads for common routes (with/without tools, provider allowlists).
- Live tests (behind `cfg(feature = "live_api_tests")`):
  - Do not count as pass if skipped; surface counts: pass/fail/ignored with notable failure summaries.
  - Emit to `target/test-output/openrouter_e2e/…` for evidence.

11) Migration Plan (Phased, Low-Risk)

Phase 0: Bridge (no behavior change)
- Re-export `llm2` DTOs in `llm`:
  - `pub use crate::llm2::{LLMParameters as L2Params, ProviderPreferences as L2ProvPrefs, …}`
  - Add `From` conversions where the old type shape differs (e.g., Option<Vec<T>> ↔ Vec<T>). 
- Update `session::build_comp_req` to call `llm2/wire::build_openrouter_request` internally; keep the old return type temporarily or convert.

Phase 1: Unify message/request types
- Move `RequestMessage`/`Role` ownership to `llm2/chat_msg.rs`; remove the duplicates from `llm/mod.rs` (keep type aliases there during transition).
- Remove `llm/openrouter/model_provider::CompReq` in favor of `llm2/request::CompReq`.

Phase 2: Endpoint/catalog normalization
- Consolidate endpoint shapes: choose `llm2/request/endpoint.rs::Endpoint` as the import shape.
- Introduce canonical `ModelRecord`/`EndpointRecord` in `llm2` and write an `OpenRouterImporter` that maps from the import shapes to canonical records.
- Keep `openrouter_catalog::fetch_*` as importer internals; stop exporting them once the RegistryService is live.

Phase 3: RegistryService + prefs
- Implement `RegistryService<I,S>` with `CatalogImporter` and `CatalogStore`.
- Adapt TUI flows:
  - Model picker uses `search_models` results (ModelRecord[]).
  - Expanding a model calls `ensure_model_endpoints(model)` and displays EndpointRecord rows.
  - Selection/profile changes update `RegistryPrefs` (persist via UserConfig module).

Phase 4: Session routing
- `process_llm_request` switches to `route_plan(model)` + `plan.to_comp_req` + `Transport` builder → `WireRequest` → HTTP.
- Tool enforcement uses canonical `supported_parameters` from EndpointRecord; only fall back to model hint when endpoint data is missing and clearly marked as “uncertain”.

Phase 5: Cleanup
- Remove duplicated types/functions in `src/llm` that are now provided by `llm2`.
- Update tests to target `llm2` shapes and RegistryService flows. Keep live tests feature-gated.

12) Concrete De-dup Map (What to move/use)
- Use from `llm2` (make authoritative):
  - `newtypes.rs`: keys, wrappers, Transport, ProviderConfig, Quant.
  - `chat_msg.rs`: RequestMessage, Role, LLMParameters, OaiChatReq.
  - `request/*`: CompReq, JsonObjMarker, ToolChoice.
  - `wire.rs`: build_openrouter_request (and future direct providers).
- Keep in `llm` for now, then remove:
  - `llm/openrouter/model_provider::CompReq` → replace with `llm2/request::CompReq`.
  - `llm::ProviderPreferences` → replace with `llm2::ProviderPreferences` (provide From/Into).
  - `llm::RequestMessage/Role` → replace with `llm2::chat_msg` versions (type alias during transition).
  - `session::build_comp_req` → use `llm2/wire` then migrate call sites.

13) Evidence and Readiness Gates
- For each phase, provide a short test summary with pass/fail/ignored counts and store artifacts under `target/test-output/...`:
  - Importer normalization tests: counts per field; tool parameter cardinality checks (see `json_visitor` precedents).
  - Request build snapshots for routes with and without tools.
  - Live endpoints sanity when feature is ON (assert key properties: tool_calls observed, endpoints non-empty, provider slugs parsed).
- If evidence is missing, report “readiness not established” explicitly.

14) Open Questions / Future Work
- Direct providers
  - Implement `build_direct_oai_request` (OpenAI/Anthropic/Groq) using their exact wire schema.
  - Add `Direct*Importer` variants to populate canonical records from those ecosystems.
- Zero-copy deserialization
  - Consider a borrowed-deserialize layer for importers with GAT-backed lifetimes for large JSON; normalize into owned records for the store.
- Tooling macros
  - Consolidate and generate common tool definitions via macros to reduce boilerplate, keeping static dispatch.
- Safety gate integration
  - Plumb IoManager into importer/store and test harness logs; add hash verification on all writes.

Appendix: Quick Code Sketches

RoutePlan → CompReq builder
```rust
pub struct RoutePlan {
    pub model: ModelKey,
    pub endpoints: Vec<EndpointRecord>,
    pub profile: LLMParameters,
}

impl RoutePlan {
    pub fn to_comp_req(
        &self,
        messages: Option<Vec<crate::llm2::chat_msg::RequestMessage>>,
        prompt: Option<String>,
    ) -> crate::llm2::request::CompReq {
        let allow = self
            .endpoints
            .iter()
            .map(|e| e.key.provider.slug.clone())
            .collect::<Vec<_>>();
        crate::llm2::request::CompReq {
            messages,
            prompt,
            model: Some(self.model.id()),
            models: Some(vec![self.model.id()]),
            provider: Some(crate::llm2::ProviderPreferences::allow(allow)),
            temperature: self.profile.temperature,
            top_p: self.profile.top_p,
            max_tokens: self.profile.max_tokens,
            ..Default::default()
        }
    }
}
```

Transport builder (static dispatch via enum)
```rust
pub fn build_wire_request(
    prov: &ProviderConfig,
    model: &ModelKey,
    msgs: Option<Vec<RequestMessage>>,
    prompt: Option<String>,
    params: &LLMParameters,
) -> color_eyre::Result<WireRequest> {
    match &prov.transport {
        Transport::OpenRouter { .. } => crate::llm2::wire::build_openrouter_request(model, prov, msgs, prompt, params)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e)),
        Transport::DirectOAI { .. } => {
            // TODO provider-specific schema
            color_eyre::bail!("DirectOAI transport not yet implemented")
        }
    }
}
```

RegistryService outline
```rust
pub struct RegistryService<I, S> {
    importer: I,
    store: S,
    prefs: RegistryPrefs,
}

impl<I: CatalogImporter, S: CatalogStore> RegistryService<I, S> {
    pub async fn ensure_model_endpoints(&self, k: &ModelKey) -> color_eyre::Result<Vec<EndpointRecord>> {
        // TTL check → importer → store → return
        // Prefer endpoints as source of truth for capability decisions
        unimplemented!()
    }

    pub fn route_plan(&self, k: &ModelKey) -> color_eyre::Result<RoutePlan> {
        // Resolve prefs + store facts → filter → plan
        unimplemented!()
    }
}
```

Summary
- We converge on `llm2` as the authoritative module for all DTOs and wire building, make OpenRouter “just an importer”, and structure a registry service with crisp separation of facts vs preferences. The migration replaces duplicated shapes with the new `llm2` types, shifts request building to the wire layer, and introduces a route planning step that generalizes to future providers without dynamic dispatch.

