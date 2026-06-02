# Fixtures, Docs, Xtask

## Findings

1. **Medium** `crates/test-utils/src/fixture_dbs.rs:692-766` now routes `fresh_backup_fixture_db()` through `backup_fixture_path_or_seed()`, which silently falls back to committed seed DBs in `tests/backup_dbs/` when the shared snapshot dir is missing. That weakens the contract documented in `docs/testing/BACKUP_DB_FIXTURES.md:12-16,71-85` and conflicts with the handoff note in `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/2026-05-17_corpus-type-shape-matrix-handoff.md:141-146` to avoid permissive backup loading. Callers such as `xtask/tests/command_acceptance_db.rs:13-20` can now pass against repo seeds even when the configured snapshot dir is absent or stale. Keep the general helper strict, or make seed fallback an explicit opt-in.

## Open Questions

- Is the seed fallback required for isolated probe clones, or should those callers stage snapshots first?

## Test Gap

- No regression test covers the missing-shared-snapshot / present-seed case, so this behavior can still slip through.
