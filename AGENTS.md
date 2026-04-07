# Repository Guidelines

## Rust version
We are using rust version 2024 in all crates.

## Shared Agent Documents
- When the user asks you to create a new document, you should use the `docs/active/agents` directory, unless directed otherwise.
- Shared agent documents are in `docs/active/agents`
- See `docs/active/agents/readme.md` for naming conventions of files and directories, and further details.

## Reading Logs
When the user asks you to "check the logs", "read the logs", "look into the logs", or similar:
- use `jq` for `.json` logs
  - make one initial query to see the shape, if the log structure is unfamiliar
  - in follow-up queries, prefer to limit output lines, and focus on the highest-signal log elements
- do not directly check/read/look into `.sqlite` logs or backup databases
- use `rg` for logs of other file types


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

### Fail-until-impl (strict tests)
- Do not use tautological assertions (`is_ok() || is_err()`, match arms that accept both outcomes with no further checks).
- For behavior tests that require real output, do not add `Err` branches that pass on placeholder or “not yet implemented” messages; assert success with `expect`/`unwrap` on `Ok` and real invariants, or use intentional negative tests with `assert!(result.is_err())` plus concrete error expectations.
- Prefer exercising production entrypoints (`Command::execute`, executor paths) rather than failing only inside the test with `todo!()`.
- Until implementation exists, failure may be a panic from `todo!()` in the code under test or an `expect` on `Ok` that is not yet satisfied; do not paper over that with stub-tolerant matches.
