# ploke-error v3 — Status Report, Gaps, Blockers, and Recommendations

Scope
- Crate: ploke-error
- Goal: Implement the v3 plan focused on a unified Error type, Severity, DomainError, Result alias, context extensions, policy-driven emission, and optional diagnostics/serde.

Progress so far (Phase 0 largely complete)
- Core surface
  - Error enum with Fatal, Warning, Internal, Domain, Context and transitional UiError/TransformError variants.
  - Severity enum and Error::severity() implemented.
  - Result<T, Error> alias exported.
  - DomainError added (structured domain taxonomy).
- Ergonomics
  - Context model: SourceSpan, ErrorContext; ContextExt with with_path/with_span/with_snippet/with_backtrace.
  - ResultExt provides policy-driven emission helpers.
- Policy layer
  - ErrorPolicy trait (Send + Sync), NoopPolicy.
  - TracingPolicy behind "tracing".
  - MiettePolicy behind "diagnostic" (new in 0006).
- Diagnostics (opt-in)
  - cfg_attr(feature = "diagnostic") derives across Error and sub-errors.
- Serialization (opt-in)
  - serde feature added and wired to dependency (new in 0006).
  - serde derives on Severity, DomainError, WarningError.

Gaps to completion (by plan section)
- Phase 1 (Conversions and Uniformity)
  - Not yet implemented across dependent crates (ploke-io, ploke-rag, ploke-db, syn_parser).
  - Need From mappings and consistent severity decisions per mapping tables.
- Phase 2 (Ergonomics and Policy Adoption)
  - No CombinedPolicy or app-level policy wiring provided yet; downstream crates must integrate.
  - No example of UI integration using MiettePolicy shown in docs/examples.
- Phase 3 (Feature roll-out and measurement)
  - Benchmarks and LOC reduction metrics not yet collected.
  - No workspace-wide diagnostics rendering examples or CLI flags to toggle features.
- Phase 4 (Deprecations)
  - UiError/TransformError are still present and not marked deprecated.
- Additional
  - No serde on Error/FatalError/InternalError/ContextualError due to non-serializable fields; requires design.
  - No smartstring or proc-span integration (intentionally deferred).
  - No CombinedPolicy, rate-limiting or dedupe policy support (potential enhancement).

Key blockers needing a decision
- Serialization strategy for non-serializable sources
  - Option A: Do not support serde for Error/Fatal/Internal; rely on Domain/Warning + external rendering.
  - Option B: Introduce feature-gated “flattened” representations (e.g., Serialize by stringifying sources and OS codes).
  - Option C: Provide mirror types (e.g., ErrorDto) solely for telemetry/logging with lossy conversion.
  - Decision required to proceed with serde coverage beyond the current safe subset.
- Deprecation schedule for UiError/TransformError
  - When to mark as #[deprecated], and how long to provide shims before removal.
- Policy composition
  - Whether to ship a CombinedPolicy abstraction in this crate or leave composition to application crates.
- Diagnostic labeling
  - How aggressively to add SourceSpan-based labels and NamedSource in ContextualError; requires lifetime and storage decisions.

Quality assessment and critique
- Strengths
  - Clear separation of concerns: policy-driven emission decouples side-effects from logic.
  - Good use of cfg_attr to keep optional integrations zero-cost by default.
  - ResultExt methods are ergonomically minimal and preserve control flow.
  - ContextExt captures context lazily, aligning with performance goals.
- Areas to improve
  - Error enum mixes structural variants and transitional string variants (UiError/TransformError). These should be deprecated per plan.
  - InternalError and FatalError contain trait object/Arc sources that complicate serialization and equality; consider lighter-weight variants or mirrored DTOs for telemetry.
  - ContextualError printing currently uses Debug of ErrorContext; richer Display/Diagnostic annotations would help users.
  - ResultExt uses &impl ErrorPolicy; taking &dyn ErrorPolicy may be clearer to downstream users, though both are equivalent at call sites.
  - TracingPolicy maps Fatal to error level (acceptable), but consider tagging fatal to ease search in logs (e.g., error with fatal=true field or custom event target).

Rust idioms and patterns to adopt
- Prefer &dyn Trait in public APIs when trait object expected at runtime; use generics when monomorphization is intended. For ResultExt, switching to &dyn ErrorPolicy could clarify intent.
- Provide From/Into conversions and small constructors for domain variants (e.g., DomainError::ui(msg)) to reduce boilerplate downstream.
- Consider AsRef<Path> where appropriate in context helpers; currently ContextExt::with_path takes Into<PathBuf>, which is fine but AsRef<Path> may reduce cloning.
- Use #[non_exhaustive] on public enums (DomainError, WarningError, InternalError, FatalError) to allow expansion without breaking changes.
- When adding diagnostics, implement Diagnostic fields like code(), help(), and related labels to improve UX.
- For policy combinators, consider a small enum or tuple struct that implements ErrorPolicy and delegates in sequence.

Recommendations and next steps
- Deprecate UiError and TransformError in a follow-up (Phase 2), with migration guidance pointing to DomainError.
- Add CombinedPolicy (optional): a small type that takes Vec<Box<dyn ErrorPolicy>> and iterates emit/classify with first-wins classification or a configurable strategy.
- Extend ContextualError diagnostics:
  - Add optional NamedSource and labels derived from SourceSpan when "diagnostic" feature is enabled.
  - Provide helper constructors on ErrorContext to build labels/snippets ergonomically.
- Finalize serde strategy:
  - Short-term: keep serde derives on Severity/Domain/Warning only and document the limitation.
  - Mid-term: add ErrorDto for telemetry with lossy conversions from Error; gate behind "serde" to avoid default overhead.
- Author mapping docs for dependent crates and begin Phase 1 adoption:
  - Provide per-crate conversion tables and a small example per offender.
- Add examples folder:
  - Mini binary demonstrating TracingPolicy and MiettePolicy usage.
- Metrics suite:
  - Add a doc or script to collect LOC/boilerplate deltas and enable simple benchmarks.

Risk considerations
- Over-serialization: forcing serde across trait objects will create brittle code; prefer DTOs if serialization is required.
- Feature creep: keep default minimal; ensure optional features are orthogonal and document combinations.
- Backward compatibility: use #[non_exhaustive] and maintain transparent conversions to reduce downstream breakage.

Conclusion
- The v3 core is solid and mostly in place. The primary remaining work is adoption across crates, policy composition ergonomics, richer diagnostics, and a deliberate serde strategy. Decisions on serde coverage and deprecation timing will unlock the next implementation steps.
