# Implementation Log 0004 — Add policy-driven error emission

1) Summary of changes with rationale
- Added ErrorPolicy trait to separate error classification and emission from core logic
  Rationale: Enables applications to control how errors are handled without contaminating library code
- Implemented NoopPolicy (default) and TracingPolicy (behind "tracing" feature)
  Rationale: Provides sensible defaults while allowing rich integration with tracing infrastructure
- Added ResultExt trait with emit_event and severity-specific emission methods
  Rationale: Enables ergonomic error emission at boundaries while preserving result propagation
- Added optional "tracing" feature to keep core crate lean
  Rationale: Follows v3 principle of optional integrations to avoid unnecessary dependencies

2) Notes on correctness and design
- ErrorPolicy is Send + Sync to support cross-thread usage
- All emission methods on ResultExt return Self, allowing chaining and preserving control flow
- TracingPolicy uses appropriate tracing levels based on error severity
- No breaking changes; all additions are backward-compatible

3) Follow-ups
- Add miette diagnostic integration behind "diagnostic" feature
- Implement serde serialization behind "serde" feature
- Add smartstring optimization behind "smartstring" feature
- Document usage patterns and migration examples
- Begin integrating into application crates (ploke-tui, etc.)

Committed changes:
- policy.rs: ErrorPolicy trait with NoopPolicy and TracingPolicy implementations
- result_ext.rs: ResultExt trait for policy-driven emission
- lib.rs: Module declarations and re-exports
- Cargo.toml: Optional features including "tracing" with dependency
