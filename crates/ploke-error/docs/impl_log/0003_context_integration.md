# Implementation Log 0003 — Integrate ContextualError into top-level Error; fix lazy context

1) Summary of changes with rationale
- Added Error::Context(#[from] ContextualError) to ploke_error::Error
  Rationale: Allows ContextExt to wrap errors with rich context while preserving ergonomic `?` propagation via From.
- Exported SourceSpan and ContextExt from lib.rs
  Rationale: Make the new primitives easily usable across the workspace.
- Made ErrorContext::new lazy (no default backtrace capture)
  Rationale: Align with v3 performance goals; capture backtraces only when requested.
- Corrected ContextExt::with_path to wrap the original error into ContextualError::WithContext instead of synthesizing InternalError
  Rationale: Preserve the original error and its classification; attach context orthogonally.

2) Notes on correctness and design
- ContextualError and ErrorContext now derive Clone to preserve the existing Clone on Error.
- Severity mapping treats Error::Context as Severity::Error by default; policies can still decide how to emit.
- No external dependencies were added; changes are additive and backward-compatible.

3) Follow-ups
- Consider a dedicated diagnostics feature that implements miette::Diagnostic for Error and ContextualError using SourceSpan.
- Audit other ContextExt methods for richer file path propagation in snippet/backtrace helpers.
- Begin introducing ResultExt and ErrorPolicy per V3_PLAN.md.

Committed changes:
- lib.rs: re-exports and Error::Context variant + severity mapping.
- context.rs: Clone derives, lazy backtrace, and ContextExt::with_path fix.
