# Implementation Log 0008 — CombinedPolicy and deprecation of transitional variants

1) Summary of changes with rationale
- Added CombinedPolicy implementing ErrorPolicy composition.
  Rationale: Applications often want to both trace and render diagnostics (or delegate to multiple sinks). A small policy combinator allows composing multiple policies without pushing side-effects into library code, aligning with the policy-first design.
- Deprecated transitional top-level variants UiError and TransformError in Error.
  Rationale: V3 directs callers to structured DomainError variants. Deprecation guides migration without breaking existing crates immediately.

2) API details
- CombinedPolicy
  - Location: src/policy.rs; re-exported from lib.rs.
  - Shape:
    - pub struct CombinedPolicy { policies: Vec<Box<dyn ErrorPolicy>> }
    - new(), with_capacity(), from_vec(), push(), add_boxed()
    - ErrorPolicy impl:
      - classify: returns the maximum Severity among inner policies (Warning < Error < Fatal); defaults to error.severity() when empty.
      - emit: delegates to all inner policies in order.
  - Notes: Uses trait objects intentionally at the app layer; monomorphization remains for ResultExt methods that accept &impl ErrorPolicy.

- Deprecations
  - Error::UiError and Error::TransformError now carry #[deprecated] attributes with migration notes to use Error::Domain(DomainError::{Ui|Transform} { message }).

3) Correctness and design notes
- CombinedPolicy is additive; does not alter existing policy behavior. Empty CombinedPolicy preserves the default severity via Error::severity().
- Delegation order for emit is the insertion order. This allows, for example, tracing first then diagnostic rendering.
- No changes to feature gates; TracingPolicy still behind "tracing", MiettePolicy behind "diagnostic".
- Deprecations introduce warnings without breaking code; migration can proceed incrementally.

4) Follow-ups
- Optionally add builder-style APIs for CombinedPolicy if callers prefer &mut self chaining without moves.
- Consider a configurable classification strategy (e.g., first-wins) if a use-case emerges.
- Continue Phase 2: adopt DomainError across dependent crates and remove ad-hoc string variants.

Committed changes
- src/policy.rs: add CombinedPolicy with ErrorPolicy implementation.
- src/lib.rs: re-export CombinedPolicy; deprecate UiError and TransformError variants.
