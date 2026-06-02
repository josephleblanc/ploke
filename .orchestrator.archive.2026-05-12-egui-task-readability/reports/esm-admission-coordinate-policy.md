# Worker Report: Admission Coordinate/Policy

Task: `esm-admission-coordinate-policy`

Changed files:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`

Summary:

- Added `EditSurfaceAdmission { coordinate, policy }` to the backend admission
  path.
- `validate_edit_surface_candidate` now requires explicit admission authority.
- The checked base artifact is bound to the coordinate target artifact id.
- Backend constructs grants with `surface::Grant::for_coordinate(...)`.
- `CheckedSurfaceEdit` carries the admitted coordinate/policy.
- TUI apply now has `Apply::from_results_with_authority(...)` and retains
  authority through reported/applied states.

Verification:

- `cargo test -p ploke-eval edit_surface_bridge_accepts_single_file_proposal 2>&1 | tail -n 40`
- `cargo test -p ploke-eval edit_surface_bridge_accepts_single_file_proposal 2>&1 | rg -n "error\\[|validate_edit_surface_candidate|Grant::new|cli_facing.rs:1645|cli_facing.rs:9968|backend.rs:998"`
- `cargo check -p ploke-eval 2>&1 | tail -n 120`

Result:

- Verification is blocked by upstream callsites still using the old API.

Blockers:

- `cli_facing.rs` still calls `validate_edit_surface_candidate` without
  `EditSurfaceAdmission`.
- `cli_facing.rs` still has a test/helper call to removed
  `surface::Grant::new(...)`.
- Full grant persistence requires backend/History integration so
  `SurfaceEvidence::checked(...)` receives the original grant evidence instead
  of hardcoding `grant: None`.
