
# 1) Mental model (the three “M/E/P” layers)

* **Model (M)** = the canonical thing users think of (e.g. `openai/gpt-4o`, `moonshotai/kimi-k2`).
  Facts here come from `/models` and are mostly *capability hints* (context\_length, supported\_parameters, description).

* **Endpoint (E)** = a concrete serving endpoint for that model (provider-backed), found via `/models/:author/:slug/endpoints`.
  This is where **truth** lives for: actual `supported_parameters`, pricing, limits (max\_prompt/completion), quantization, uptime, etc.

* **Provider (P)** = the operator of an endpoint (DeepInfra, Fireworks, Z.AI, …).
  A provider may serve many endpoints across many models; you mostly use its **name/slug** for user preferences and routing hints.

### Key relationships

* 1 Model → N Endpoints (each with a Provider).
* Routing happens at the Model level but is **constrained/filtered** by Endpoint facts + User preferences on Providers/Params.

# 2) Normalized internal schema

You already noticed OpenRouter docs don’t perfectly match payloads. Fix that by **normalizing** OpenRouter’s responses into a small set of internal, versioned structs. Treat OpenRouter as *one importer* that fills your canonical store.

```rust
// ========= Canonical keys =========
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelKey {
    pub author: String,   // "openai"
    pub slug: String,     // "gpt-4o"
}
impl ModelKey {
    pub fn from_id(id: &str) -> Option<Self> {
        let (a, s) = id.split_once('/')?;
        Some(Self { author: a.to_string(), slug: s.to_string() })
    }
    pub fn id(&self) -> String { format!("{}/{}", self.author, self.slug) }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderKey(pub String);  // "deepinfra", "z-ai", …

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointKey {
    pub model: ModelKey,
    pub provider: ProviderKey,       // from endpoint `tag` (slug)
    pub quant: Option<Quant>,        // fp4, bf16, …
}

// ========= Canonical facts =========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub key: ModelKey,
    pub name: String,
    pub created: i64,                        // unix ts from /models
    pub description: String,
    pub architecture: Architecture,          // your existing type
    pub context_hint: Option<u32>,           // model-level context_length
    pub supported_params_hint: Vec<SupportedParameters>, // model-level hints
    // derived/cache helpers
    pub last_refreshed_s: i64,               // monotonic cache timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub key: EndpointKey,
    pub model_name: String,                  // human name
    pub provider_display: String,            // "DeepInfra", "Z.AI", …
    pub pricing: ModelPricing,               // normalized to $/token
    pub supported_parameters: Vec<SupportedParameters>,
    pub context_length: u32,                 // authoritative
    pub max_completion_tokens: Option<u32>,
    pub max_prompt_tokens: Option<u32>,
    pub uptime_30m: Option<f32>,
    pub supports_implicit_caching: Option<bool>,
    pub status: Option<i32>,
    pub last_refreshed_s: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderRecord {
    pub key: ProviderKey,
    pub display_name: String,                // "DeepInfra"
    pub notes: Option<String>,
}
```

**Why normalize?**

* Your UI and routing logic operate on a **stable** in-app shape.
* Importers (OpenRouter today, OpenAI/Anthropic/Groq tomorrow) map into the same structs.
* You can unit-test importers with JSON fixtures without touching UI/business logic.

# 3) User preferences & profiles (overlay layer)

