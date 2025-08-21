# ploke-error v3 Error Handling Plan
Higher functionality, lower boilerplate, and better performance with idiomatic Rust patterns.

Summary
- Replace ad-hoc error plumbing with a small, composable core: Severity, DomainError, Result alias, Context/Result extensions, and a policy-driven emission layer.
- Keep libraries pure (return Result) and move emission/side-effects into application policies.
- Provide feature-gated diagnostics (miette), tracing, serde, and smartstring to keep the core lean by default.

Quality Goals (see the separate criteria file for details)
- Minimal boilerplate, ergonomic propagation
- Performance with lazy/opt-in context
- Consistent taxonomy and mapping
- Composability and separation of concerns
- Observability with clean user/dev output
- Locality and readability of control flow

Problems Observed (from current offenders)
- Verbose, repeated boilerplate for mapping/propagating errors across async boundaries and actors
- Event emission/logging mixed with core IO/search logic
- Ad-hoc string-based domain variants and inconsistent mappings
- Eager backtrace/context capture and scattered context types
- Unnecessary matches around spawn/join result handling and manual ordering logic

Proposed Architecture (v3)
- Core types
  - Error: Keep as the single top-level error.
  - Severity: enum Severity { Warning, Error, Fatal } with Error::severity().
  - DomainError: Structured, non-ad-hoc domain cases (Ui, Transform, Db, Io, Rag, Ingest).
  - Result alias: pub type Result<T, E = Error> = core::result::Result<T, E>.
- Context extensions (ergonomics, zero-cost when unused)
  - SourceSpan: Stable, lightweight span (file + optional offsets/line/col).
  - ErrorContext: { span: Option<SourceSpan>, file_path: PathBuf, code_snippet: Option<String>, backtrace: Option<Backtrace> }.
  - ContextExt<Result>: with_path, with_span, with_snippet, with_backtrace (lazy capture).
- Policy-driven emission (for long-running apps)
  - ErrorPolicy: classify(&Error) -> Severity; emit(&Error).
  - NoopPolicy (default). TracingPolicy behind "tracing" feature. Miette rendering behind "diagnostic".
  - ResultExt and ErrorExt: emit_event(policy), emit_warning/error/fatal (policy-driven).
- Optional features (no default deps)
  - diagnostic (miette), tracing, serde, smartstring, proc-span, backtrace-default.
- Macros (thin, optional)
  - Optional helpers to reduce conversion noise (e.g., domain_ui!("msg")) if needed; keep minimal.

API Sketch (additions)
- pub type Result<T, E = Error> = core::result::Result<T, E>
- #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Severity { Warning, Error, Fatal }
- impl Error { pub fn severity(&self) -> Severity { … } }
- #[cfg(feature = "smartstring")] pub type Msg = smartstring::alias::String; else type Msg = Box<str>;
- #[derive(Debug, Clone, thiserror::Error)]
  pub enum DomainError {
    #[error("UI error: {message}")] Ui { message: Msg },
    #[error("Transform error: {message}")] Transform { message: Msg },
    #[error("Database error: {message}")] Db { message: Msg },
    #[error("IO error: {message}")] Io { message: Msg },
    #[error("RAG error: {message}")] Rag { message: Msg },
    #[error("Ingest error: {message}")] Ingest { message: Msg },
  }
- pub struct SourceSpan { file: PathBuf, start: Option<usize>, end: Option<usize>, line: Option<u32>, col: Option<u32> }
- pub trait ContextExt<T> { fn with_path(self, p: impl Into<PathBuf>) -> Result<T>; fn with_span(self, s: SourceSpan) -> Result<T>; fn with_snippet<S: Into<String>>(self, s: S) -> Result<T>; fn with_backtrace(self) -> Result<T>; }
- pub trait ErrorPolicy: Send + Sync { fn classify(&self, e: &Error) -> Severity; fn emit(&self, e: &Error); }
- pub trait ResultExt<T> { fn emit_event(self, policy: &impl ErrorPolicy) -> Self; /* plus emit_warning/error/fatal */ }
- Compatibility: keep existing UiError/TransformError temporarily; add Error::Domain(DomainError) now; deprecate in a later phase.

