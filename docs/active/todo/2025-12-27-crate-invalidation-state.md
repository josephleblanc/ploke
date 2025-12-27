# Todo for crate identity + coarse invalidation 2025-12-27

## Planning
- Implement stable CrateId + CrateInfo and WorkspaceRoots (crate lookup by id/path/namespace). (done)
- Add CrateFocus { crate_id } and migrate focus usage away from raw paths. (done)
- Add CrateVersionMap, Deps, and InvalidationSet to shared state.
- Derive PathPolicy from CrateFocus + extra_read_roots (no stored policy).
- Wire reindex flow to bump CrateVersionMap and mark dependents stale.
- Wire query flow to surface staleness (warn/deny/reindex policy).
- Add tests for invalidation propagation and policy derivation.

## Notes
- Step 1 implementation added `CrateId`/`CrateInfo`/`WorkspaceRoots` in `crates/ploke-core/src/workspace.rs` and rewired focus lookups in ploke-tui to use `focused_crate_root()`/`focused_crate_name()`.
- Tests still need updates that referenced raw `crate_focus` paths (see `crates/ploke-tui/tests/`).

## References
- ADR: docs/active/ADRs/005-crate-invalidation-state.md
