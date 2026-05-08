# Testing Audit Note

- date: 2026-04-15
- task title: testing audit note
- task description: compact note capturing the current testing-surface findings for later user review, with emphasis on incomplete tests and backup fixture policy drift
- related planning files: `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`, `docs/testing/BACKUP_DB_FIXTURES.md`

## Highest-Signal Findings

1. Active observability test appears incomplete and should not be trusted as coverage.
   - [crates/ploke-db/tests/observability_tests.rs](/home/brasides/code/ploke/crates/ploke-db/tests/observability_tests.rs:108)
   - Active non-ignored test setup still contains `todo!()` placeholders for `model` and `provider_slug`.
   - Tracked separately in [2026-04-15-observability-test-todo-panic.md](/home/brasides/code/ploke/docs/active/bugs/2026-04-15-observability-test-todo-panic.md).
2. Backup fixture registry and documentation have drifted.
   - [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md:79)
   - [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs:230)
   - The docs still reference `ploke_db_primary_2026-03-21.sqlite`, while the active registry points at `ploke_db_primary_2026-03-22.sqlite`.
3. Older fixture guidance is still being taught in some docs and helpers.
   - [docs/testing/TEST_GUIDELINES.md](/home/brasides/code/ploke/docs/testing/TEST_GUIDELINES.md:128)
   - [crates/ploke-rag/docs/FIXTURE_CONFIGURATION.md](/home/brasides/code/ploke/crates/ploke-rag/docs/FIXTURE_CONFIGURATION.md:5)
   - Both still expose older fixture paths/assumptions.
4. Several test and benchmark helpers still hard-code a legacy backup fixture path instead of using the active registry-backed fixture.
   - [crates/ploke-db/src/index/hnsw.rs](/home/brasides/code/ploke/crates/ploke-db/src/index/hnsw.rs:285)
   - [crates/ploke-db/src/multi_embedding/hnsw_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs:1244)
   - [crates/ploke-db/src/utils/test_utils.rs](/home/brasides/code/ploke/crates/ploke-db/src/utils/test_utils.rs:14)
   - [crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs:346)
   - [crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs:14)
   - [crates/ploke-db/benches/resolver_bench.rs](/home/brasides/code/ploke/crates/ploke-db/benches/resolver_bench.rs:11)

## Additional Note

- [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md:1) is beyond its 7-day review window for fixture review as of 2026-04-15.

## Recommended Follow-Up

- run a focused fixture review before any more backup-fixture changes
- decide whether the hard-coded legacy fixture references are intentional regression anchors or cleanup candidates
- treat backup-fixture guidance drift as a documentation-and-test-policy issue, not just a search/replace task
