# Expanded Proposals Review — Model/Provider System

Context
- This document expands upon crates/ploke-tui/docs/feature/next_steps.md, deepening proposals A–F and introducing additional proposals focused on provider handling.
- It provides an evaluation rubric (7 criteria), a comparative matrix, and a phased recommendation.

Evaluation Rubric (7 criteria)
1) Implementation Complexity — Relative effort and code churn required.
2) UX Impact — Direct value to end-users within the TUI.
3) Maintainability — Long-term clarity, isolation, and testability.
4) Performance — Latency, throughput, and resource impact.
5) Flexibility — Ability to extend to new models/providers and experimental features.
6) Operability — Observability, monitoring, debugging, and real-world resiliency.
7) Risk — Probability of breakage or regressions in core flows.

Expanded Proposals (A–F)

A — Expand Supported Models Catalog (Curated + Discovered)
- Deepening: Introduce a “discovered” pool populated from OpenRouter with metadata; curated defaults remain the backbone for stability. Add a promote-to-config command (e.g., /model promote <id>) to copy a discovered entry into user config with overridable llm_params.
- Pros: Immediate user value; bounded scope; low risk; improves discoverability; leverages existing openrouter_catalog client.
- Cons: Requires clear UX to avoid overwhelming users with noisy/low-quality models.
- Key tasks:
  - Add ModelCatalog abstraction (curated + discovered).
  - Extend model list rendering to show sections and highlight promotions.
  - Persist last-refresh timestamp and discovery count.

B — Generalize Providers (Adapters + Traits)
- Deepening: Define ProviderAdapter trait with request shaping, response parsing, tool support, and capability validation hooks. Implement OpenRouterAdapter first, then OpenAI/Anthropic; keep CustomAdapter for local/dev.
- Pros: Enables clean separation between transport/format and model records; unlocks routing/fallback; reduces coupling in user_config.
- Cons: Moderate-to-high initial effort; touches request lifecycle; needs strong tests.
- Key tasks:
  - Trait definition + adapters; façade API to llm_manager.
  - Conformance tests with Wiremock; golden-response fixtures.
  - Incremental migration behind a feature flag.

C — Capability-Based Provider Routing
- Deepening: Define RequiredCapabilities per request (tools, JSON mode, context length, max cost); implement route selection with pluggable policies (cost-optimized, reliability-optimized).
- Pros: Smarter provider selection and graceful fallback; improves success rates.
- Cons: Requires reliable and up-to-date capability/pricing cache; careful UX needed when routing differs from selected provider.
- Key tasks:
  - Capability normalizer; policy engine; routing audit logs to chat/debug pane.

D — Provider Health and Performance Monitoring
- Deepening: Health probes (startup and periodic), latency histograms, error rate dashboards, circuit breaker for recurring failures, provider score computation for routing.
- Pros: Operating confidence; easier triage; data to inform routing (C).
- Cons: Additional background tasks; metric storage/aggregation choices.
- Key tasks:
  - Simple ping endpoints or minimal chat probe; Prometheus-style metrics emit; circuit-breaker state machine.

E — Dynamic Model Configuration Management
- Deepening: Hierarchical parameter inheritance (global -> provider -> request); validation against capability cache; presets (creative/precise/economical).
- Pros: Strong UX; safer tweaks; reproducible settings via named presets.
- Cons: Validation can drift if cache is stale; needs good defaults.
- Key tasks:
  - Parameter schema; preset bundles; validation warnings inline in “model info”.

F — Multi-Provider Cost and Usage Tracking
- Deepening: Track estimated/requested usage by provider/model; surface cost forecasts; integrate with routing (C) and health (D).
- Pros: Budget-aware workflows; evidence-driven provider choices.
- Cons: Pricing varies and may be missing or conditional; estimation accuracy caveats.
- Key tasks:
  - Cost estimator module; per-request and rolling aggregates; chat-side summaries.

New Proposals (G–J)

G — Provider SDK and Plugin Interface
- Idea: Define a stable Provider SDK (traits + types) and optional plugin API (dyn-load feature gated) so 3rd parties can add providers without touching ploke-tui core.
- Pros: Scalability of integrations; isolates churn; invites community contributions.
- Cons: API stability constraints; unsafe plugin boundaries if dynamic loading is enabled; requires strong versioning policy.
- Key tasks:
  - Provider SDK crate with minimal dependencies; feature-gated dynamic loader; strict semver and integration tests.

H — Rate Limit, Retries, and Backpressure Coordinator
- Idea: Centralize rate-limit heuristics, exponential backoff, jitter, token buckets, and queue prioritization (e.g., tool calls prioritized over long generations).
- Pros: Prevents cascading failures; smoother UX; fewer hard errors; reduces provider bans.
- Cons: Complex to tune; needs provider-specific knobs and telemetry.
- Key tasks:
  - Coordinator service; policy configs per provider; queue diagnostics (/llm queue status) and per-request timeouts.

I — Secrets, Identity, and Environment Management
- Idea: Formalize API key handling: named credentials, vault integration (env or file), per-provider scoping, redaction by default; support profile switching (work/personal).
- Pros: Security best practices; fewer foot-guns; reproducible environments.
- Cons: Requires migration helpers; must ensure never to log secrets.
- Key tasks:
  - Credential store abstraction; profile switching commands; stricter serialization redaction and diff-friendly config.

