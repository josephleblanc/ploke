# ploke-error Code Review and Refactor Plan

This document reviews the current state of the `ploke-error` crate and proposes a refactor plan to deliver a consistent, ergonomic, and context-rich error system across the `ploke` workspace.

## Executive Summary

- The current error taxonomy is inconsistent across crates.
- Context capture exists (ErrorContext) but is not integrated into the main error flow.
- Cross-crate conversions into `ploke_error::Error` are ad hoc and conflate severity.
- Diagnostics and user-facing output vary by crate (thiserror, anyhow, color_eyre).
- We propose a unified, ergonomic API in `ploke-error` with:
  - A single `Error` type, a `Severity` enum, and a `Result` alias.
  - Rich context via a lightweight, stable `SourceSpan` and contextual extension trait.
  - Optional "diagnostic" feature implementing `miette::Diagnostic`.
  - A clear mapping strategy for conversions from other crates.
  - Consistent semantics for Fatal vs Internal vs Warning with documented guidance.
  - Migration guidance and incremental roll-out plan.

---

## Current State Review

Modules and types:
- lib.rs
  - Re-exports module types and defines `Error` with variants:
    - Fatal(FatalError), Warning(WarningError), Internal(InternalError)
    - UiError(String), TransformError(String)
  - Provides `is_warning()`
- fatal.rs: FatalError variants for syntax/db corruption/file ops/utf8/etc.
  - Manual Clone impl due to Arc fields
- warning.rs: WarningError with UnlinkedModules, OrphanFile, UnresolvedRef, PlokeDb(String)
- internal.rs: InternalError with CompilerError, InvalidState, NotImplemented, EmbedderError
- context.rs: ErrorContext with Span, file_path, code_snippet, backtrace; ContextualError::WithContext(Box<Error>, ErrorContext)

Observations:
1. Inconsistent taxonomy
   - Top-level `Error` mixes structural variants (Fatal/Warning/Internal) with ad hoc domain strings (UiError, TransformError).
   - Some crates convert their domain errors into `ploke_error::Error` inconsistently (e.g., db errors to Warning; rag errors to Internal).
2. Context is bolt-on
   - ErrorContext is separate and unused by the primary error variants.
   - `proc_macro2::Span` suggests coupling to proc-macro ecosystems; may not be ideal for runtime-only crates.
   - Backtraces are captured unconditionally by `ErrorContext::new`, which can be costly and not always needed.
3. Cross-crate conversions
   - Present in other crates, but not standardized. Mapping choices (Warning vs Fatal vs Internal) lack clear guidance.
4. Ergonomics
   - No workspace-wide `Result<T>` alias.
   - Requiring crate authors to choose among multiple top-level variants reduces clarity.
   - No consistent error codes or diagnostics for user presentation.
5. Coupling/Smells
   - context.rs depends on `PathBuf` via `super::*` import from lib.rs.
   - Manual Clone for FatalError is error-prone long-term.
6. User-facing variability
   - Some crates use thiserror; tui may use anyhow/color_eyre.
   - Lack of a coherent "diagnostic path" for user-friendly reports.

---

## Goals

- Consistency: A single `Error` type with a predictable severity and domain.
- Context: Lightweight source context propagation without mandatory proc-macro dependencies.
- Ergonomics: Concise helpers to create, wrap, and convert errors; type alias for `Result`.
- Diagnostics: Optional integration with `miette` (or similar) for rendering rich error messages.
- Stability: Backwards-compatible additions first; structured deprecation plan next.
- Performance: Lazy/opt-in context capturing (backtrace/snippets) only when requested.

---

## Proposed Design

### 1) Core Types

- Error: keep as the single top-level error type
  - Variants:
    - Fatal(FatalError)
    - Warning(WarningError)
    - Internal(InternalError)
    - Domain(DomainError)  // new, to avoid ad hoc UiError/TransformError strings
- Severity: add an enum and a method on Error
  - enum Severity { Warning, Error, Fatal }
  - impl Error { fn severity(&self) -> Severity }
    - Fatal(_) => Fatal
    - Warning(_) => Warning
    - Internal(_) | Domain(_) => Error  // default "error", not warning or fatal
- Result alias:
  - pub type Result<T, E = Error> = std::result::Result<T, E>;

