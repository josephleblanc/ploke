# Backup DBs Next Attempt

Use this note before resuming work on
[2026-03-19_backup-dbs.md](../../todo/2026-03-19_backup-dbs.md).

## What To Preserve

- Keep the shared fixture registry in
  [crates/test-utils/src/fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs)
  as the single source of truth for backup fixture metadata.
- Keep immutable fixture consumers on the shared helper path:
  - `shared_backup_fixture_db(...)`
  - `fresh_backup_fixture_db(...)`
- Keep `xtask` strict:
  - validate fixtures by importing them the way tests do
  - re-check the fixture contract after a save/reload roundtrip
  - do not add permissive import behavior to make stale fixtures pass

## What To Avoid

- Do not substitute schema-repair or migration work for the explicit task of
  creating or recreating the expected fixtures.
- Do not broaden the task by pursuing the shortest path to green tests if that
  changes the requested shape of the solution.
- Do not leave recreated dated backups disconnected from the tests that still
  point at older fixture paths without documenting that gap immediately.

## Better Orchestration

- Keep one hard phase boundary at a time:
  1. registry/helpers
  2. `xtask` lifecycle commands
  3. actual fixture recreation
  4. crate test validation
- After each phase:
  - write one short progress doc in `docs/active/agents/`
  - run the smallest relevant validation
  - summarize only the actionable delta
- Use sub-agents more narrowly:
  - one for exploration
  - one for editing
  - one for review
  - one for test execution
  - then stop and synthesize

## Remaining Priority

- Recreate the fixtures that tests actually load, not only dated alternates.
- Convert any remaining immutable tests that still rebuild from a backup path to
  the shared cached helper when mutation is not required.
- Re-run fixture-consuming crate tests only after the expected fixture files are
  current.
