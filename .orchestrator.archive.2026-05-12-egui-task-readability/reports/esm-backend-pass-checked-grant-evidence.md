# Worker Report: Backend Checked Grant Evidence

Task: `esm-backend-pass-checked-grant-evidence`

Changed file:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`

Summary:

- `CheckedSurfaceEdit` now stores typed `CheckedSurface`.
- Backend derives target/base/after/patch identity from the checked carrier.
- `surface_evidence()` passes the full checked surface object into
  `SurfaceEvidence::checked(...)`.
- Persisted History evidence now receives checked coordinate, policy, and grant
  evidence instead of string-only policy recovery.
- Extended the bridge test to assert persisted checked grant evidence.

Verification:

- `cargo fmt --all`
- `cargo check -p ploke-eval 2>&1 | tail -n 120`
- `cargo test -p ploke-eval edit_surface_bridge_accepts_single_file_proposal 2>&1 | tail -n 60`
- `cargo check -p ploke-eval --tests --message-format=short 2>&1 | rg 'backend.rs|tui.rs|cli_facing.rs|error\\['`

Result:

- Backend/TUI compile errors did not surface before remaining `cli_facing.rs`
  blockers.

Blockers:

- `cli_facing.rs:1645` still needs `EditSurfaceAdmission`.
- `cli_facing.rs:11002` still uses the old `SurfaceEvidence::checked(...)`
  test/helper signature.