Split **facts** (above) from **preferences** (below). Preferences are persisted in your `UserConfig` (TOML) and reference the canonical keys.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<f32>,
    pub max_tokens: Option<u32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub min_p: Option<f32>,
    pub top_a: Option<f32>,
    pub seed: Option<i64>,
    pub response_format: Option<JsonObjMarker>,
    pub tools_required: bool,         // convenience: must have Tools in supported_parameters
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelProfile {            // user-named param sets per model
    pub name: String,               // e.g. "creative-0.8" or "eval-sweep"
    pub params: LLMParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelPrefs {
    pub model: ModelKey,                            // canonical id
    pub default_profile: Option<String>,            // name from profiles
    pub profiles: Vec<ModelProfile>,                // per-model param sets
    pub allowed_providers: Option<Vec<ProviderKey>>,// allowlist (None=all)
    pub banned_providers: Option<Vec<ProviderKey>>,// denylist
    pub required_supported_params: Vec<SupportedParameters>, // must-have
    pub selected_endpoints: Vec<EndpointKey>,       // for explicit routing
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryPrefs {
    pub global_default_profile: Option<String>,
    pub models: Vec<ModelPrefs>,
    pub strictness: ModelRegistryStrictness,        // you already have this
}
```

**Lookups you’ll do a lot**:

* Find `ModelPrefs` by `ModelKey`.
* Resolve “effective profile” for a model = (model.default\_profile) → profile.params, else (global\_default\_profile), else fallback.

# 4) Cache + refresh strategy (pragmatic & fast)

* Store **models list** (from `/models`) with a **short TTL** (e.g. 6–12 hours) — it changes but not per minute.
* Fetch **endpoints for a model lazily** when a user expands that model in the picker, with a **short TTL** (e.g. 30–60 minutes). Cache by `EndpointKey`.
* Keep a **schema\_version** in your cache. If you change internal structs, bump it and invalidate gracefully.
* Always tolerate missing/nullable fields (`serde(default)`) and cast numerics to your chosen types.

A simple trait set keeps this tidy:

```rust
#[async_trait::async_trait]
pub trait CatalogImporter {
    async fn refresh_models(&self) -> color_eyre::Result<Vec<ModelRecord>>;
    async fn refresh_endpoints(&self, model: &ModelKey) -> color_eyre::Result<Vec<EndpointRecord>>;
}

pub trait CatalogStore {
    fn put_models(&self, models: &[ModelRecord]) -> color_eyre::Result<()>;
    fn get_models(&self) -> color_eyre::Result<Vec<ModelRecord>>;

    fn put_endpoints(&self, eps: &[EndpointRecord]) -> color_eyre::Result<()>;
    fn get_endpoints(&self, model: &ModelKey) -> color_eyre::Result<Vec<EndpointRecord>>;

    fn now_s(&self) -> i64; // inject clock for testability
}
```

Your existing `refresh_from_openrouter` becomes an **importer** that writes into a **store** (Cozo/sled/sqlite/in-mem) instead of directly into `registry.capabilities`.

# 5) Routing algorithm (building `CompReq` correctly)

Given (ModelKey, optional Selected Endpoints, Preferences):

1. **Collect eligible endpoints**

   * Start from cached endpoints for the model (fetch if TTL expired).
   * Filter by:
   * allowlist / denylist providers
   * `required_supported_params` (+ `LLMParameters.tools_required`)
   * optional quantization constraints (future).

2. **If the user explicitly selected endpoints** for this model, intersect with the above; if the intersection is empty, explain why in the UI.

3. **Construct the request**
   Two workable strategies with OpenRouter:

   * **Primary**: set `model: Some(model_key.id())`, and if you want explicit routing among providers, set `models: Some(vec![model_key.id()])` plus `provider: Some(ProviderPreferences{ allow: vec![…] })`.
   * **Alternate**: omit `model` and only set `models: [same canonical id]` with provider allowlist.
     Keep this logic in one function so you can swap strategies if OpenRouter behavior changes.

```rust
pub struct RoutePlan {
    pub model: ModelKey,
    pub endpoints: Vec<EndpointRecord>, // filtered & eligible
    pub profile: LLMParameters,         // resolved profile
}

impl RoutePlan {
    pub fn to_comp_req<'a>(&self, msg: Option<Vec<crate::llm::RequestMessage>>, prompt: Option<String>) -> CompReq<'a> {
        let provider_allow: Vec<String> = self.endpoints
            .iter()
            .map(|e| e.key.provider.0.clone())
            .collect();

        CompReq {
            messages: msg,
            prompt,
            model: Some(self.model.id().as_str()),
            models: Some(vec![self.model.id()]),
            provider: Some(ProviderPreferences::allow(provider_allow)), // your type
            temperature: self.profile.temperature,
            top_p: self.profile.top_p,
            top_k: self.profile.top_k,
            max_tokens: self.profile.max_tokens,
            frequency_penalty: self.profile.frequency_penalty,
            presence_penalty: self.profile.presence_penalty,
            repetition_penalty: self.profile.repetition_penalty,
            min_p: self.profile.min_p,
            top_a: self.profile.top_a,
            seed: self.profile.seed,
            response_format: self.profile.response_format.clone(),
            tools: None,
            tool_choice: None,
            ..Default::default()
        }
    }
}
```

> Note: your “No model available” UX hint still stands. If `eligible_endpoints.is_empty()`, show a targeted message: *“No endpoints meet your constraints. Loosen `require tools` or allow more providers.”*

# 6) Picker UX flow (fast & intuitive)

**Search list (Models):**

* Backing data: `Vec<ModelRecord>` indexed by:

  * id, name, canonical\_slug (author/slug), and (optionally) trigram on description.
* UI shows: name, author/slug, context hint, (tool support hint if available).
* Keybindings:

  * `/` begin search; `Enter` to commit.
  * `l` expand → triggers endpoint fetch if stale.

**Expanded list (Endpoints for selected Model):**

* Rows: `provider_display | quant | price (in/out per 1K) | context | supports tools ✓/× | uptime`.
* Keybindings:

  * `Space` toggle select endpoint.
  * `I` (inspect) shows full details (limits, supported params, etc.).
  * `p` choose profile (or `P` to manage profiles).
  * `r` run with selection (opens “parameter sweep” if enabled).

**Profiles & parameters:**

* `p` on model → pick default profile or ad-hoc edit.
* `S` “sweep” → opens a small form:

  * pick parameter (temperature/top\_p/etc.), range `[start, stop]`, steps `n`, (optionally) seeds `m`.

# 7) Parameter sweeps (safe, traceable fan-out)

Represent a sweep as a plan with generated **variants** and stable **correlation ids** for results:

```rust
#[derive(Debug, Clone)]
pub struct ParamVariant {
    pub label: String,                // e.g. "temp=0.2"
    pub params: LLMParameters,
}

#[derive(Debug, Clone)]
pub struct BatchRequestPlan {
    pub route: RoutePlan,
    pub variants: Vec<ParamVariant>,  // orthogonal to endpoints
    pub concurrency: usize,           // backpressure limit
}

impl BatchRequestPlan {
    pub fn grid_temperature(mut self, start: f32, stop: f32, steps: usize) -> Self {
        let step = if steps > 1 { (stop - start) / (steps as f32 - 1.0) } else { 0.0 };
        self.variants = (0..steps).map(|i| {
            let t = start + step * (i as f32);
            let mut p = self.route.profile.clone();
            p.temperature = Some(t);
            ParamVariant { label: format!("temp={:.2}", t), params: p }
        }).collect();
        self
    }
}
```

Execution strategy:

* For each `ParamVariant` × each selected endpoint (or just let OR route across allowed providers), issue a request with a **request\_id** you generate (include model, provider slug, variant label).
* Collect: latency, token counts, cost estimate (`price * tokens`), and a short automatic summary (first N chars or structured eval if `response_format=json_object`).
* UI shows a sortable table; allow saving the best as a new **profile**.

# 8) Persistence layout (TOML + cache store)

**TOML (`~/.config/ploke/config.toml`)**

* Keep your existing `UserConfig`, but replace the model bits with `RegistryPrefs` above.
* Add a `version = 1` for future migrations.

**Cache store**

* Use Cozo/sqlite/sled — whichever you already ship. Tables/collections:

  * `models { key_id TEXT PRIMARY KEY, record JSON, last_refreshed_s INT, schema_version INT }`
  * `endpoints { key_id TEXT PRIMARY KEY, model_id TEXT, record JSON, last_refreshed_s INT, schema_version INT }`
  * Secondary index by `model_id` for fast endpoint lookups.

# 9) Robustness & tests

* **Fixtures**: save raw `/models` & `/endpoints` JSON snapshots into `tests/fixtures/...`.
* **Importer tests**: deserialize → normalize → assert invariants (e.g., `$ / token ≥ 0`, `context_length > 0`, `supports_tools` computed correctly).
* **Property tests** (proptest/quickcheck): for numeric ranges (temperature/top\_p grids), ensure count and monotonicity.
* **Serde leniency**: `#[serde(default)]` on every optional field; prefer `Option<T>` and post-normalize to sane defaults.

# 10) Putting it together (thin façade API)

Expose a small, ergonomic API that the TUI and the request layer both use:

```rust
pub struct RegistryService<I: CatalogImporter, S: CatalogStore> {
    importer: I,
    store: S,
    prefs: RegistryPrefs, // load/save from TOML
}

impl<I: CatalogImporter, S: CatalogStore> RegistryService<I, S> {
    pub async fn search_models(&self, q: &str) -> color_eyre::Result<Vec<ModelRecord>> { /* filter */ }

    pub async fn ensure_model_endpoints(&self, key: &ModelKey) -> color_eyre::Result<Vec<EndpointRecord>> {
        // TTL check, fetch via importer if stale, write to store, return from store
    }

    pub fn route_plan(&self, model: &ModelKey) -> color_eyre::Result<RoutePlan> {
        // resolve prefs (profile + provider filters), resolve eligible endpoints, build plan
    }

    pub fn set_default_profile(&mut self, model: &ModelKey, profile: &str) { /* update prefs + save */ }

    pub fn save_profile(&mut self, model: &ModelKey, profile: ModelProfile) { /* upsert */ }

    pub fn select_endpoints(&mut self, model: &ModelKey, eps: Vec<EndpointKey>) { /* update prefs */ }
}
```

# 11) Subtle but important choices

* **Truth source**: treat `/endpoints` as “ground truth” for capabilities. Use `/models` only for discovery + hints.
* **Supports tools**: compute from endpoint `supported_parameters` only; if missing, fall back to model-hint (but mark as “uncertain” in UI).
* **Price display**: always precompute **per-1K tokens** and **per-1M tokens** for the UI; store both to avoid recomputing.
* **Units**: normalize all token limits/prices to *integers* where sensible (`u32` for tokens) to prevent float surprises in comparisons.
* **Extensibility**: when you add Anthropic/OpenAI/Groq direct providers, implement a new `CatalogImporter` that maps their catalog → the same `ModelRecord`/`EndpointRecord`. Your picker doesn’t change.

---

if you want, I can sketch the OpenRouter importer that populates these canonical records (mapping from your existing `ModelsEndpoint`/`Endpoint` types), plus a tiny in-mem store so you can integrate the picker right away.
