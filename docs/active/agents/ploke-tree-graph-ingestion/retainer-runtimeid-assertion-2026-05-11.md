# Retainer RuntimeId Assertion Report - 2026-05-11

Task: `retainer-runtimeid-assertion-map`

Result: read-only report.

Findings:

- The exact assertion is in `crates/ploke-tree/src/lib.rs:1475-1484`, in
  `fs_run_store_loads_run_attempt_evidence`.
- `RuntimeId` is a transparent string wrapper in
  `crates/ploke-records/src/ids.rs:12`, with `Display` implemented at
  `ids.rs:23-26`, but no inherent `as_str()` method.
- The smallest valid fix surface is the `ploke-tree` test assertion only; no
  `ploke-records` API change is needed.
- Current workspace already has the equivalent tuple-field fix, and the
  targeted verification passes.

Verification reported by retainer:

- `cargo test -p ploke-tree fs_run_store_loads_run_attempt_evidence -- --nocapture 2>&1 | tail -n 40`

Changed files by retainer: none.
