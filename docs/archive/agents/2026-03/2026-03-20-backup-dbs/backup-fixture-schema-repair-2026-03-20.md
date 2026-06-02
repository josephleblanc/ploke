# Backup Fixture Schema Repair 2026-03-20

- Added `cargo xtask repair-backup-db-schema --fixture <id>` to explicitly
  repair stale backup fixtures that predate the `workspace_metadata` relation.
- The command is intentionally narrow:
  - restore the backup as-is into an empty DB
  - run the real `workspace_metadata` schema create script
  - write the repaired backup back to the registered fixture path
- Updated the operator docs and fixture inventory to point at this repair path
  when `verify-backup-dbs` fails on the missing `workspace_metadata` relation.
