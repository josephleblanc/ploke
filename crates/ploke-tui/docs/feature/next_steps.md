# Next Steps for Model/Provider System

## Current Proposals Review

### Proposal A — Expand Supported Models Catalog
- Decision: Curated defaults vs. automatic population.
  - Curated defaults: Explicit, stable behavior, versionable; minimal boilerplate with a helper (already used by insert_openrouter).
  - Automatic population: Query OpenRouter and synthesize entries; pros: breadth; cons: noisy UX, less control over parameters.
- Design:
  - Keep curated defaults as the backbone; add a "discovered" pool populated from OpenRouter with metadata only.
  - Surface discovered models in `model list` under a separate section; allow "promoting" discoveries to user config with a command.
- Registry structure:
  - Split Provider (endpoint details, credentials) from Model (capabilities, defaults).
  - Introduce a `ModelCatalog` with normalized records (id, display_name, capabilities, pricing).
  - Providers then reference a `model_id` in the catalog and override llm_params.
- Boilerplate reduction:
  - Provide macros/builders to register model families. Avoid one struct-per-model explosion (e.g., KimiK2Model, Gpt4oModel, Gpt5Model, etc.).

### Proposal B — Generalize for Arbitrary Providers and Model Definitions
- Define ProviderKind traits (format_request, parse_response, supports_tools, etc.).
- Create adapters: OpenRouterProvider, OpenAIProvider, AnthropicProvider, CustomProvider.
- Model records remain data (id, capabilities), decoupled from provider behaviors.
- Allow multiple providers serving the same model_id (routing/fallback potential).

## Additional Proposals

### Proposal C — Capability-Based Provider Routing
- Introduce a routing layer that selects providers based on required capabilities
- Define capability requirements per request (tools, streaming, context length, cost limits)
- Implement automatic fallback when primary provider fails or lacks capabilities
- Add routing policies: cost-optimized, performance-optimized, reliability-optimized
- Enable hybrid routing for complex requests that might benefit from multiple providers

### Proposal D — Provider Health and Performance Monitoring
- Add health checks for all configured providers (connectivity, latency, error rates)
- Implement performance metrics tracking (response times, token throughput)
- Create a provider scoring system based on reliability and performance
- Add automatic degradation detection and alerting
- Implement circuit breaker pattern for failing providers

### Proposal E — Dynamic Model Configuration Management
- Add runtime model parameter tuning without config file changes
- Implement parameter inheritance system (global -> provider -> request level)
- Create parameter validation against model capabilities
- Add parameter presets for common use cases (creative, precise, fast, economical)
- Enable A/B testing of parameters for different tasks

### Proposal F — Multi-Provider Cost and Usage Tracking
- Implement detailed cost tracking per provider, model, and request
- Add usage quotas and budget alerts
- Create cost visualization and reporting capabilities
- Implement cost-based routing decisions
- Add integration with provider billing APIs for accurate tracking

## Comparison Criteria

1. **Implementation Complexity** - How difficult is the implementation and how much existing code needs to change?
2. **User Experience Impact** - How much does this improve the end-user experience?
3. **Maintainability** - How easy will this be to maintain and extend in the future?
4. **Performance Impact** - What are the performance implications (latency, memory, CPU)?
5. **Flexibility** - How well does this support future requirements and extensions?
6. **Risk Level** - What is the risk of introducing bugs or breaking existing functionality?
7. **Value Proposition** - What unique value does this provide compared to other proposals?

## Detailed Comparison

| Proposal | Complexity | UX Impact | Maintainability | Performance | Flexibility | Risk | Value |
|----------|------------|-----------|-----------------|-------------|-------------|------|-------|
| A - Expand Catalog | Medium | High | High | Low | Medium | Low | High |
| B - Generalize Providers | High | High | High | Low | High | Medium | Very High |
| C - Capability Routing | Medium-High | High | High | Low-Medium | High | Medium | High |
| D - Health Monitoring | Medium | Medium | High | Low | High | Low | Medium-High |
| E - Dynamic Config | Medium | Medium-High | Medium | Low | High | Low | Medium |
| F - Cost Tracking | Medium | Medium | High | Low | Medium | Low | Medium |

## Final Recommendation

Based on the analysis, I recommend a phased approach:

**Phase 1 (Immediate - 2-4 weeks):**
- Implement Proposal A (Expand Catalog) to improve model discoverability
- Implement Proposal D (Health Monitoring) for better provider reliability

**Phase 2 (Medium-term - 1-2 months):**
- Implement Proposal B (Generalize Providers) for architectural flexibility
- Implement Proposal C (Capability Routing) for intelligent provider selection

**Phase 3 (Long-term - 2-3 months):**
- Implement Proposal E (Dynamic Config) for advanced parameter management
- Implement Proposal F (Cost Tracking) for financial awareness

This approach balances immediate user value with long-term architectural improvements, minimizing risk while maximizing flexibility for future enhancements.

## Rationale

1. **Proposal B is the highest priority** for architectural reasons - it creates a solid foundation for all other enhancements
2. **Proposal A provides immediate user value** with relatively low risk
3. **Proposals D and C complement each other** - monitoring enables intelligent routing
4. **Proposals E and F are valuable but lower priority** - they enhance rather than enable core functionality

The recommended sequence ensures we build a robust, flexible system while continuously delivering user value at each step.

Other Development Paths
1) Tooling and Safety
- Add schema validation for config.toml to catch drift early.
- Add a `dry-run` validator for model switches that checks keys, capabilities, and network reachability.

2) UX and Observability
- Persist and display last-refresh timestamp for capabilities.
- Add `/provider alias` management commands and `/model search <query>` across curated + discovered catalogs.
- Integrate cost estimates into chat sidebar using cached pricing.

Milestones
- M1: Introduce ModelCatalog abstraction and "discovered vs. curated" views.
- M2: Provider adapters (request formatting split) and simple routing hooks.
- M3: Alias/editor UX polish and end-to-end tests for switching + tool calls.
