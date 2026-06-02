# 2026-05-15 Shared Fixture Cache

Date: 2026-05-15
Task title: Shared fixture cache
Task description: Reduce friction from source corpus checkouts and generated fixture artifacts across multiple worktrees, Codex instances, and machines without weakening backup DB validation.
Related planning files:
- [Backup DB fixture registry](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)
- [Backup DB recreation guide](/home/brasides/code/ploke/docs/how-to/recreate-backup-db-fixtures.md)

## Intent

The fixture workflow has three distinct concepts that were previously tangled:

- Source corpus checkouts, such as `dtolnay__semver`.
- Generated backup DB artifacts, such as `corpus_semver_type_graph_2026-05-06.sqlite`.
- Repo-local test expectations under `tests/backup_dbs/`.

The operational goal is shared, idempotent setup across checkouts and agents. The correctness goal is unchanged: missing, stale, or schema-drifted backup fixtures should still fail loudly.

## Implemented Slice

- Runtime fixture loading now uses the shared DB snapshot fixture directory:
  `$XDG_CONFIG_HOME/ploke/db_snapshot_fixtures`, falling back to
  `~/.config/ploke/db_snapshot_fixtures`.
- `PLOKE_DB_SNAPSHOT_FIXTURE_DIR` can override the shared snapshot directory for
  tests or unusual local setups.
- `cargo xtask fixtures ensure --snapshots` ensures current active and typed
  graph snapshots exist in the shared snapshot directory. It validates active
  fixtures in the normal profile, invokes a typed-only xtask pass for typed
  graph fixtures, and stages committed seed backups only when a shared snapshot
  is missing.
- `cargo xtask fixtures regenerate --all` regenerates every automated active
  and typed graph fixture into the shared snapshot directory using the
  currently registered filenames. It runs active fixtures in the normal profile,
  invokes a typed-only xtask pass for typed graph fixtures, and skips manual
  legacy/orphaned snapshots.
- `cargo xtask recreate-backup-db --fixture <id>` writes regenerated outputs to
  the shared snapshot directory instead of `tests/backup_dbs/`.
- `cargo xtask recreate-backup-db --fixture <typed-corpus-id>` now prepares GitHub corpus sources through a shared cache root.
- `cargo xtask fixtures ensure --typed` prepares all registered typed GitHub corpus source checkouts and stages typed graph seed snapshots.
- The corpus source cache root is selected by `PLOKE_FIXTURE_HOME`, then
  `<db_snapshot_fixtures>/_source_cache`.
- Corpus sources are split into bare mirrors and per-revision checkouts:
  - `corpus/mirrors/<checkout_slug>.git`
  - `corpus/checkouts/<checkout_slug>/<rev>`
- Per-fixture lock files under `locks/` prevent concurrent agents from mutating the same mirror/checkout preparation path.

## Deferred

- The committed registry paths in `tests/backup_dbs/` are now seed paths, not
  the normal runtime load paths.
- `fresh_backup_fixture_db` and `shared_backup_fixture_db` load from the shared
  snapshot directory and fail if the expected registered filename is not staged.
- No backup import path was made permissive.
- The first strict staging smoke test exposed that
  `fixture_nodes_canonical_2026-05-06.sqlite` is missing `resolved_type_use`.
  That seed must be regenerated or explicitly migrated before
  `fixtures ensure --snapshots` can fully populate a clean shared directory.
- A targeted smoke regeneration of `fixture_nodes_canonical` into
  `/tmp/ploke-db-snapshot-fixtures-regen` produced and validated
  `fixture_nodes_canonical_2026-05-15.sqlite`, so the new shared-output
  recreation path works for that stale seed.
- `cargo xtask fixtures regenerate --all` was added and run successfully
  against `/home/brasides/.config/ploke/db_snapshot_fixtures`. It regenerated
  five active fixtures in the normal profile and four typed graph corpus
  fixtures in a typed-only sub-pass.

## Next Useful Slices

- Regenerate or explicitly migrate stale committed seed backups that fail
  strict staging validation.
- Add file locks around generated DB creation before allowing `fixtures ensure
  --typed` to build missing DB artifacts automatically.
- Add a cache key/fingerprint file next to generated DB snapshots if we need to
  support multiple schema/profile variants with the same logical fixture id.
- Refresh [BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md) before changing backup fixture registry entries or backup-dependent tests; its last review date is 2026-05-15.
