# Repository Guidelines

## Correctness Guardrails
- Do not relax internal correctness, consistency, validation, schema, or import semantics without explicit user approval first.
- If a possible fix would make the system more permissive, tolerate previously invalid states, silently skip expected data, or weaken invariants, stop and ask before implementing it.
- When presenting such a proposal, describe the tradeoff plainly: what invariant would be weakened, what failures would stop surfacing, and what safer alternatives exist.

## Backup Fixtures
- Treat backup fixture databases under `tests/backup_dbs/` as schema-coupled fixtures, not as long-term compatibility targets by default.
- When schema changes add, remove, or rename stored relations, prefer regenerating backup fixtures or adding an explicit migration path rather than loosening import behavior.
- Do not make backup import paths silently tolerate missing relations, extra relations, or schema drift unless the user explicitly approves that change.
- If tests fail because a backup fixture predates the current schema, first propose regenerating the fixture backups and only propose permissive loading or migration tooling as explicit alternatives.
- Before changing backup fixtures or tests that depend on them, check [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md) for the current registry, fixture consumers, and regeneration instructions.
- If the fixture review date in [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md) is more than 7 days old, remind the user and ask whether they want to start a fixture review now before making more backup-fixture changes.

## Test Execution
- When running tests, use a sub-agent to execute the test command and report the output back to the main agent.
- Use follow-up sub-agent test runs for retries or narrowed repros when needed, so the main thread keeps only the summarized result and next action.
