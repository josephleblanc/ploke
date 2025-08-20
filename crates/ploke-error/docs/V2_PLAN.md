# ploke-error v2 Error System Plan
Ergonomic, context-rich, and efficient error handling across the ploke workspace.

This document distills essentials from REVIEW_AND_PLAN.md, clarifies open questions, and provides an actionable v2 plan focused on consistency, performance, and ease of integration for both libraries and long-running applications.

---

## 1) Executive Summary

- Keep a single top-level Error type with clear severity semantics.
- Add lightweight severity and result ergonomics.
- Introduce DomainError for structured, non-ad hoc domain cases.
- Provide context-capture that is lazy and purely opt-in.
- Provide a minimal, feature-gated integration surface for diagnostics, tracing, and string optimization.
- Define a clear separation of concerns: libraries return results; applications classify/emit.
- Add an event/policy layer so long-running applications can emit and continue without panicking or tearing down control flow.

---

## 2) Current Snapshot (What’s already done)

- Top-level Error with variants: Fatal(FatalError), Warning(WarningError), Internal(InternalError), plus stringy UiError and TransformError.
- Established sub-error modules with thiserror: fatal.rs, warning.rs, internal.rs.
- ErrorContext and ContextualError exist but are not integrated into Error’s main flow; context capture uses proc_macro2::Span and captures Backtrace unconditionally in new().
- Conversions in other crates exist but are ad hoc (some map to Warning, others to Internal).
- Manual Clone for FatalError because of Arc fields.

These provide a solid foundation but are missing severity, domain-typed errors, optional diagnostics, lazy context, and an event/policy layer.

---

## 3) What remains to be done (v2 Goals)

- Consistent severity modeling
  - Add Severity enum and Error::severity().
  - Provide a Result<T> alias for convenience.

- Domain errors (non-ad hoc)
  - Add Error::Domain(DomainError) with structured payloads (Ui, Transform, Db, Io, Rag, Ingest).
  - Deprecate UiError and TransformError variants in a later phase.

- Context and diagnostics
  - Introduce SourceSpan (file + optional offsets/line/col).
  - Replace proc_macro2::Span in ErrorContext with Option<SourceSpan>.
  - Make Backtrace acquisition lazy and opt-in only via helper methods.
  - Feature-gate diagnostics (miette) and provide Diagnostic impls when enabled.

- Event and policy layer for long-running apps
  - Add ErrorPolicy trait with classify(&Error) -> Severity and emit(&Error).
  - Provide default NoopPolicy; add TracingPolicy behind tracing feature.

- Lightweight integration surface
  - Provide ContextExt and Error/Result extension traits for ergonomic wrapping and emission.
  - Provide optional serde, tracing, miette, smartstring, and proc-span features.

- Cross-crate conversions
  - Standardize conversions into Error based on mapping rules (db, io, ingest, rag, embed).
  - Keep conversions centralized and documented.

- Migration/deprecations
  - Provide phased migration from ad hoc string variants to DomainError.
  - Keep current API compatible initially; add deprecations with clear guidance.

---

## 4) Performance and Efficiency Considerations

- Avoid unnecessary allocations:
  - Prefer Box<str> or Cow<'static, str> for messages by default.
  - Optionally enable SmartString via a feature for frequently-instantiated short messages:
    - Feature: smartstring (off by default)
    - Type alias: pub type Msg = SmartString; else Msg = Box<str>
  - Use PathBuf where necessary; consider Arc<PathBuf> only when sharing across threads or clones is common.

- Lazy context capture:
  - No automatic Backtrace capture in ErrorContext::new().
  - Add with_backtrace() helper to capture on-demand.
  - In diagnostics workflows (miette), compute labels/spans only when feature is enabled.

- Minimal default dependency footprint:
  - Default features: none (lean core).
  - diagnostics (miette), tracing, serde, smartstring, proc-span are opt-in.
  - No default logging dependencies to keep library footprint small.

- Zero-cost for non-users:
  - Extension traits should compile away when unused; event/policy layer should be thin and behind features.

---

## 5) Features and Optional Integrations

Recommended feature flags:
- diagnostic: Adds miette and implements Diagnostic for Error and subtypes.
- tracing: Provides TracingPolicy and emission helpers using the tracing crate.
- serde: Adds Serialize/Deserialize for Error and subtypes where feasible.
- smartstring: Uses SmartString for error message storage via a type alias.
- proc-span: Adds conversions from proc_macro2::Span to SourceSpan and optional helpers.
- backtrace-default: Opt-in default backtrace capture when building ErrorContext, otherwise only captured via with_backtrace().

