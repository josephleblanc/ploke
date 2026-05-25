# 2026-05-25 TT Expr Core / Ploke Loop Integration

Date: 2026-05-25

Task title: Merge `tt-expr-core` with `/home/brasides/code/ploke` `feature/ploke-loop`

Task description: Use this checkout as the integration workspace, preserving the typed type-resolution work while merging in the advanced primary branch from `/home/brasides/code/ploke`.

Related planning files:

- [`docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/README.md`](../2026-05-10_tt-expr-core_type-resolution-handoff/README.md)
- [`docs/active/agents/2026-05-15_tt-expr-core_fixture-cache/README.md`](../2026-05-15_tt-expr-core_fixture-cache/README.md)
- [`docs/testing/BACKUP_DB_FIXTURES.md`](../../../testing/BACKUP_DB_FIXTURES.md)
- [`docs/how-to/recreate-backup-db-fixtures.md`](../../../how-to/recreate-backup-db-fixtures.md)

## Current Setup

- Integration checkout: `/home/brasides/code/agent-dir/ploke`
- Integration branch: `integration-tt-expr-core-with-ploke-loop-20260525`
- Payload starting point: `tt-expr-core` at `c55f9ef6`
- Primary source to merge: `/home/brasides/code/ploke` branch `feature/ploke-loop` at `4d9fb849`
- Important constraint: `/home/brasides/code/ploke` has unrelated uncommitted work and should not be used as the conflict-resolution workspace.

## Non-Negotiable Merge Rules

- Preserve the typed type-resolution work from `tt-expr-core`.
- Keep `PLOKE_WORKSPACE_REGISTRY_PATH` support from the advanced branch.
- Keep registry/config env serialization around workspace registry tests.
- Do not weaken backup fixture validation to make stale DBs pass.
- If backup fixture schema drift appears, regenerate fixtures or add explicit migration tooling.
- Do not make backup import silently tolerate missing relations, extra relations, root mismatches, or schema drift.

## Backup Fixture Path Rule

This rule is the main failure mode from the previous integration attempt.

Backup DB files may contain absolute source paths. A DB generated from
`/home/brasides/code/ploke` can be structurally valid but semantically wrong
when loaded from `/home/brasides/code/agent-dir/ploke` or an integration
worktree. That means path-sensitive DB snapshots are not safely shareable
across worktrees just because the schema imports.

Use these scopes deliberately:

- `FixturePathScope::CheckoutLocal`
  - for DBs whose rows may encode the current checkout root
  - output must be root-scoped under `tests/backup_dbs/local/<stem>__root-<hash>.sqlite`
  - bulk xtask commands must not stage or regenerate these into `~/.config/ploke/db_snapshot_fixtures`
- `FixturePathScope::SharedSnapshot`
  - for source-pinned corpus snapshots where the fixture contract is intended to be shared
  - may use `PLOKE_DB_SNAPSHOT_FIXTURE_DIR` / `~/.config/ploke/db_snapshot_fixtures`
- `_source_cache`
  - stores pinned source mirrors/checkouts, not generated DB rows
  - sharing is acceptable because identity is repo URL plus commit, not the current workspace root

Do not use `fixture.path()` blindly as an output path in xtask code. For
checkout-local fixtures, `fixture.path()` can fall back to a shared snapshot
candidate when the local root-scoped file does not exist. Write paths must be
scope-aware.

## Known Fixture State From Previous Attempt

The old integration worktree was deleted, but the previous merge attempt found:

- `fixture_nodes_canonical` regenerated successfully as checkout-local.
- `ploke_db_primary` regenerated successfully as checkout-local.
- `ws_fixture_01_canonical` regenerated successfully as checkout-local.
- `ws_fixture_01_member_single` regenerated successfully as checkout-local.
- `fixture_nodes_local_embeddings` failed regeneration because local embedding recreation left `7` non-file nodes unembedded.

Treat the local-embedding failure as a real invariant failure. Diagnose which
nodes remain unembedded and why. Do not relax the fixture contract without
explicit user approval.

## Expected Merge Flow

1. Fetch `/home/brasides/code/ploke` `feature/ploke-loop` into this checkout.
2. Start a no-commit merge from that fetched ref.
3. Resolve conflicts using the rules above.
4. Run focused compile checks before broader validation.
5. Regenerate only checkout-local path-sensitive fixtures locally.
6. Run strict backup fixture validation.
7. Commit once the merge is coherent and validation is clean.

## Validation Plan

- `cargo fmt --all`
- `cargo check -p xtask`
- `cargo check -p ploke-db --tests --features typed_type_graph`
- `cargo check -p ploke-tui --features typed_type_graph,test_harness`
- `cargo xtask verify-backup-dbs`

## Open Watchpoints

- `ploke-db` lib-unit tests may need crate-local DB loading to avoid duplicate-crate `Database` type splits through `ploke-test-utils`.
- TUI tests that sandbox `XDG_CONFIG_HOME` or workspace registry state must preserve `PLOKE_DB_SNAPSHOT_FIXTURE_DIR`.
- The merge should preserve `tool_contracts` defaults while adding typed graph features.
- The local-embedding fixture failure may indicate drift between what the indexer embeds and what the DB counts as pending embeddable non-file nodes.
