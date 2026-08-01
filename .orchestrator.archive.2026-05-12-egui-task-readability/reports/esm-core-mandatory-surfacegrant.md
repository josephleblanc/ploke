# Worker Report: Mandatory SurfaceGrant Authority

Task: `esm-core-mandatory-surfacegrant`

Changed files:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`

Summary:

- Split authority from material containment.
- Kept `GrantAuthority` mandatory for authority-bearing grants.
- Introduced `MaterialScope` as the non-authority containment adapter.
- Made `Grant` always authority-bearing.
- Made `Check` preserve grant authority.
- Updated owned tests to construct authority-bearing grants explicitly and to
  assert broad/request/narrow/check authority behavior.

Verification:

- `rustfmt --edition 2024 crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`
- `cargo test -p ploke-eval edit_surface 2>&1 | tail -n 60`
- `cargo test -p ploke-eval edit_surface 2>&1 | rg -n -C 2 "error\\[E|--> crates/ploke-eval/src/cli/prototype1_state/(backend|cli_facing|edit_surface/surface|edit_surface/tests)"`

Blockers:

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs` still has old
  `surface::Grant::new(...)` callsites.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` still has an old
  `surface::Grant::new(...)` callsite.
- `cli_facing.rs` has backend signature mismatches after the new admission API.
