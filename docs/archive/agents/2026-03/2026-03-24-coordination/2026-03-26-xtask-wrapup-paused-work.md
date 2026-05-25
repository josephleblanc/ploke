---
date: 2026-03-26
task_title: "xtask commands spec wrap-up (paused)"
task_description: >
  We started implementing the PRIMARY_TASK_SPEC `xtask` command surface, then
  shifted priorities after running against a real-world parse target and
  expanding `parse debug` tooling. This document captures what is kept, what is
  paused, and what is trimmed so the current state remains explainable and safe.
related_planning_files:
  - docs/active/agents/2026-03-24-coordination/PRIMARY_TASK_SPEC.md
  - docs/active/agents/2026-03-24-coordination/2026-03-25-test_matrix.md
---
## Intent
Wrap up partial implementation of the `xtask` commands spec without leaving behind:
- **Silent dead code** (types/modules that look important but are unused),
- **Landmines** (CLI-exposed commands that panic via `todo!()`),
- **Orphaned tests** (gap-signal tests that only prove a panic with no documented rationale).
The guiding policy for this wrap-up is:
- **Functional → keep** (even if it expanded beyond the original spec).
- **Non-functional but clearly in spec → feature-gate** (compile-time) and document.
- **Non-functional and ambiguous/not in spec → trim**.
## What changed vs the original spec
The original plan in `docs/active/agents/2026-03-24-coordination/PRIMARY_TASK_SPEC.md` targeted a broad agent-facing command surface (parse, transform, ingest, DB ops, and eventually headless TUI tooling).
After a partial implementation, we used `xtask` against a complex real-world parsing target and were pulled into **parser failure investigation**, which drove an expansion of:
- **`parse debug …`**: structured diagnostics intended to make discovery/resolve/merge failures actionable.
This “debug tooling first” trajectory is valuable and is kept, but it means the original spec is **paused** rather than fully implemented.
## Canonical test matrix
The canonical coordination test matrix is:
- `docs/active/agents/2026-03-24-coordination/2026-03-25-test_matrix.md`
`xtask/tests/test_matrix.md` is intentionally only a pointer to that file (no duplicated matrix).
## Current `xtask` surface: keep vs paused vs trim
### Keep (functional; covered by tests and/or already used)
**Parse commands (including debug tooling)**
- `xtask/src/commands/parse.rs`
- `xtask/src/commands/parse_debug.rs`
Primary acceptance coverage is tracked in the canonical matrix, including:
- `xtask/tests/parse_commands.rs`
- `xtask/tests/command_acceptance_parse.rs`
**Database commands that currently execute (non-panicking)**
- `xtask/src/commands/db.rs` (subset)
- `xtask/tests/db_commands.rs`
- `xtask/tests/command_acceptance_db.rs` (for `list-relations`, `embedding-status`)
**Fixture / backup DB lifecycle helpers**
These are used to keep fixture workflows reproducible and safe and are documented in:
- `xtask/README.md`
- `docs/testing/BACKUP_DB_FIXTURES.md`
### Paused (non-functional but clearly in spec; must be gated)
These are **in spec** (PRIMARY_TASK_SPEC A.4 / M.4 infra) but currently contain `todo!()` paths that can panic if exposed by default.
**DB index commands**
- `db hnsw-build`
- `db hnsw-rebuild`
- `db bm25-rebuild`
Implementation stubs live in:
- `xtask/src/commands/db.rs` (command bodies are `todo!()` today)
Related tests are currently “gap-signal” (prove a panic), e.g.:
- `xtask/tests/command_acceptance_db.rs` (the `*_panics_until_implemented` tests)
**Registry / arg-to-command construction infra**
`xtask/src/executor.rs` contains a `CommandRegistry` factory stub (`todo!()`) and a `MaybeAsync::block` pending-branch stub (`todo!()`).
Related tests include a gap-signal panic test:
- `xtask/tests/executor_tests.rs`: `registry_factory_panics_until_command_construction_implemented`
**How to re-enable paused items**
Paused items are intended to be available only when explicitly enabled, via:
- `cargo test -p xtask --features xtask_unstable`
- `cargo xtask --features xtask_unstable …`
### Trim (non-functional and ambiguous / not in spec)
Any stubs that are not required for current functional commands/tests and are not clearly on the critical path for resuming the spec should be trimmed to reduce conceptual weight.
Candidates to evaluate during wrap-up include:
- `xtask/src/test_harness.rs`: `FixtureBuilder::build` (`todo!()`), if unused.
If trimmed, record the decision here with the commit summary / rationale.
## Resume notes (next concrete steps when priorities return)
When resuming the original spec, the “next steps” are:
1. Implement DB index subcommands (`hnsw-build`, `hnsw-rebuild`, `bm25-rebuild`) and replace any gap-signal tests with behavior-gated acceptance tests on a real fixture DB.
2. Decide whether the registry/factory approach in `xtask/src/executor.rs` remains desirable given the current clap-driven CLI (`xtask/src/cli.rs`). If it stays, implement arg construction and remove the gap-signal panic tests.
3. Re-open PRIMARY_TASK_SPEC milestones A.2/A.3 (transform/ingest) and add new commands + tests as per the matrix “Planned tests” table.