How this reduces boilerplate in offender crates
- ploke-io write.rs
  - Replace manual IoError -> PlokeError mapping with From conversions targeted into Error/DomainError/Fatal once, close to type definition.
  - Use Result alias and ? propagation across async boundaries.
  - Use ContextExt for path/snippet/backtrace only when helpful.
  - Let application install a policy to emit file-change events/logs; write.rs stops emitting directly.
- ploke-io actor.rs
  - Replace hand-rolled spawn/join error mapping with small helpers:
    - A bounded-concurrency routine returning Vec<Result<_>> while preserving order; move ordering logic into a util using indices once.
    - Use ResultExt::emit_event(policy) at boundaries where the app wants to continue.
  - Root of request handling stays focused on orchestration, not logging.
- ploke-rag core/mod.rs
  - Replace strings in RagError conversions with DomainError::Rag{..}.
  - Futures join with ? where possible and return Domain/Internal/Fatal consistently.
  - Screen BM25 fallback/strict policies without repeating error text mapping.
- ploke-tui events.rs
  - Stop constructing user-visible strings in core error types; use a rendering layer in the UI with miette when enabled.
  - Event emission flows through an ErrorPolicy; business logic remains free of display decisions.

Migration Milestones (step-by-step)
1) Phase 0 — Introduce v3 primitives (non-breaking)
   - Add modules in ploke-error: severity.rs, domain.rs, context_ext.rs, policy.rs, result_ext.rs.
   - Extend Error with Domain(DomainError) and severity().
   - Add Result alias and SourceSpan.
   - Add features: diagnostic, tracing, serde, smartstring, proc-span, backtrace-default.
   - Keep UiError/TransformError for now (compat).
2) Phase 1 — Centralize conversions
   - Define From mappings in ploke-io for IoError -> Error (Fatal/Domain/Warning/Internal per policy).
   - Define From mappings in ploke-rag for RagError -> Error (Domain/Internal/Fatal).
   - Localize all From impls next to their error type definitions.
3) Phase 2 — Ergonomics and policy adoption
   - Replace manual match boilerplate with Result alias and ContextExt/ResultExt.
   - In ploke-tui, introduce a TracingPolicy (cfg(feature="tracing")) and integrate emission at UI boundaries.
   - Enable miette rendering behind the diagnostic feature; map SourceSpan to labels when present.
4) Phase 3 — Feature roll-out and measurement
   - Enable tracing and diagnostic features in application crates; libraries stay lean.
   - Measure improvements using the criteria (see separate doc).
5) Phase 4 — Deprecations
   - Mark UiError/TransformError as deprecated with guidance to DomainError.
   - Begin replacing proc_macro2::Span in ErrorContext with SourceSpan; keep proc-span compatibility.
6) Phase 5 — Cleanup (major bump)
   - Remove deprecated variants and old context flows.
   - Ensure all crates implement standardized conversions and rely on policy-driven emission.

Success Metrics
- Boilerplate reduction: ≥30% fewer lines across offenders in error propagation/mapping blocks.
- Performance: no measurable regression in critical paths; context capture is opt-in.
- Consistency: all cross-crate conversions route through standardized From impls; DomainError replaces string variants.
- Observability: UI renders diagnostic-quality messages when features enabled; library code stays clean.
- Maintainability: fewer ad-hoc enums/strings; centralized policy for emission.

Risks and Mitigations
- Risk: Hidden coupling via policy usage. Mitigation: keep policy interfaces minimal, feature-gated, and unit-test boundaries.
- Risk: Overuse of DomainError. Mitigation: mapping guidance and lint/docs; keep Fatal/Internal for integrity/bug classes only.
- Risk: Feature explosion. Mitigation: no default features; optional integrations only.

Next Actions (for this crate)
- Implement new modules/types behind additive, non-breaking changes.
- Document mapping rules per crate.
- Provide small examples and migration guide snippets mirroring the offenders.
