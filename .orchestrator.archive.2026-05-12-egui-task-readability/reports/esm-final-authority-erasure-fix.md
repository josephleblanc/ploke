# Edit Surface Final Authority Erasure Fix

## Scope

- Removed the remaining authority-erasure shape from the TUI apply state: reported,
  applied, and rejected edit-surface apply states now carry
  `surface::GrantAuthority` directly.
- Kept `Grant` and `Check` authority-bearing: authority accessors return
  `&GrantAuthority`, not `Option`.
- Preserved checked surface grant authority through backend evidence into
  History using typed `CheckedSurface`.
- Made checked grant coordinates persist typed `RuntimeId` rather than a bare
  `String`.
- Fixed test assertions to compare checked runtime coordinates as `RuntimeId`.

## Invariant

The normal checked edit-surface path cannot construct or persist a checked grant
without coordinate and policy authority. Material containment remains separate
from authority-bearing grant/check evidence.

## Verification

- `cargo fmt --all`
- `cargo check -p ploke-eval 2>&1 | tail -n 100`
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 120`
- `rg -n "Option<[^>]*GrantAuthority|Option<surface::GrantAuthority|Grant::new\\(|from_results_with_authority|authority: None|grant: None|runtime_id: String|checked.*runtime_id.*String" crates/ploke-eval/src/cli/prototype1_state/edit_surface crates/ploke-eval/src/cli/prototype1_state/backend.rs crates/ploke-eval/src/cli/prototype1_state/history.rs crates/ploke-eval/src/cli/prototype1_state/parent.rs crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`

The final scan produced only unrelated `runtime_id: String` projection hits in
stream timing and sealed runtime evidence, not checked surface grant
coordinates.
