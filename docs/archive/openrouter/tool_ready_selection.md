# OpenRouter Tool-Ready Selection – Context and API Quick Reference

Goal
- Enable fast, non-blocking model selection with provider-level tool support information.
- User chooses a model and a concrete provider for that model that supports tools.

Key Concepts
- Model: "author/slug" (e.g., openai/gpt-4o). Aggregates multiple providers.
- Provider: Upstream service actually serving the model (e.g., openai). OpenRouter mediates requests.
- Tools support is provider-specific. A model may have some providers with tools and others without.

Catalog Endpoints
- Default listing: GET {base_url}/models/user
  - Filtered list based on the user’s provider preferences.
  - Fields we consume:
    - model.id, model.name
    - model.supported_parameters: ["tools", ...] (not authoritative for provider)
    - model.top_provider.{context_length, max_completion_tokens}
    - model.pricing.{prompt, completion} (strings) → normalize to f64
    - model.providers[]:
      - id (provider slug)
      - supported_parameters: ["tools", ...]
      - capabilities.tools (fallback)
      - context_length, pricing
- Per-model endpoints (authoritative per-provider details):
  - GET {base_url}/models/:author/:slug/endpoints
  - Each endpoint contains provider-specific supported_parameters, context/pricing, status.

Determining tool support
- Provider-level (preferred):
  - provider.supported_parameters contains "tools" → supports_tools = true
  - Fallback: provider.capabilities.tools == true
- Model-level:
  - Do not conflate into a single boolean for UX decisions; present provider-level truth in UI.
  - Optionally show model.supported_parameters as a hint.

Requesting tool calls (OpenAI-style body)
- Required fields:
  - model: "author/slug"
  - messages: [...]
  - tools: [ { type: "function", function: { name, description, parameters } } ]
- Provider routing preference:
  - Include a provider selection/preferences object in the request body so OpenRouter targets the chosen provider.
  - Schema is under development; wire once the request builder code is provided.

Async UX Strategy
- On “model search <kw>”:
  1) Immediately open overlay with keyword and empty results (snappy feel).
  2) Spawn async fetch of /models/user; filter by keyword.
  3) After overlay opens, pre-cache per-model endpoints for visible results.
  4) Populate provider rows with supports_tools per provider.
- Selection: user picks one provider for the chosen model; persist to RuntimeConfig.

Error handling
- If a chosen provider lacks tools at call time, show a clear error and suggest known tool-capable providers for that model.

Open Questions (pending code)
- Provider routing schema in request body (allow/deny/order?). Provide the request builder code to finalize.
- Event used to deliver async results to the UI (AppEvent/SystemEvent). Provide events code.

References
- Pricing normalization: map prompt/completion strings to f64.
- Context length fallback: model.context_length || top_provider.context_length.
