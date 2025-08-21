# Implementation Log 0007 — Matklad-informed IO strategy, iterator ergonomics, and plan addendum

1) Summary of changes with rationale
- Add FatalError::os_code() and FatalError::file_operation(...)
  Rationale: Following matklad’s guidance for std::io::Error, expose OS error codes programmatically without leaking std::io::Error in the public API and provide a consistent constructor for IO mapping.
- Add IterResultExt with collect_ok() and first_error()
  Rationale: Reduce boilerplate when handling Iterator<Item = Result<T, Error>> by providing eager collection with early error return and a lightweight probe for the first error.
- Update V3_PLAN.md with an addendum distilled from matklad’s blog
  Rationale: Capture durable design insights (IO handling, representation efficiency, API shape) and the USER caveats (monomorphization preference, no #[non_exhaustive], avoid forcing From impls).

2) Notes on correctness and design
- FatalError::os_code() is additive and does not change existing behavior. It allows runtime branching on OS conditions (e.g., EWOULDBLOCK) without downcasting.
- IterResultExt is eager for collect_ok() by design (matches standard Result<Vec<_>> collect behavior), and does not require lifetime-heavy iterator wrappers.
- Public API remains generic over &impl ErrorPolicy (monomorphization-friendly), aligning with USER preference.
- We intentionally did not add serde derives for FatalError due to the non-serializable std::io::Error field; a DTO approach may be introduced later behind a feature if needed.

3) Fixes and cleanup
- None required.

4) Follow-ups
- Consider adding try_for_each_ok(F) for side-effect processing that short-circuits on first error.
- Provide guidance on mapping std::io::ErrorKind to domain-specific decisions in downstream crates.
- Explore a telemetry DTO (feature-gated) for Fatal/Internal errors to support serde without leaking trait objects.
- Examples: add a small snippet demonstrating os_code()-based branching and IterResultExt usage.

Committed changes
- src/fatal.rs: add os_code() and file_operation(...) helpers.
- src/result_ext.rs: add IterResultExt with collect_ok() and first_error().
- src/lib.rs: re-export IterResultExt.
- docs/V3_PLAN.md: add matklad-inspired addendum and reflect USER caveats.
