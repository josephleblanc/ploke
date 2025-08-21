# Error Handling Quality Criteria and Reasoning

Purpose
Define measurable criteria for error quality and explain the rationale behind the v3 design, reflecting on anti-patterns observed in:
- ploke-io/src/actor.rs
- ploke-io/src/write.rs
- ploke-rag/src/core/mod.rs
- ploke-tui/src/app/events.rs

A) Quality Criteria (5–7, actionable)
1) Boilerplate reduction and propagation ergonomics
   - Minimal explicit matches for error wrapping; favor `?`, `From`, and small extension traits.
   - Centralized conversions (From impls) instead of ad-hoc string mapping at call sites.
   - Target: ≥30% fewer lines in error-only code blocks after migration.

2) Performance and allocation discipline
   - Lazy/opt-in context capture (e.g., backtraces, snippets).
   - No default heavy features; diagnostic/tracing/serde/smartstring are opt-in.
   - Target: no regression in critical-path latency; fewer heap allocations for common flows.

3) Consistency of taxonomy and mapping
   - Single top-level Error with clear Severity classification.
   - DomainError for non-fatal, non-internal domain cases; Fatal/Internal reserved for integrity/bug classes.
   - Target: every cross-crate mapping specified; no ad-hoc string variants.

4) Composability and separation of concerns
   - Libraries return Result and do not emit globally; applications install an ErrorPolicy to emit/log/continue.
   - Emission and classification are policy-driven, not hard-coded in core logic.
   - Target: offenders move side-effects out of core modules into policy/application layer.

5) Observability and diagnostics
   - Optional miette-based diagnostics with stable SourceSpan labels.
   - Structured messages suitable for UI and logs; user-facing strings assembled at the edges.
   - Target: improved UX in TUI without contaminating library code paths.

6) Locality and readability of control flow
   - Prefer single-pass flows with `?` to nested matches and repeated mapping across async boundaries.
   - Encapsulate ordering/concurrency patterns (e.g., bounded joins) once in utilities.
   - Target: fewer custom loops/matches around join handles and channel responses.

B) Reflection on Current Anti-Patterns
- Repeated error mapping at spawn/join boundaries, manually re-ordering results and mapping panics.
- Event emission mixed into core IO/search logic, making it hard to reuse and test.
- Stringly-typed ad-hoc variants (UiError/TransformError) downstream, lacking structure.
- Eager context capture and tight coupling to proc_macro2::Span in runtime code.
- Inconsistent mapping decisions for similar errors across crates.

C) Alternative Designs Considered
1) Minimalist “thiserror everywhere + From impls + keep emitting inline”
   - Pros: Simple to grasp; zero new traits.
   - Cons: Emission still mixed with logic; boilerplate remains around spawn/join and UI messages.

2) Policy-first, diagnostics-last
   - Introduce ErrorPolicy with classify/emit; move all emission to app layer.
   - DomainError for structured non-fatal errors; libraries return Result and use `?`.
   - Pros: Major boilerplate reduction; strong separation of concerns; testability.
   - Cons: Requires small learning curve; adds a couple of traits.

3) Macro-heavy sugar (e.g., error!(), fatal!(), domain!(…))
   - Pros: Shortest call-sites.
   - Cons: Magic/control-flow opacity; harder to refactor and lint; risk of over-abstraction.

Decision (using criteria)
- Choose (2) Policy-first, diagnostics-last + structured DomainError.
  - Maximizes boilerplate reduction and composability (Criteria 1, 4, 6).
  - Preserves performance via lazy context and opt-in features (Criteria 2).
  - Improves taxonomy and observability (Criteria 3, 5) without macro magic.

D) How v3 Addresses Offenders
- actor.rs
  - Replace manual task result reordering with a utility that returns Vec<(idx, Result<_>)>; use `?` per file and From for mapping; push emission to the boundary via ResultExt::emit_event.
- write.rs
  - Normalize path and handle file ops with `?` and From; compute hashes and return Result; no inline emission; optional ContextExt.with_path for better diagnostics.
- rag core/mod.rs
  - Replace string variants with DomainError::Rag{…}; uniform timeout/channel mappings to Internal; strict vs fallback handled without recreating strings per call.
- tui events.rs
  - Render miette Diagnostics (when enabled) at the UI edge; map severity via policy; do not bake user strings into core error flows.

E) Measurement Plan
- Count LOC removed in error-handling/mapping blocks pre/post migration.
- Benchmark write/read/search hot paths with and without diagnostic feature.
- Trace consistency: validate all conversions go through standardized From impls.
- UX checks: confirm UI renders structured diagnostics without changing library code.

This reasoning and the criteria inform the v3 plan and guide migrations to a cleaner, faster, and more maintainable error system.
