# Progress Tracker: xtask Commands Feature

**Date:** 2026-03-25  
**Task:** xtask commands feature for agent-accessible diagnostics  
**Branch:** feature/xtask-commands  
**Doc/code alignment pass:** 2026-03-25 (see [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md) — codebase truth + PRIMARY_TASK_SPEC adherence)

---

## Current milestone: M.3 — Implement architecture foundation

**Summary:** Foundation modules, clap CLI (`parse` / `db` / `help-topic`), and integration tests exist under `xtask/`, but most command bodies are `todo!()`, the binary entrypoint does not dispatch the new CLI, and several PRIMARY_TASK_SPEC requirements (e.g. `ploke_error::Error`, usage wiring) are not yet met. See **Doc/code alignment** below.

### M.3.1 — Implement types + plan next steps

**Status:** In progress (partial)  
**Started:** 2026-03-25  
**Task adherence review:** Passed — proceed to M.3 (historical)

| Role | Status | Output / notes |
|------|--------|----------------|
| Bookkeeping | Complete | [TABLE_OF_CONTENTS.md](./TABLE_OF_CONTENTS.md), [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md) |
| Planning (M.5 / A.5–A.6) | Complete (draft) | [m5-planning.md](./m5-planning.md) → [design/m6-planning.md](./design/m6-planning.md) |
| Engineering — core | Partial | [error.rs](../../../../xtask/src/error.rs), [context.rs](../../../../xtask/src/context.rs): present; `ploke_error` not integrated per PRIMARY_TASK_SPEC §D |
| Engineering — executor / usage | Partial | [executor.rs](../../../../xtask/src/executor.rs), [usage.rs](../../../../xtask/src/usage.rs): present; async path and some registry paths still `todo!()`; CLI does not call `UsageTracker` |
| Engineering — commands / CLI | Partial | [commands/mod.rs](../../../../xtask/src/commands/mod.rs), [parse.rs](../../../../xtask/src/commands/parse.rs), [db.rs](../../../../xtask/src/commands/db.rs), [cli.rs](../../../../xtask/src/cli.rs): clap surface exists; execute bodies stubbed |
| Binary entry | Gap | [main.rs](../../../../xtask/src/main.rs): legacy string commands only; **does not** call `Cli::run()` |

### M.3.2 — Add TDD tests

**Status:** In progress (partial)  
**Started:** 2026-03-25

Integration tests exist under [`xtask/tests/`](../../../../xtask/tests/) (`context_tests`, `error_tests`, `executor_tests`, `parse_commands`, `db_commands`). Many scenarios are placeholders or expect M.4 implementation; this is **not** strict “tests fail until impl” TDD across the board.

**Canonical test matrix (PRIMARY_TASK_SPEC §E.1):** [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md).  
**M.3.2 mention in PRIMARY_TASK_SPEC** of `xtask/tests/test_matrix.md`: optional pointer only — see test matrix doc for resolution.

---

### Doc/code alignment (living notes)

| Topic | State |
|--------|--------|
| Dual CLI | Legacy `cargo xtask <string>` vs library `Cli` / clap subcommands; only legacy is wired in `main` |
| A.1–A.4 behavior | Stubs (`todo!()`) in `commands/parse.rs`, `commands/db.rs` |
| A.2 / A.3 CLI modules | Not present as `commands/transform.rs` or `commands/ingest.rs` |
| PRIMARY_TASK_SPEC §D `ploke_error::Error` | Not used in `xtask` today (`XtaskError` is local) |
| PRIMARY_TASK_SPEC §B usage / rolling suggestions | `UsageTracker` exists; not integrated in `cli::Cli::execute` |
| M.5 / A.5–A.6 | Planned in [design/m6-planning.md](./design/m6-planning.md); no `tui` / `tool` commands in crate yet |

---

## Completed: M.1 — Survey crates

### M.1.1 — Survey candidate commands

**Status:** Complete

| Crate | Output |
|-------|--------|
| syn_parser | [sub-agents/survey-syn_parser.md](./sub-agents/survey-syn_parser.md) |
| ploke_transform | [sub-agents/survey-ploke_transform.md](./sub-agents/survey-ploke_transform.md) |
| ploke_embed | [sub-agents/survey-ploke_embed.md](./sub-agents/survey-ploke_embed.md) |
| ploke_db | [sub-agents/survey-ploke_db.md](./sub-agents/survey-ploke_db.md) |
| ploke_tui | [sub-agents/survey-ploke_tui.md](./sub-agents/survey-ploke_tui.md) |
| ploke_test_utils | [sub-agents/survey-test-utils.md](./sub-agents/survey-test-utils.md) |

### M.1.2 — Key functions not in list

**Status:** Complete

### M.1.3 — Cross-crate commands

**Status:** Complete

### M.1.4 — Cross-crate function information

**Status:** Skipped (covered in M.1.3)

### M.1.5 — Final review

**Status:** Complete — proceed to M.2

---

## Completed: M.2 — Design architecture + documentation

### M.2.1 — Multi-agent review + design

**Status:** Complete

| Role | Output |
|------|--------|
| Architecture 1 | [design/architecture-proposal-1.md](./design/architecture-proposal-1.md) |
| Architecture 2 | [design/architecture-proposal-2.md](./design/architecture-proposal-2.md) |
| Architecture 3 | [design/architecture-proposal-3.md](./design/architecture-proposal-3.md) |
| Logical test design | [design/test-design-requirements.md](./design/test-design-requirements.md) |

### M.2.2 — Design consolidation

**Status:** Complete — **Proposal 3** (executor + resources) selected; see [design/architecture-decision.md](./design/architecture-decision.md)

---

## Upcoming milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| M.4 | Full implementation (replace stubs, wire `main` → `Cli`, tracing, PRIMARY_TASK_SPEC §B–§D gaps) | Not started |
| M.5 | Expand into `ploke-tui` (A.5–A.6); see [m5-planning.md](./m5-planning.md) | Not started (planning doc draft exists) |

---

## Blockers

None currently.

---

## Notes

- Branch `feature/xtask-commands` (per coordination history).
- Command matrix (survey vs `xtask` implementation): [2026-03-25-command-matrix.md](./2026-03-25-command-matrix.md).
- Task adherence: [TASK_ADHERENCE_PROMPT.md](./TASK_ADHERENCE_PROMPT.md).