- DomainError:
  - Encapsulates errors by sub-system without deciding severity.
  - Variants with structured payloads, not just String:
    - Ui { message: String }
    - Transform { message: String }
    - Db { message: String }    // Optional; can also map to Internal/Fatal directly
    - Io { message: String }
    - Rag { message: String }
    - Ingest { message: String }
  - Rationale: removes arbitrary string variants in `Error` and provides a flexible place to attach domain-specific info/codes.

### 2) Context and Diagnostics

- SourceSpan (new, stable) replaces raw `proc_macro2::Span` in the core context:
  - struct SourceSpan {
      file: PathBuf,
      start: Option<usize>, end: Option<usize>, // byte offsets if available
      line: Option<u32>, col: Option<u32>,
    }
  - Lightweight and applicable outside proc-macro contexts.
- ErrorContext:
  - Replace Span with Option<SourceSpan>
  - Make backtrace optional and captured only via helper methods, not always in new()
  - Keep code_snippet optional
- ContextualError:
  - Integrate with Error via helper methods:
    - trait ContextExt<T> {
        fn with_path(self, path: impl Into<PathBuf>) -> Result<T>;
        fn with_span(self, span: SourceSpan) -> Result<T>;
        fn with_snippet<S: Into<String>>(self, snippet: S) -> Result<T>;
        fn with_backtrace(self) -> Result<T>; // capture lazily
      }
  - Implementation: wraps Err(e) into Error::Internal/Error plus ContextualError (or stash context in a rich wrapper type).
  - Optional "diagnostic" feature: implement `miette::Diagnostic` for Error and include `SourceSpan` as labels where available.

### 3) Cross-Crate Conversions (From/Into)

Provide clear mapping guidance and implement standard conversions:

- ploke-db::DbError -> ploke_error::Error
  - NotFound => Warning(UnresolvedRef?) or Domain(Db { message, code }) with Error severity.
  - QueryConstruction/QueryExecution => Internal(CompilerError(...)) or Domain(Db { ... }).
  - UuidConv/Cozo => Internal(CompilerError(...)) or Domain(Db { ... }).
  - Rationale: Db errors are typically not Fatal unless data corruption. If db signals corruption, map to Fatal(DatabaseCorruption(...)).

- ploke-io::IoError -> ploke_error::Error
  - ContentMismatch => Fatal(ContentMismatch { ... })
  - ParseError => Fatal(SyntaxError(msg))
  - OutOfRange => Fatal(FileOperation { ...invalid input... })
  - FileOperation => Fatal(FileOperation { ... })
  - Utf8 => Fatal(Utf8 { ... })
  - Recv => Internal(CompilerError(...))
  - ShutdownInitiated => Fatal(ShutdownInitiated)
  - Rationale: IO tends to be fatal for the current operation. Some may be downgraded to Warning based on caller policy (future: policy knobs).

- ingest/syn_parser::SynParserError -> ploke_error::Error
  - Syntax/Parsing => Fatal(SyntaxError)
  - InternalState => Internal(InvalidState(...))
  - MultipleErrors => Internal(CompilerError(joined))
  - NotFound/Resolution issues => Domain(Ingest/Transform { ... }) or Warning based on UX needs.
  - Rationale: Parsing issues are usually fatal to the ingest task, not necessarily fatal to the process.

- rag::RagError -> ploke_error::Error
  - Db => map via DbError mapping rule
  - Channel => Internal(CompilerError("channel ..."))
  - Embed => Internal(NotImplemented or EmbedderError(..)) or Domain(Rag { ... })
  - Search => Domain(Rag { ... }) or Internal(NotImplemented) depending on maturity

Key: Document the mapping expectations and keep them consistent. DomainError enables "error but not warning/fatal" without inventing new top-level variants.

### 4) Optional Features

- Feature: diagnostic
  - Adds `miette` dependency
  - Implements `miette::Diagnostic` for Error, FatalError, WarningError, InternalError, DomainError
  - Uses `SourceSpan` labels to improve display
  - Enables rich user-facing reports in `ploke-tui` while core crates remain lean
- Feature: serde
  - Derive Serialize/Deserialize for Error and subtypes where feasible to enable structured logging and telemetry
