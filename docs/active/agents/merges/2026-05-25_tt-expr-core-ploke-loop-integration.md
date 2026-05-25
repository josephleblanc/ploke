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

## 2026-05-25 Merge Start

Started `git merge --no-commit --no-ff FETCH_HEAD` after fetching
`/home/brasides/code/ploke feature/ploke-loop`.

Initial conflict set:

- `AGENTS.md`
- `Cargo.lock`
- `crates/ploke-db/src/bm25_index/mod.rs`
- `crates/ploke-db/src/database.rs`
- `crates/ploke-db/src/index/hnsw.rs`
- `crates/ploke-db/src/multi_embedding/db_ext.rs`
- `crates/ploke-db/src/multi_embedding/hnsw_ext.rs`
- `crates/ploke-db/src/utils/test_utils.rs`
- `crates/ploke-rag/src/context/mod.rs`
- `crates/ploke-rag/src/core/unit_tests.rs`
- `crates/ploke-tui/Cargo.toml`
- `crates/ploke-tui/src/app/commands/unit_tests/decision_tree.rs`
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
- `crates/ploke-tui/src/app_state/database.rs`
- `crates/ploke-tui/src/tools/request_code_context.rs`
- `crates/ploke-tui/src/user_config.rs`
- `crates/ploke-tui/tests/integration.rs`
- `crates/ploke-tui/tests/integration/eval_embedding_selection_live.rs`
- `crates/ploke-tui/tests/integration/post_apply_rescan.rs`
- `crates/ploke-tui/tests/integration/workspace_subset_remove.rs`
- `crates/test-utils/src/fixture_dbs.rs`
- `crates/test-utils/src/lib.rs`
- `docs/active/agents/readme.md`
- `docs/how-to/recreate-backup-db-fixtures.md`
- `docs/testing/BACKUP_DB_FIXTURES.md`
- `xtask/src/context.rs`
- `xtask/src/main.rs`
- `xtask/tests/command_acceptance_db.rs`

Resolution order: docs and backup fixture policy first, then fixture/xtask
code, then DB/RAG/TUI code.

## 2026-05-25 Fixture Path Resolution

Resolved the `crates/test-utils` and `xtask` fixture-path merge by preserving
three distinct fixture scopes:

- checkout-local fixtures use root-hashed files under `tests/backup_dbs/local/`
  for write paths
- shared-snapshot fixtures are reserved for source-pinned corpus DBs
- legacy fixtures keep their registered fallback path

Important implementation note: `FixtureDb::path()` remains a read candidate and
can point at the shared snapshot fallback when a checkout-local file does not
exist. New xtask write sites therefore use scope-aware output helpers instead
of writing to `fixture.path()` blindly. Bulk `fixtures ensure --snapshots` and
`fixtures regenerate --all` now avoid staging checkout-local DBs into the
shared snapshot directory.

## 2026-05-25 Conflict Resolution Checkpoint

All textual merge conflicts have been resolved and staged. The remaining work
is formatting, focused compile checks, fixture validation, and any follow-up
fixes those checks expose.

Resolution decisions made after the fixture-path block:

- `ploke-db` test/helper call sites now use `FixtureDb::checked_path()` instead
  of calling `backup_fixture_path_or_seed()` directly. This keeps fixture scope
  selection centralized and prevents call sites from accidentally treating a
  write candidate as a read fallback.
- `ploke-rag` keeps the advanced branch's
  `get_snippet_context_nodes_ordered()` path so sparse retrieval can assemble
  snippets without requiring dense embedding rows. The typed branch's
  type-context attachment is preserved by deriving `node_ids` from returned
  `EmbeddingData` rows.
- `ploke-tui` keeps `tool_contracts`, `demo`, and the typed graph feature
  wiring. Default features now include both `tool_contracts` and
  `typed_type_graph`.
- Workspace registry env handling now uses
  `PLOKE_WORKSPACE_REGISTRY_PATH_ENV` instead of repeating the raw env-var
  string in test code.
- TUI tests that sandbox process-global config now restore both
  `PLOKE_WORKSPACE_REGISTRY_PATH` and `PLOKE_DB_SNAPSHOT_FIXTURE_DIR`. Where a
  test may create snapshot/registry files, the fixture-dir env is pointed at
  that test's temp tree rather than the machine-shared snapshot fixture dir.
- Workspace subset integration tests keep the typed branch's
  `write_current_schema_workspace_snapshot()` helper. This imports the
  canonical workspace fixture through current strict schema logic and writes a
  fresh temp snapshot, instead of copying a potentially stale DB byte-for-byte.
- `Cargo.lock` was resolved in favor of the advanced branch's newer transitive
  versions for `rustls`, `socket2`, and `thiserror` in the `quinn` entries.

## 2026-05-25 Validation and Local Fixture Regeneration

Validation was run after resolving all textual conflicts.

Passed:

- `cargo fmt --all`
- `cargo check -p syn_parser`
- `cargo check -p xtask`
- `cargo check -p ploke-db --tests --features typed_type_graph`
- `cargo check -p ploke-tui --features typed_type_graph,test_harness`
- `cargo run -p xtask -- --help`

`cargo check -p xtask` initially exposed real typed-graph drift in the advanced
branch's `syn1` visitor: new node constructors were missing `where_predicates`
and trait `associated_type_bounds`, and old supertrait/impl-type paths still
used raw legacy type IDs. The fix was to route those `syn1` paths through the
typed type-use helpers and populate the new typed fields.

Strict backup fixture validation behaved as intended. The first
`cargo run -p xtask -- verify-backup-dbs` run rejected stale shared fallback
snapshots for checkout-local fixtures because they were missing the
`resolved_type_use` relation:

- `ploke_db_primary` from
  `/home/brasides/.config/ploke/db_snapshot_fixtures/ploke_db_primary_2026-05-06.sqlite`
- `ws_fixture_01_member_single` from
  `/home/brasides/.config/ploke/db_snapshot_fixtures/ws_fixture_01_member_single_2026-05-06.sqlite`

Those failures were not worked around by relaxing import behavior. Instead, the
checkout-local fixtures were regenerated under the current worktree root:

- `cargo run -p xtask -- recreate-backup-db --fixture ploke_db_primary`
  wrote `tests/backup_dbs/local/ploke_db_primary__root-fa2345b7bd52.sqlite`
- `cargo run -p xtask -- recreate-backup-db --fixture ws_fixture_01_member_single`
  wrote `tests/backup_dbs/local/ws_fixture_01_member_single__root-fa2345b7bd52.sqlite`

The final `cargo run -p xtask -- verify-backup-dbs` passed with those
checkout-local root-scoped DBs. `tests/backup_dbs/local/` remains ignored by
git, so these regenerated DBs are available to this checkout without
overwriting or staging machine-shared snapshots.

`git diff --cached --check` was also run. It reported whitespace warnings in
merged advanced-branch docs/generated/archive files. Those warnings were not
treated as an integration blocker and were not mass-cleaned in this merge,
because doing so would add unrelated churn across the large primary-branch
payload.
