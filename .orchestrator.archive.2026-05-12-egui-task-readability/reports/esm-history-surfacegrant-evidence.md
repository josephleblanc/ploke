# Worker Report: History SurfaceGrant Evidence

Task: `esm-history-surfacegrant-evidence`

Changed file:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`

Summary:

- Added typed `SurfaceGrantEvidence` and `SurfaceCheckEvidence`.
- Added History-side binding and verification for grant authority when admitted
  candidate-artifact facts include artifact/runtime coordinates.
- Added focused tests for checked surface evidence.

Verification:

- `cargo fmt --all` succeeded.
- `cargo test -p ploke-eval surface_evidence_checked_persists_typed_check_evidence -- --exact 2>&1 | tail -n 40`

Result:

- Verification is blocked by backend/API compile errors outside the History
  lane.

Blockers:

- `CheckedSurfaceEdit::surface_evidence(...)` currently does not pass the
  original `GrantAuthority`/`SurfacePolicyId`, so History cannot preserve the
  checked SurfaceGrant losslessly until backend/check APIs pass it through.
- `backend.rs` still has old `surface::Grant::new(...)` callsites.
- `cli_facing.rs` still calls the old backend admission signature.
