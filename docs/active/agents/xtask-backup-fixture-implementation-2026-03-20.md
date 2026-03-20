# Xtask Backup Fixture Implementation 2026-03-20

- Added registry-backed `xtask` commands:
  - `cargo xtask verify-backup-dbs`
  - `cargo xtask recreate-backup-db --fixture <id>`
- Verification now checks active backup fixtures by default, imports them using
  their configured mode, saves a temporary roundtrip backup, reloads it, and
  re-validates the fixture contract.
- Recreation is mixed-mode by design:
  - `fixture_nodes_canonical` is automated as a current-schema snapshot
  - non-hermetic or repro fixtures print exact manual recreation steps
- Added the operator guide in
  [docs/how-to/recreate-backup-db-fixtures.md](/home/brasides/code/ploke/docs/how-to/recreate-backup-db-fixtures.md).
