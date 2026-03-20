# Shared Backup Helper Progress 2026-03-20

- Added executable fixture policy to
  [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs):
  - `fresh_backup_fixture_db(&FixtureDb)`
  - `shared_backup_fixture_db(&FixtureDb)`
  - import-mode, embedding, and primary-index enforcement now come from the
    fixture registry instead of crate-local setup code
- Switched immutable backup consumers to the shared helper:
  - [crates/ploke-db/src/bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs)
  - [crates/ploke-rag/src/core/unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs)
  - [crates/ploke-rag/tests/integration_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/tests/integration_tests.rs)
  - [crates/ploke-tui/src/test_utils/new_test_harness.rs](/home/brasides/code/ploke/crates/ploke-tui/src/test_utils/new_test_harness.rs)
- Updated fixture docs to point immutable callers at the shared helper and the
  registry-backed fixture IDs.

Risks / follow-up:

- `shared_backup_fixture_db` intentionally caches only successful immutable
  loads. If a fixture is missing or broken, callers retry on the next access
  instead of caching the failure.
- Mutable/local fixture loaders are still duplicated in a few places by design;
  they were not part of this pass.
- I did not run tests or `cargo check`, so the main remaining risk is compile
  cleanup for any newly unused imports or mismatched error conversions.