- Feature: tracing
  - Integrate with the `tracing` crate for structured, leveled emission.
  - Provide an extension trait to emit events with `Severity` mapped to tracing levels (Warning -> warn, Error/Internal/Domain -> error, Fatal -> error with marker).
  - Keep behind a feature flag so core libraries don’t pull `tracing` by default. Recommended for applications (e.g., `ploke-tui`).

### 5) Helpers and Macros

- Add Result alias and ContextExt to ease usage.
- Consider small macros:
  - fatal!(...) -> Error::Fatal(FatalError::...)
  - warn!(...) -> Error::Warning(WarningError::...)
  - internal!(...) -> Error::Internal(InternalError::...)
  - domain!(Ui, "...") -> Error::Domain(DomainError::Ui { message: ... })
- Keep macros minimal to maintain clarity.

### 6) Policy Layer (Severity classification and event emission)

- Introduce an `ErrorPolicy` trait (Send + Sync) that dependent crates can implement to:
  - classify(&Error) -> Severity to downgrade/upgrade Domain errors (and, when appropriate, Warning/Internal) at runtime without changing core error types.
  - emit(&Error) to route errors to logs/event bus instead of short-circuiting control flow in long-running loops.
- Provide a default `NoopPolicy`. In `ploke-tui`, implement a policy that:
  - emits via `tracing` when the "tracing" feature is enabled,
  - renders via `miette` when the "diagnostic" feature is enabled.
- Best practices:
  - Libraries: Prefer returning `Result<T, ploke_error::Error>`; avoid global emission.
  - Applications/binaries: Apply an `ErrorPolicy` at subsystem boundaries to decide when to emit or escalate.

---

## API Sketch

Note: Illustrative only; does not change current code yet.

```rust
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone)]
pub enum Severity { Warning, Error, Fatal }

impl Error {
    pub fn severity(&self) -> Severity {
        match self {
            Error::Warning(_) => Severity::Warning,
            Error::Fatal(_) => Severity::Fatal,
            Error::Internal(_) | Error::Domain(_) => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("UI error: {message}")]
    Ui { message: String },
    #[error("Transform error: {message}")]
    Transform { message: String },
    #[error("Database error: {message}")]
    Db { message: String },
    #[error("IO error: {message}")]
    Io { message: String },
    #[error("RAG error: {message}")]
    Rag { message: String },
    #[error("Ingest error: {message}")]
    Ingest { message: String },
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
```

Example usage:

```rust
use ploke_error::{Error, FatalError, Result, ContextExt, DomainError};

fn do_work() -> Result<()> {
    // ...
    Err(Error::Domain(DomainError::Transform { message: "bad schema".into() }))
        .with_path("/tmp/input.json")?
}
```

---

## Migration Plan

Phased, with minimal breakage:

Phase 0: Additions (non-breaking)
- Add `pub type Result<T, E = Error>`.
- Add `Severity` enum and `Error::severity()`.
- Add `DomainError` and `Error::Domain(DomainError)` variant.
- Add `SourceSpan` and `ContextExt` trait (kept in a new `context_ext` module).
- Add optional `diagnostic` feature and `miette` implementations gated behind it.
- Keep existing `UiError` and `TransformError` in `Error` for now.

Phase 1: Conversions and Uniformity
- Introduce new `From` impls in dependent crates to map into:
  - Domain errors where applicable (`Ui`, `Transform`, `Db`, `Io`, etc.)
  - Internal vs Fatal consistently based on documented rules.
- Introduce and adopt an `ErrorPolicy` in application crates (e.g., `ploke-tui`) to classify Domain errors into Warning/Error/Fatal at runtime and to emit events via `tracing`/`miette` when features are enabled.
- Update `ploke-tui` to render `miette::Diagnostic` when `diagnostic` feature is enabled.
- Update read-only conversions in workspace gradually to the new mappings.
- Document mapping rules and policy guidance in this crate’s docs.

Phase 2: Deprecations
- Deprecate `UiError` and `TransformError` variants in `Error` with a clear message:
  - “Use Error::Domain(DomainError::Ui { .. }) instead.”
- Consider replacing `proc_macro2::Span` in ErrorContext with `SourceSpan`.
- Provide `#[cfg(feature = "compat")]` shims for a transition period.

Phase 3: Cleanup
- Remove deprecated variants and old context pattern in a major version bump.
- Ensure all workspace crates use the standardized conversions and helpers.

---

## Mapping Guidance (Initial)

