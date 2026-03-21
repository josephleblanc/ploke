# Backup DBs: `ploke-tui` Loading Survey (2026-03-20)

Survey scope: backup DB loading in `crates/ploke-tui` test code and test helpers,
compared against the desired backup-fixture shape in
[2026-03-19_backup-dbs.md](/home/brasides/code/ploke/docs/active/todo/2026-03-19_backup-dbs.md):
registry-backed `FixtureDb` metadata plus shared immutable loading through
`shared_backup_fixture_db(...)` where tests only need read access.

## Conforming

- [new_test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs)
  loads the shared local-embedding fixture through
  `shared_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)`.
  This matches the intended shared immutable helper path.
- The following tests conform by consuming that helper rather than loading
  backups directly:
  - [apply_code_edit_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs)
  - [editing_bulk_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/editing_bulk_tests.rs)
  - [command_feedback_policy.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/command_feedback_policy.rs)
  - [tool_call_event_ordering.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/tool_call_event_ordering.rs)
  - [tool_ui_payload_fixture.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/tool_ui_payload_fixture.rs)
  - the first fixture-backed case in
    [get_code_edges_regression.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/get_code_edges_regression.rs)
- [get_code_edges_regression.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/get_code_edges_regression.rs)
  also conforms for the real-crate backup cases by using
  `shared_backup_fixture_db(&PLOKE_DB_PRIMARY)`.

## Non-Conforming

- [test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_harness.rs)
  still builds `TEST_APP` with:
  - an env override `PLOKE_TEST_DB_BACKUP`
  - a hard-coded canonical backup path
  - direct `db.import_from_backup(...)`
  This bypasses the shared `FixtureDb` registry and shared helper APIs.
- The current consumers of that non-conforming helper are:
  - [overlay_fixture_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/overlay_fixture_tests.rs)
  - [overlay_manager_smoke.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/overlay_manager_smoke.rs)
- [exec_real_tools_live_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs)
  defines its own `TEST_DB_NODES` with:
  - a hard-coded backup path
  - direct `db.import_from_backup(...)`
  - a crate-local `Arc<Mutex<Database>>`
  This also bypasses the registry/helper path.

## Strong Migration Candidates Now

- [overlay_manager_smoke.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/overlay_manager_smoke.rs)
  is a strong candidate. It uses `get_state()` from the old `TEST_APP` helper
  only to render overlays; it does not mutate DB contents.
- [overlay_fixture_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/overlay_fixture_tests.rs)
  is also a strong candidate. It mutates proposal state, not the underlying DB,
  so the backup DB usage is still effectively immutable.
- The main migration target is therefore the old helper itself in
  [test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_harness.rs):
  those overlay tests should move off its ad hoc backup import path and onto the
  registry-backed fixture path or the newer headless harness.

## Justified Exception

- [exec_real_tools_live_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs)
  is the only clear partial exception.
  It appears to be live-tool scaffolding rather than a pure immutable fixture
  consumer, and it wraps the DB in `Arc<Mutex<Database>>` for potentially
  stateful test flows.
  That makes it a weaker fit for the shared immutable helper right now.
  Even so, it should still be treated as non-conforming on path/config source:
  if touched later, it should at least source fixture metadata from the
  registry instead of hard-coding the backup filename.

## Extra Note

- [apply_code_edit_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs)
  has a stale prerequisite comment describing the canonical backup file, but
  the actual runtime path comes from
  [new_test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs).
  That is doc drift, not a loading-pattern regression.
