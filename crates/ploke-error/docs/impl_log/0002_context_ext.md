# Implementation Log 0002 — Add ContextExt trait and SourceSpan

1) Summary of changes with rationale
- Added SourceSpan struct to provide a stable, lightweight alternative to proc_macro2::Span
  Rationale: Decouples error context from proc-macro ecosystem, making it usable in runtime-only crates
- Implemented ContextExt trait with extension methods for Result<T, Error>
  Rationale: Provides ergonomic, lazy context attachment without contaminating happy paths
- Enhanced ErrorContext to work with SourceSpan instead of proc_macro2::Span
  Rationale: Aligns with the new stable context representation
- Added with_span, with_snippet, and with_backtrace methods to ErrorContext
  Rationale: Provides a fluent API for building context incrementally

2) Observations of Rust best practices in action
- Used a fluent builder pattern for SourceSpan to make construction ergonomic
- Implemented ContextExt as a trait extension for Result<T, Error> following Rust's extension trait pattern
- Kept context capture lazy (only when explicitly requested) to maintain performance
- Used generic bounds (Into<PathBuf>, Into<String>) to maximize flexibility for callers
- Maintained backward compatibility by keeping existing ErrorContext fields while enhancing them

3) Questions/blockers requiring decision
- Should we completely remove proc_macro2::Span from ErrorContext or keep it for compatibility?
- How should we handle the conversion between proc_macro2::Span and SourceSpan when needed?
- Should ContextExt methods modify existing context or always create new ContextualError wrappers?
- What's the right balance between eager and lazy context capture for different use cases?
- Should we add more specific context types (e.g., for function calls, module paths)?

Next planned steps
- Add optional "diagnostic" feature with miette integration
- Implement ErrorPolicy trait for policy-driven error handling
- Add ResultExt trait for result-level operations
- Begin documenting cross-crate mapping guidance
