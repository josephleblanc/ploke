# Backup DBs Uniform Helper Rule 2026-03-20

Based on the per-crate loader surveys:

- [backup-dbs_progress-2026-03-20-ploke-db-loader-survey.md](../backup-dbs_progress-2026-03-20-ploke-db-loader-survey.md)
- [ploke-rag-backup-loading-survey-2026-03-20.md](../ploke-rag-backup-loading-survey-2026-03-20.md)
- [backup-dbs_ploke-tui-loading-survey_2026-03-20.md](../backup-dbs_ploke-tui-loading-survey_2026-03-20.md)

the working rule for backup-backed tests is:

- If the test only reads fixture contents and does not require isolation, load
  the DB through `shared_backup_fixture_db(&FIXTURE_...)`.
- If the test needs a fresh mutable DB, explicit import-mode coverage, or
  isolation from shared state, keep a fresh loader or crate-local wrapper.
- `ploke-db` lib-unit tests remain the documented exception: they should prefer
  registry constants for fixture paths, but cannot directly use
  `shared_backup_fixture_db(...)` because of the duplicate-crate-type split.

Applied in this pass:

- `ploke-rag`
  - `dense_context_uses_multi_embedding_relations()` in
    [crates/ploke-rag/src/core/unit_tests.rs](../../../../crates/ploke-rag/src/core/unit_tests.rs)
    now uses the existing shared loader path.
- `ploke-tui`
  - the legacy `TEST_APP` loader in
    [crates/ploke-tui/src/test_harness.rs](../../../../crates/ploke-tui/src/test_harness.rs)
    no longer uses env override plus a hard-coded backup path.
  - it now uses a registry-backed crate-local one-time
    `fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)` static rather than
    `shared_backup_fixture_db(...)`, because the harness wires that DB into
    mutable runtime services.
  - current consumers:
    - [crates/ploke-tui/tests/overlay_fixture_tests.rs](../../../../crates/ploke-tui/tests/overlay_fixture_tests.rs)
    - [crates/ploke-tui/tests/overlay_manager_smoke.rs](../../../../crates/ploke-tui/tests/overlay_manager_smoke.rs)

Deferred for now:

- `ploke-db` mutable/per-test loaders and the documented unit-test exception
- `ploke-tui` live-tool scaffolding in
  [crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs](../../../../crates/ploke-tui/src/app/commands/exec_real_tools_live_tests.rs)