- Fatal:
  - File IO failures that prevent progress (read/write/permissions)
  - ContentMismatch for indexed files
  - Syntax parsing failures that invalidate source
  - ShutdownInitiated
- Warning:
  - Unlinked modules
  - Orphan files
  - NotFound conditions when the operation is optional/recoverable
- Internal (Error severity):
  - CompilerError
  - InvalidState bugs
  - Channel failures
  - Unimplemented/NYI features
- Domain (Error severity by default):
  - UI flow issues, Transform validation failures, DB non-corruption failures, RAG search failures

Provide explicit mapping tables in crate docs for each workspace crate to maintain consistency.

---

## Implementation Notes and Risks

- Backtrace capture: Use `std::backtrace::Backtrace` lazily; avoid overhead on happy paths.
- proc_macro2::Span: Avoid in core runtime; prefer `SourceSpan`. Keep a conversion helper from `Span` when used in proc-macro contexts.
- Manual Clone in FatalError: Acceptable; could be simplified if Arc fields change, but correctness > brevity.
- `use super::*` dependency for PathBuf in context.rs: Tight coupling; prefer explicit imports in each module.
- Optional `miette`: Keep feature-gated to avoid pulling it into core crates unnecessarily.
- Avoid anyhow/color_eyre in core: confine to UI crates; core uses `thiserror` + optional `miette`.
- Forward OS error codes: When converting `std::io::Error`, forward `raw_os_error()` into diagnostics (e.g., as a code or extra field) and include in Display where appropriate. Do not invent synthetic codes for other domains.
- Thread safety: `ErrorPolicy` and any event-emission trait must be `Send + Sync` to support cross-thread use in long-running services.
- Source mapping reliability: "Reliable source mapping" refers to the ability to map internal items to file/byte/line positions even through transformations (e.g., macro expansion). We will not track macro expansion; `SourceSpan` will be best-effort and record file, optional byte offsets, and optional line/col, plus an optional snippet.

---

## Checklist

- Add: Result alias, Severity, DomainError, SourceSpan, ContextExt, optional miette integration.
- Document: Mapping rules per crate; examples; migration steps.
- Implement: New From conversions in workspace crates.
- Deprecate: UiError, TransformError.
- Update: context.rs to not rely on `super::*` for PathBuf, and to make backtrace lazy.
- Validate: Consistent behavior in ploke-db, ploke-io, ploke-rag, syn_parser, ploke-tui.

---

## Decisions and Clarifications

- NotFound policy:
  - Database search “not found” is Warning severity. Represent as a Warning (e.g., reuse `WarningError::UnresolvedRef` or add a dedicated `WarningError::NotFound { what, where }` during implementation).
  - Parser/graph “expected node missing” indicates an invalid internal state and should terminate that pipeline. Map to `InternalError::InvalidState` (or a dedicated internal variant) and surface as Error severity (or Fatal if the application policy dictates).
  - Malformed user code ends the parsing task, bubbles up a clear error (typically `FatalError::SyntaxError` or a domain-specific ingest error), and allows the UI to continue running.
- Error codes:
  - Forward OS error codes when available (e.g., `std::io::Error::raw_os_error()`). We do not invent synthetic numeric codes for other domains.
  - Include OS codes in diagnostics (as `miette::Diagnostic::code` or related metadata) and optionally in Display strings where it aids supportability.
- SourceSpan and “reliable source mapping”:
  - By “reliable” we mean accuracy through transformations like macro expansion and re-exports. We will not attempt macro-expansion mapping; `SourceSpan` is best-effort and records file, optional byte offsets, and optional line/col, with optional snippet capture.
  - Provide conversions from `proc_macro2::Span` when present; otherwise allow callers to construct `SourceSpan` from known file/offsets.
- Policy layer:
  - Add an `ErrorPolicy` trait (Send + Sync) for runtime classification of errors into severities and for event emission without forcing return-based control flow.
  - Provide guidance and a default no-op implementation; recommend that applications (e.g., `ploke-tui`) implement a policy that emits via `tracing` and renders via `miette` when features are enabled.

---

## Conclusion

This plan establishes a cohesive error strategy centered on a single `Error` type with severity awareness, consistent domain modeling, and optional rich diagnostics. It supports incremental migration and results in clearer, more actionable error handling both in logs and user interfaces.
