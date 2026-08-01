# Worker Report: History Checked Grant API

Task: `esm-history-checked-grant-api`

Changed file:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`

Summary:

- `SurfaceEvidence::checked` now accepts a typed `CheckedSurface` carrier.
- Checked surface evidence no longer hardcodes `grant: None`.
- Checked grant authority is persisted immediately.
- Binding logic refines checked grant coordinates into admitted candidate
  coordinates during `bind_candidate_artifact`.

Verification:

- `cargo fmt --all`
- `cargo test -p ploke-eval surface_evidence_checked_persists_typed_check_evidence -- --exact 2>&1 | tail -n 40`
- `cargo check -p ploke-eval 2>&1 | rg -n "SurfaceEvidence::checked|history.rs:3065|backend.rs|cli_facing.rs|error\\[E0061\\]|error\\[E0277\\]" | tail -n 80`

Result:

- Verification is blocked by backend and cli_facing callsites that still use
  old API shapes.

Blockers:

- `backend.rs` must construct and pass the new typed `CheckedSurface` carrier.
- `cli_facing.rs` has test/helper use of the old `SurfaceEvidence::checked`
  signature.
- `cli_facing.rs:1645` still lacks live `EditSurfaceAdmission`.
