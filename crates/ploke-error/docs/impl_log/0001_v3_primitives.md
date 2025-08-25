# Implementation Log 0001 — Introduce v3 primitives (DomainError, Severity, Result alias)

1) Summary of changes with rationale
- Added DomainError enum and Error::Domain variant (with From via #[error(transparent)]).
  Rationale: Replace stringly-ad-hoc UiError/TransformError with a structured domain bucket to standardize cross-crate mappings while keeping semantics clear.
- Introduced Severity enum and Error::severity() to consistently classify errors as Warning, Error, or Fatal.
  Rationale: Enables policy-driven behavior and consistent UI/logging without contaminating library code paths.
- Added Result<T, E = Error> alias for ergonomic propagation.
  Rationale: Reduces boilerplate and makes return types uniform across crates.
- Kept existing UiError/TransformError for compatibility; they map to Severity::Error by default.
  Rationale: Incremental migration path without requiring immediate downstream refactors.
- No new dependencies or features added in this step.
  Rationale: Keep Phase 0 lightweight and non-invasive.

2) Observations of Rust best practices in action
- Used thiserror with transparent variant + #[from] to centralize conversions and enable idiomatic ? propagation.
- Kept changes additive and modular (new modules domain.rs, severity.rs) to preserve local reasoning and compile-time boundaries.
- Provided a simple Result alias leveraging std::result::Result to avoid noise at call sites.
- Derived Clone/Debug on DomainError and Copy on Severity for cheap and predictable usage patterns.
- Avoided premature context/diagnostic capture to keep performance costs opt-in (to be added in later steps).

3) Questions/blockers requiring decision
- Enum growth policy: Should Error be marked #[non_exhaustive] to future-proof additional variants without semver pain?
- Context integration: For ContextExt, should we introduce a dedicated Error::Contextual variant or store context separately to avoid breaking matches?
- Deprecation plan: Timeline for deprecating UiError/TransformError in favor of DomainError::Ui/Transform.
- SourceSpan: Confirm the target minimal struct and when to migrate ErrorContext away from proc_macro2::Span.
- Features: Which optional features to land next (diagnostic, tracing, serde), and their default-off gating strategy.

Next planned steps
- Add SourceSpan and the ContextExt trait (lazy, opt-in context capture).
- Prepare optional features (diagnostic/tracing) scaffolding without pulling new deps by default.
- Document initial cross-crate mapping guidance leveraging DomainError and Severity.
