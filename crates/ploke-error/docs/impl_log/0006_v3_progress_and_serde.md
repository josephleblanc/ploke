# Implementation Log 0006 — Wire serde feature and add MiettePolicy

1) Summary of changes with rationale
- Wired "serde" feature to the serde dependency (optional) in Cargo.toml.
  Rationale: Allow downstream crates to opt into structured serialization without impacting the default footprint.
- Added cfg_attr(feature = "serde") derives for Severity, DomainError, and WarningError.
  Rationale: These types are frequently sent across process/UI boundaries and are safe to serialize.
- Introduced MiettePolicy behind the "diagnostic" feature, and re-exported it from lib.rs.
  Rationale: Provide a ready-to-use policy that renders rich error reports via miette::Report for applications that opt in.

2) Notes on correctness and design
- We did not add serde derives for Error, FatalError, InternalError, or ContextualError because they contain non-serializable sources (e.g., Arc<dyn Error>, std::io::Error) and recursion that needs careful design.
- MiettePolicy does not assume tracing; it prints a miette::Report to stderr. Applications can still use TracingPolicy if preferred, or combine policies externally.
- All additions are feature-gated and additive; no breaking changes.

3) Fixes and cleanup
- None required in this step.

4) Follow-ups
- Explore a serializable representation for InternalError/FatalError variants (e.g., flattening non-serializable sources into strings) under the serde feature.
- Provide an example in docs showing how to choose between TracingPolicy and MiettePolicy based on enabled features.
- Consider a CombinedPolicy that can delegate to multiple policies in order (e.g., tracing then miette).

Committed changes:
- Cargo.toml: wire "serde" feature to optional serde dep.
- severity.rs: add serde derives (feature-gated).
- domain.rs, warning.rs: add serde derives (feature-gated).
- policy.rs: add MiettePolicy behind "diagnostic".
- lib.rs: re-export MiettePolicy behind "diagnostic".
