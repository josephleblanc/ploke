# Worker Report: CLI Edit-Surface Admission

Task: `esm-cli-supply-edit-admission`

Changed file:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

Summary:

- Updated test/helper callsites to pass real `EditSurfaceAdmission`.
- Replaced remaining local `surface::Grant::new(...)` use with
  authority-bearing grant construction where real test authority exists.
- Refused to synthesize production admission authority from prompt/request/TUI
  state.

Verification:

- `rg -n "validate_edit_surface_candidate\\(|Grant::new\\(" crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `cargo check -p ploke-eval 2>&1 | tail -n 120`

Result:

- `cargo check` is still blocked by the production callsite at
  `cli_facing.rs:1645`, where real admission authority is not currently in the
  local call frame.

Blocker:

- The live deterministic production path has `ParentIdentity` in scope, but the
  worker reports it does not carry the required parent `runtime_id` and target
  `artifact_id` needed to build a real `Coordinate`.