J — Testing, Mocking, and Offline Replay for Providers
- Idea: Wiremock-based integration layer, response cassettes for deterministic offline replay; contract tests per adapter; failure-mode simulation (timeouts, 429, 5xx).
- Pros: Confidence to refactor layer (B, C, H); stable CI; facilitates community PRs.
- Cons: Requires disciplined fixture updates; cassette format governance.
- Key tasks:
  - Test harness with fixtures; replay mode toggle; CI pipeline; error-injection hooks.

Comparative Matrix (qualitative)
Legend: Low / Medium / High

| Proposal | Complexity | UX Impact | Maintainability | Performance | Flexibility | Operability | Risk |
|---------:|------------|-----------|-----------------|------------|-------------|-------------|------|
| A Catalog (Curated+Discovered) | Low | High | High | Low | Medium | Medium | Low |
| B Provider Generalization      | High | High | High | Low | High | Medium | Medium |
| C Capability Routing           | Med-High | High | High | Low-Med | High | Medium | Medium |
| D Health & Monitoring          | Medium | Medium | High | Low | High | High | Low-Med |
| E Dynamic Config               | Medium | Medium-High | Medium | Low | High | Medium | Low |
| F Cost & Usage                 | Medium | Medium | High | Low | Medium | Medium | Low |
| G Provider SDK/Plugin          | Med-High | Medium | High | Low | Very High | Medium | Medium |
| H Rate Limit & Backpressure    | Medium | Medium-High | High | High (smoother) | Medium | High | Medium |
| I Secrets & Profiles           | Low-Med | Medium | High | Low | Medium | High | Low |
| J Testing & Replay             | Medium | Medium | Very High | Low | Medium | Very High | Low |

Synthesis and Interdependencies
- B (Generalization) unlocks C (Routing), G (SDK), H (Coordinator), and simplifies D/E/F/J.
- D (Monitoring) enhances C (Routing) and H (Backpressure) decisions; both need good telemetry.
- A (Catalog) is the fastest user win and complements E (parameter presets) and F (cost display).
- I (Secrets) is low-risk, high-ROI hygiene that reduces incidents and improves portability.
- J (Testing) is a force multiplier: de-risks B/C/H and accelerates iteration.

Final Recommendation (Phased)

Phase 1 — Quick Wins (2–4 weeks)
- A Catalog: Curated + discovered models with promote-to-config; show discovery counts and last-refresh.
- I Secrets: Named profiles, stricter redaction-by-default, never log; ensure env precedence is documented; add /provider profile <name>.
- J Testing: Wiremock fixtures for OpenRouter; replay mode for CI; basic failure-mode tests.

Phase 2 — Architectural Foundation (4–8 weeks)
- B Provider Generalization: ProviderAdapter trait + OpenRouterAdapter; façade in llm_manager; migrate request lifecycle incrementally behind a feature flag.
- D Health & Monitoring: Health checks, latency histograms, circuit breaker; surface minimal status in “model info”.

Phase 3 — Intelligence and Control (6–10 weeks, can be parallelized)
- C Capability Routing: Add RequiredCapabilities, policy engine, and routing audit logs to chat; allow user override to pin provider per-request.
- H Rate Limit & Backpressure: Coordinator with token buckets/backoff; inspection command (/llm queue status); priority lanes (tool calls, short prompts).
- E Dynamic Config: Parameter hierarchy and presets with validation against cached capabilities.

Phase 4 — Ecosystem and Scale (ongoing)
- G Provider SDK/Plugin: Publish stable SDK crate; optional dynamic plugins behind feature gate; sample adapter repo to seed contributions.
- F Cost & Usage: Cost estimator with rolling totals; integrate into routing policies for budget-aware decisions.

Acceptance Criteria (high level)
- Users can safely discover and promote models; secrets are handled securely; provider health/latency are visible.
- Adapters enable adding a new provider without touching core TUI logic.
- Routing/backpressure reduce failure rates and smooth latency without surprising the user (audit logs available).
- CI runs offline with replay; adapters ship with contract tests; regressions are caught before release.

Risk Mitigation
- Feature-flag major refactors (B, C, H) and ship incrementally.
- Maintain a fallback “direct OpenRouter path” until adapters stabilize.
- Strict logging discipline (no secrets); structured logs for routing and backpressure decisions.
- Document config precedence and migration; provide a “dry-run” validator for model switches.

KPIs to Track
- Failure rate (per provider), median/p95 latency, queue length, and circuit-breaker open time.
- Successful tool-call completion rate and retry cycles required.
- Cost per message/session (estimated); proportion of routed vs. pinned requests.
- Test coverage for adapters and replay determinism in CI.

Appendix: Suggested Commands (future work)
- /model promote <id> — Copy discovered model into user config with editable params.
- /provider profile <name> — Switch named credential/profile.
- /llm queue status — Inspect backpressure queues and pending requests.
- /provider health — Show health summary, error rates, and circuit-breaker states.
- /routing policy <name> — Switch between routing policies (cost, performance, reliability).
