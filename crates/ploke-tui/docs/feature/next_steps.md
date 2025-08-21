# Next Steps for Model/Provider System

Proposal A — Expand Supported Models Catalog
- Decision: Curated defaults vs. automatic population.
  - Curated defaults: Explicit, stable behavior, versionable; minimal boilerplate with a helper (already used by insert_openrouter).
  - Automatic population: Query OpenRouter and synthesize entries; pros: breadth; cons: noisy UX, less control over parameters.
- Design:
  - Keep curated defaults as the backbone; add a “discovered” pool populated from OpenRouter with metadata only.
  - Surface discovered models in `model list` under a separate section; allow “promoting” discoveries to user config with a command.
- Registry structure:
  - Split Provider (endpoint details, credentials) from Model (capabilities, defaults).
  - Introduce a `ModelCatalog` with normalized records (id, display_name, capabilities, pricing).
  - Providers then reference a `model_id` in the catalog and override llm_params.
- Boilerplate reduction:
  - Provide macros/builders to register model families. Avoid one struct-per-model explosion (e.g., KimiK2Model, Gpt4oModel, Gpt5Model, etc.).

Proposal B — Generalize for Arbitrary Providers and Model Definitions
- Define ProviderKind traits (format_request, parse_response, supports_tools, etc.).
- Create adapters: OpenRouterProvider, OpenAIProvider, AnthropicProvider, CustomProvider.
- Model records remain data (id, capabilities), decoupled from provider behaviors.
- Allow multiple providers serving the same model_id (routing/fallback potential).

Other Development Paths
1) Tooling and Safety
- Add schema validation for config.toml to catch drift early.
- Add a `dry-run` validator for model switches that checks keys, capabilities, and network reachability.

2) UX and Observability
- Persist and display last-refresh timestamp for capabilities.
- Add `/provider alias` management commands and `/model search <query>` across curated + discovered catalogs.
- Integrate cost estimates into chat sidebar using cached pricing.

Milestones
- M1: Introduce ModelCatalog abstraction and “discovered vs. curated” views.
- M2: Provider adapters (request formatting split) and simple routing hooks.
- M3: Alias/editor UX polish and end-to-end tests for switching + tool calls.
