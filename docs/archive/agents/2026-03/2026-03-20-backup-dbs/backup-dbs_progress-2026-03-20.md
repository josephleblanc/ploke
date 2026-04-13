# Backup DBs Progress 2026-03-20

## Completed

- Added fixture inventory and usage tracking in
  [docs/testing/BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md).
- Added AGENTS guidance for backup fixture review cadence in
  [AGENTS.md](../../../../AGENTS.md).
- Added helper review findings in
  [docs/active/reports/test-helper-review.md](../../reports/test-helper-review.md).
- Added the shared fixture registry and helper API in
  [crates/test-utils/src/fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs).
- Migrated several immutable test consumers to the shared registry/helper path.
- Added `xtask` commands for:
  - `verify-backup-dbs`
  - `recreate-backup-db --fixture <id>`
  - `repair-backup-db-schema --fixture <id>`
- Added the operator guide in
  [docs/how-to/recreate-backup-db-fixtures.md](../../../how-to/recreate-backup-db-fixtures.md).

## Verified

- `cargo check -p xtask`
- `cargo check -p ploke-test-utils --tests`
- `cargo check -p ploke-db --tests`
- `cargo check -p ploke-rag --tests`
- `cargo check -p ploke-tui --tests`

Those checks passed in the last delegated validation pass.

## Current Fixture Status

- `cargo xtask recreate-backup-db --fixture fixture_nodes_canonical`
  succeeded and produced:
  - `tests/backup_dbs/fixture_nodes_canonical_2026-03-20.sqlite`
- `cargo xtask recreate-backup-db --fixture fixture_nodes_local_embeddings`
  now succeeds in-repo and produced:
  - `tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite`
  - strict validation also passed via
    `cargo xtask verify-backup-dbs --fixture fixture_nodes_local_embeddings`
- `cargo xtask recreate-backup-db --fixture ploke_db_primary`
  now recreates from source and produced:
  - `tests/backup_dbs/ploke_db_primary_2026-03-20.sqlite`
  - strict validation passed via
    `cargo xtask verify-backup-dbs --fixture ploke_db_primary`

## Known Remaining Gaps

- `ploke-db`, `ploke-rag`, and `ploke-tui` now pass against the refreshed
  canonical/local/ploke-db fixture paths.
- The local-embedding recreation path is stricter now, but not fully hermetic
  yet because the local model revision is still unpinned.
- Some fixture consumers still rely on dated backup path updates in docs/registry
  rather than deriving every path from a single command output.

## Immediate Next Step

- Treat the backup-fixture refresh path as functionally complete, then decide
  whether to pin the local model revision for fully hermetic local-embedding
  recreation.
