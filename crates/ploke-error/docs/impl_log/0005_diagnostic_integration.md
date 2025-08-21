# Implementation Log 0005 — Baseline miette diagnostic integration (feature-gated)

1) Summary of changes with rationale
- Added optional "diagnostic" feature with miette dependency.
  Rationale: Provide rich, opt-in diagnostics without imposing runtime costs or new deps by default.
- Derived miette::Diagnostic for Error and sub-error enums (FatalError, InternalError, WarningError, DomainError, ContextualError) behind the feature flag.
  Rationale: Enable immediate Diagnostic compatibility with minimal code changes and without changing Display messages.
- Kept ContextualError transparent to its source for now when rendering diagnostics.
  Rationale: Preserve original error information; contextual labels/snippets will be added incrementally.

2) Notes on correctness and design
- All derives are gated with cfg_attr(feature = "diagnostic", …), preserving zero-overhead when the feature is disabled.
- No public API breakage; only additive attributes and optional dependency.
- We intentionally did not attempt to provide source snippets/labels yet to avoid storing extra state in ErrorContext; this will be introduced in a follow-up with careful lifetimes.

3) Fixes and cleanup
- Removed an unused import (std::sync::Arc) from policy.rs to keep the crate warning-clean.

4) Follow-ups
- Extend ContextualError diagnostics to include SourceSpan-based labels and optional NamedSource when code_snippet is available.
- Consider mapping ploke_error::Severity to miette::Severity for better downstream rendering.
- Add serde feature-gated derives for error types to enable structured logs and telemetry.
- Provide examples in docs and integrate diagnostics in ploke-tui behind the feature flag.

Committed changes:
- Cargo.toml: add optional miette dependency; wire "diagnostic" feature to it.
- Derive miette::Diagnostic for error enums behind feature gate.
- Minor cleanup in policy.rs.