Rationale: Let importing crates choose footprint and behavior. Libraries can depend only on ploke-error core; applications can enable features.

---

## 6) Traits and Minimal API for Integration

- Severity
  - enum Severity { Warning, Error, Fatal }
  - fn Error::severity(&self) -> Severity

- Result alias
  - pub type Result<T, E = Error> = std::result::Result<T, E>;

- DomainError (new)
  - Ui { message: Msg }, Transform { message: Msg }, Db { message: Msg }, Io { message: Msg }, Rag { message: Msg }, Ingest { message: Msg }

- Context types
  - SourceSpan { file: PathBuf, start: Option<usize>, end: Option<usize>, line: Option<u32>, col: Option<u32> }
  - ErrorContext { span: Option<SourceSpan>, file_path: PathBuf, code_snippet: Option<String>, backtrace: Option<Backtrace> }

- ContextExt (ergonomics)
  - For Result<T, Error>: with_path, with_span, with_snippet, with_backtrace
  - Captures context lazily and attaches via ContextualError::WithContext(Box<Error>, ErrorContext)

- Error/Result emission (for long-running apps)
  - ErrorPolicy: classify(&Error) -> Severity, emit(&Error)
  - NoopPolicy (default)
  - TracingPolicy (cfg(feature = "tracing")): emits via tracing with severity mapping
  - ResultExt and ErrorExt:
    - emit_event(self, policy: &impl ErrorPolicy) -> Self
    - emit_warning/emit_error/emit_fatal convenience methods (policy-driven)

These traits make it easy to adopt ploke-error in libraries and to implement non-disruptive error flows in applications.

---

## 7) Libraries vs Applications: Error Flow

- Libraries
  - Return Result<T, ploke_error::Error>.
  - Do not emit globally; do not assume a global logger/event bus.
  - Select Error variants carefully:
    - Syntax/file IO/system integrity -> Fatal
    - Recoverable or user-actionable items -> Warning
    - Internal bugs, NYI, channel failures -> Internal
    - Domain errors that are “errors” but not warnings/fatal -> DomainError
  - Add context sparingly and lazily via ContextExt (path/snippet/backtrace when available and helpful).
  - Avoid policy decisions; leave classification tweaks to the caller.

- Applications (long-running)
  - Install an ErrorPolicy at subsystem boundaries (task loops, services).
  - Use ResultExt::emit_* to log/emit and continue when appropriate (e.g., user input errors, intermittent network issues).
  - Reserve Fatal to terminate a pipeline/task with a graceful unwind while keeping the process alive.
  - Optional: With diagnostic feature enabled, render user-friendly reports (miette) when appropriate.

This separation keeps libraries composable and applications in control of error handling and emission.

---

## 8) Mapping Guidance (Updated)

- Fatal:
  - File IO failures that block progress (read/write/permissions)
  - Content mismatch for indexed/tracked files
  - Syntax parse failures that invalidate source
  - Shutdown initiated

- Warning:
  - Unlinked modules, orphan files
  - NotFound when operation is optional or exploratory

- Internal (Error severity):
  - CompilerError and InvalidState
  - Channel failures, NYI features
  - Unexpected states during parsing/resolution

- Domain (Error severity by default):
  - UI flow issues
  - Transform validation failures
  - DB non-corruption failures
  - RAG search failures
  - Ingest errors that aren’t syntax/IO/fatal

Applications may upgrade/downgrade Domain via ErrorPolicy as needed.

---

## 9) Open Questions to Confirm Before Implementation

- Do we want DomainError coverage for all major subsystems now (Ui, Transform, Db, Io, Rag, Ingest), or add incrementally?
- Should NotFound in DB be Warning by default or Domain(Db) at Error severity? Proposal: Warning when user-visible and recoverable; Domain(Db) otherwise.
- Are we committing to miette for diagnostics, or should we also consider ariadne or similar? Proposal: miette only, feature-gated.
- Is SmartString worth the extra dependency? Proposal: optional smartstring feature with Msg alias; default to Box<str>.
- Should we capture Backtrace by default in debug builds? Proposal: keep opt-in only; enable backtrace-default feature when desired.

---

## 10) Migration Plan (Revised)

