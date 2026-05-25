## esm-surfacegrant-coordinate-policy

Implementation worker report. Files changed:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`

Summary:

- Added `SurfacePolicyId`.
- Added `GrantAuthority` binding `loop_graph::Coordinate` and policy identity
  to a granted artifact.
- Added `Grant::for_coordinate(...)` and
  `Grant::for_coordinate_with_forbidden(...)`.
- Existing `Grant::new(...)`, `Grant::with_forbidden(...)`, and material
  `Grant::check(...)` semantics remain available for existing backend/TUI
  callsites.
- `Grant::narrow(...)` preserves bound authority.
- Added accessors `authority()`, `coordinate()`, and `policy()`.

Tests added:

- `coordinate_target_artifact_mismatch_is_rejected`
- `accepted_coordinate_grants_expose_coordinate_and_policy`

Verification:

```bash
cargo test -p ploke-eval coordinate_ -- --nocapture 2>&1 | tail -n 60
```

Result: passed locally; 3 tests passed, 0 failed.

Deferred:

- `EditableSurface::broad(...)` and legacy `Grant::new(...)` remain
  material-only constructors with no runtime/policy authority binding.
- Wiring real parent coordinates and policy IDs through parent/request
  admission requires edits outside this lane.
- `GrantAuthority` supports only `OperationTarget::Artifact` in this slice.
