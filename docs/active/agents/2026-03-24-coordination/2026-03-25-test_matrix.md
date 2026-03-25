# Test matrix for `xtask` commands

**Date:** 2026-03-25  
**Task spec:** [PRIMARY_TASK_SPEC.md](./PRIMARY_TASK_SPEC.md) section E  
**Branch:** `feature/xtask-commands`

## Canonical location

Per PRIMARY_TASK_SPEC §E.1, this file under `docs/active/agents/2026-03-24-coordination/` is the **primary** test matrix for coordination and agent workflow.

The PRIMARY_TASK_SPEC milestone M.3.2 also mentions `xtask/tests/test_matrix.md`. That path holds a **short pointer** to this document only (no duplicate matrix). If tooling is added under `xtask/tests/`, extend this file and link the new tests here.

---

## Scope and status summary

| Test file | PRIMARY_TASK_SPEC areas | Role | Current state |
|-----------|----------------|------|---------------|
| [error_tests.rs](../../../../xtask/tests/error_tests.rs) | D (error shape), C invariants indirectly | Unit/integration-style tests for `XtaskError`, `RecoveryHint`, `ErrorCode` | **Runs:** exercises real error types; not yet `ploke_error::Error` |
| [context_tests.rs](../../../../xtask/tests/context_tests.rs) | A.1–A.4 (resource prep), architecture | `CommandContext` lifecycle, workspace root, lazy resources | **Runs:** concrete resource assertions; `context_persistent_database_panics_until_implemented` documents `todo!()` on persistent path |
| [executor_tests.rs](../../../../xtask/tests/executor_tests.rs) | Architecture (M.2), C | `CommandExecutor`, `CommandRegistry`, `MaybeAsync`, sync commands | **Runs / partial:** registry construction from args and async `block()` still `todo!()` in executor |
| [parse_commands.rs](../../../../xtask/tests/parse_commands.rs) | A.1, E | Parse subcommands: discovery, phases-merge, workspace, stats, list-modules | **Fail-until-impl:** calls `execute` + `expect` on success; fails until `commands/parse.rs` is implemented |
| [db_commands.rs](../../../../xtask/tests/db_commands.rs) | A.4, E | DB subcommands: save, load, fixtures, indexes, query, stats | **Fail-until-impl:** success-oriented tests use `expect` on `Ok` only; negative tests assert `Err` with concrete checks |

**Design reference:** proof-oriented conditions and command-level expectations: [design/test-design-requirements.md](./design/test-design-requirements.md).

---

## Per-file notes (PRIMARY_TASK_SPEC E.1 style)

### `error_tests.rs`

- **Underlying targets:** [`xtask::error`](../../../../xtask/src/error.rs) (`XtaskError`, `RecoveryHint`, `ErrorCode`).
- **Expected functionality:** stable error construction, display, conversion helpers, recovery hints.
- **Invariants:** error variants round-trip for tests that assert matching.
- **Fail states:** IO / validation paths return predictable `XtaskError` shapes.
- **Edge cases:** nested sources, `CommandFailed` formatting.
- **Gap vs PRIMARY_TASK_SPEC §D:** no `ploke_error::Error` integration yet — document as M.4 work.

### `context_tests.rs`

- **Underlying targets:** [`CommandContext`](../../../../xtask/src/context.rs).
- **Expected functionality:** temp dir, workspace discovery, cached `Arc` resources where implemented.
- **Invariants:** repeated `workspace_root()` stable; `database_pool()` same `Arc` when initialized.
- **Fail states:** missing workspace; `get_database` with persistent path (currently `todo!()`).
- **Edge cases:** default context vs `new()`.
- **Related tests elsewhere:** DB commands once context provides real DB handles.

### `executor_tests.rs`

- **Underlying targets:** [`executor.rs`](../../../../xtask/src/executor.rs) (`CommandExecutor`, `CommandRegistry`, `MaybeAsync`).
- **Expected functionality:** register commands, execute sync paths, usage hooks when wired.
- **Invariants:** executor config honored; sync `MaybeAsync::Ready` executes.
- **Fail states:** duplicate registration; async path without runtime (still `todo!()`).
- **Gap:** CLI does not use executor for `parse`/`db` today — integration is M.4.

### `parse_commands.rs`

- **Underlying targets:** [`commands/parse.rs`](../../../../xtask/src/commands/parse.rs), syn_parser APIs from survey docs.
- **Expected functionality:** each subcommand calls the surveyed function and returns serializable diagnostics.
- **Invariants:** invalid paths → structured `XtaskError` with recovery text (PRIMARY_TASK_SPEC B/D).
- **Fail states:** parse errors from `syn_parser` surfaced to stdout/stderr per spec.
- **Current state:** tests invoke `Command::execute` and `expect` success where appropriate; failures come from stub `todo!()` or unmet assertions until M.4.

### `db_commands.rs`

- **Underlying targets:** [`commands/db.rs`](../../../../xtask/src/commands/db.rs), `ploke_db`, `ploke_test_utils` fixtures.
- **Expected functionality:** backup/restore, fixture load, indexes, queries, stats per [survey-ploke_db.md](./sub-agents/survey-ploke_db.md).
- **Invariants:** cozo errors forwarded (PRIMARY_TASK_SPEC B.1); fixture paths validated.
- **Fail states:** missing backup file; invalid query; schema mismatch.
- **Current state:** success paths use `expect` on `Ok` only (no stub-tolerant `Err` branches); until M.4, `todo!()` in commands panics during execute.

---

## Hypothesis template (for new rows)

When adding tests, keep PRIMARY_TASK_SPEC E.3 discipline:

1. **To prove:** …  
2. **Why useful:** …  
3. **When this would not prove correctness:** …  

---

## Updates log

| Date | Change |
|------|--------|
| 2026-03-25 | Initial population; canonical path + pointer to `xtask/tests/test_matrix.md` |
| 2026-03-25 | Matrix updated for fail-until-impl policy (`parse_commands`, `db_commands`) |