Phase 0: Additions (non-breaking)
- Add Severity, Result alias, DomainError, SourceSpan, ContextExt, ErrorPolicy, ResultExt/ErrorExt.
- Add feature flags: diagnostic, tracing, serde, smartstring, proc-span, backtrace-default.
- Keep UiError and TransformError variants.

Phase 1: Conversions and adoption
- Update conversions in dependent crates to map into Domain/Fatal/Internal/Warning consistently.
- Adopt Result alias in workspace crates.
- Integrate ErrorPolicy in application crates (e.g., ploke-tui), using tracing when enabled.

Phase 2: Deprecations
- Deprecate UiError and TransformError with guidance to use DomainError variants.
- Consider deprecating proc_macro2::Span in context.rs in favor of SourceSpan; keep proc-span feature for compatibility.

Phase 3: Cleanup (major version)
- Remove deprecated variants and old context patterns.
- Ensure all workspace crates use standardized conversions and policies.

---

## 11) Deliverables (for this crate)

- New modules and items:
  - severity.rs: Severity enum and Error::severity()
  - domain.rs: DomainError and Msg alias behind feature smartstring
  - context_ext.rs: SourceSpan + ContextExt and helpers
  - policy.rs: ErrorPolicy, NoopPolicy, TracingPolicy (cfg(feature = "tracing"))
  - result_ext.rs: ResultExt and ErrorExt with emit helpers
- lib.rs changes:
  - Re-export Severity, DomainError, Result alias, ContextExt, ErrorPolicy, ResultExt, ErrorExt.
  - Feature-gate diagnostics, tracing, serde, smartstring, proc-span, backtrace-default.
- docs:
  - Update crate-level docs with mapping guidance and examples.
  - Provide examples for libraries vs applications usage.

No behavioral changes by default (without features) to keep integration smooth.

---

## 12) Minimal API Sketch (Illustrative)

```rust
pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity { Warning, Error, Fatal }

impl Error {
    pub fn severity(&self) -> Severity {
        match self {
            Error::Warning(_) => Severity::Warning,
            Error::Fatal(_) => Severity::Fatal,
            Error::Internal(_) => Severity::Error,
            // new
            Error::Domain(_) => Severity::Error,
            // legacy
            Error::UiError(_) | Error::TransformError(_) => Severity::Error,
        }
    }
}

#[cfg(feature = "smartstring")]
pub type Msg = smartstring::alias::String;
#[cfg(not(feature = "smartstring"))]
pub type Msg = Box<str>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("UI error: {message}")]
    Ui { message: Msg },
    #[error("Transform error: {message}")]
    Transform { message: Msg },
    #[error("Database error: {message}")]
    Db { message: Msg },
    #[error("IO error: {message}")]
    Io { message: Msg },
    #[error("RAG error: {message}")]
    Rag { message: Msg },
    #[error("Ingest error: {message}")]
    Ingest { message: Msg },
}

#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub file: std::path::PathBuf,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

pub trait ContextExt<T> {
    fn with_path(self, path: impl Into<std::path::PathBuf>) -> Result<T>;
    fn with_span(self, span: SourceSpan) -> Result<T>;
    fn with_snippet<S: Into<String>>(self, snippet: S) -> Result<T>;
    fn with_backtrace(self) -> Result<T>;
}

pub trait ErrorPolicy: Send + Sync {
    fn classify(&self, e: &Error) -> Severity;
    fn emit(&self, e: &Error);
}
```

---

## 13) Acceptance Criteria

- Core crate builds with no default features and no new transitive heavy deps.
- With features enabled, provides:
  - miette diagnostics (diagnostic)
  - tracing emission (tracing)
  - serde serialization (serde)
  - smartstring optimization (smartstring)
  - proc_macro2 conversion (proc-span)
- Library ergonomics:
  - Result alias, ContextExt, DomainError available and documented.
- Application ergonomics:
  - ErrorPolicy/ResultExt/ErrorExt available and documented.
- Conversions:
  - Initial standardized conversions implemented for workspace crates (tracked in their repos).

---

## 14) Conclusion

This v2 plan focuses on ergonomics, consistency, and performance:
- One Error type with severity semantics and domain modeling.
- Lazy, opt-in context capture.
- Feature-gated integrations (diagnostics, tracing, serde, smartstring, proc-span).
- A policy layer enabling long-running apps to emit-and-continue, while libraries remain pure and composable.
- A clear migration path with minimal breakage.

This positions ploke-error as a stable, efficient foundation for uniform error handling across the workspace.
