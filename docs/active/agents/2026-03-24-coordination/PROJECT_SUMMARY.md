# Project summary: xtask commands feature

**Date:** 2026-03-25  
**Last aligned with codebase:** 2026-03-25  
**Current milestone:** M.3 — implement architecture foundation (partial)  
**Branch:** `feature/xtask-commands`  
**Progress tracker:** [progress.md](./progress.md)

---

## Executive summary

The project adds agent-oriented `xtask` commands for parsing, transform, ingest, database work, and (later) headless TUI and direct tools. **M.1** (surveys) and **M.2** (architecture) are complete. **M.3** has substantial **scaffolding** in [`xtask/`](../../../../xtask): clap-based `parse` and `db` subcommands, `CommandContext`, `Command` / executor infrastructure, `UsageTracker`, and integration tests. **Executable behavior** for the new agent commands is still mostly `todo!()`; [`xtask/src/main.rs`](../../../../xtask/src/main.rs) does not dispatch the new CLI. **M.4** will implement behavior, wire the binary, and close README gaps (e.g. `ploke_error::Error`, usage in the CLI path). **M.5** (README) / A.5–A.6 is planned in [design/m6-planning.md](./design/m6-planning.md) with entry [m5-planning.md](./m5-planning.md).

---

## Codebase truth (`xtask`)

Single source for doc claims about the crate (2026-03-25).

### Entrypoints

| Entry | Role |
|-------|------|
| [`main.rs`](../../../../xtask/src/main.rs) | **Production binary:** legacy string commands (`verify-fixtures`, `profile-ingest`, …). Does **not** call [`Cli::run()`](../../../../xtask/src/cli.rs). |
| [`cli::Cli`](../../../../xtask/src/cli.rs) | **Agent CLI (clap):** `parse`, `db`, `help-topic`. Compiled as part of the binary crate but not the default dispatch path. |
| [`xtask` library](../../../../xtask/src/lib.rs) | Public API for executor, context, tests. |

### Modules (high level)

| Path | Role |
|------|------|
| `lib.rs` | Exports `cli`, `commands`, `context`, `error`, `executor`, `usage`, `test_harness`. |
| `error.rs` | Local `XtaskError`, `RecoveryHint`, `ErrorCode` — **not** `ploke_error::Error` (README §D gap). |
| `context.rs` | `CommandContext`; persistent DB support path contains `todo!()`. |
| `executor.rs` | `Command` trait, `CommandExecutor`, `CommandRegistry`, `MaybeAsync`; async execution and some registry helpers `todo!()`. |
| `usage.rs` | `UsageTracker`, JSONL persistence, rolling-suggestion threshold — **not** invoked from `Cli::execute` (README §B gap). |
| `test_harness.rs` | `TestableCommand`, harness helpers; `FixtureBuilder::build` `todo!()`. |
| `commands/mod.rs` | `OutputFormat` (human/json/table/compact); table format returns error until M.4. |
| `commands/parse.rs` | Clap subcommands + `Command` impls; **all** `execute` bodies `todo!()`. |
| `commands/db.rs` | Clap subcommands + `Command` impls; **all** `execute` bodies `todo!()`. |
| `profile_ingest.rs` | Legacy profiling command used from `main`. |

**Not present as modules:** `commands/transform.rs`, `commands/ingest.rs`, `commands/tui.rs`, `commands/tool.rs`, `formatter/` tree (formatting is via `OutputFormat` in `commands/mod.rs`).

### `todo!()` hot spots (non-exhaustive)

- `commands/parse.rs`, `commands/db.rs`: every command `execute`.
- `context.rs`: persistent database branch.
- `executor.rs`: async block path; constructing commands from arguments.
- `test_harness.rs`: fixture builder build.
- `tests/parse_commands.rs`: many tests `todo!(… M.4)`.

---

## README adherence snapshot (sections A–G)

Statuses: **Met** | **Partial** | **Not started** | **N/A**

| Section | Theme | Status | Notes |
|---------|--------|--------|-------|
| **A.1** | Parsing commands | Partial | Clap + types exist; syn_parser not called yet; tracing on pipeline not done per task scope. |
| **A.2** | Transform | Not started | No `transform` subcommand module. |
| **A.3** | Ingest / embed | Not started | No `ingest` command; `TEST_OPENROUTER_API_KEY` not wired for a dedicated command. |
| **A.4** | Database | Partial | Clap surface mirrors survey; all stubs. |
| **A.5** | Headless TUI | Not started | Planned M.5 / [m6-planning.md](./design/m6-planning.md). |
| **A.6** | Direct tools | Not started | Same as A.5. |
| **B** | Docs, feedback, tracing log hints, usage | Partial | Help strings in `cli`; no 48h help staleness automation; `UsageTracker` unused in CLI path. |
| **C** | Per-command invariants, command enum | Partial | Clap enums satisfy “structured input”; behavioral invariants wait on M.4. |
| **D** | `ploke_error::Error` + recovery | Not started | Local `XtaskError` only. |
| **E** | Test matrix discipline | Partial | [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md) populated; proof text varies by file. |
| **F** | Out of scope | Met | No full REPL; wrappers only. |
| **G** | Per-crate module organization | Partial | `commands/parse.rs` and `commands/db.rs` under one crate; not separate `xtask-db` directories. |

---

## README spec errata (do not edit README per file banner)

Track these when interpreting the spec; coordinate with task owner if README should be corrected later.

| Issue | Location | Note |
|-------|-----------|------|
| M.1.9 / M.1.10 | README M.4 | Referenced milestone substeps are not defined in the M.1.1–M.1.5 list; treat as typo for “latest command list / test docs”. |
| Heading typo | README M.5 | “A.5-A-6” should be “A.5–A.6”. |
| Wrong path | README sub-agent guideline | `docs/agents/2026-03-24-coordingation/` — typo and wrong folder; real path is `docs/active/agents/2026-03-24-coordination/`. |
| M.5 vs M.6 | README M.3.1 vs M.5 | M.3.1 asks planning for “M.6” while milestone list uses **M.5** for TUI/tools. **Canonical milestone name for A.5–A.6: M.5.** Planning body file: [design/m6-planning.md](./design/m6-planning.md). |
| Test matrix path | README E.1 vs M.3.2 | E.1: coordination `2026-03-25-test_matrix.md`; M.3.2: `xtask/tests/test_matrix.md`. **Canonical:** coordination file; `xtask/tests/test_matrix.md` is a pointer. |

---

## What has been completed

### M.1 — Survey crates

Approved; surveys and cross-crate list documented. See [progress.md](./progress.md) and [sub-agents/](./sub-agents/).

### M.2 — Design architecture

Three proposals, consolidation, **Proposal 3** selected: [design/architecture-decision.md](./design/architecture-decision.md). Test design: [design/test-design-requirements.md](./design/test-design-requirements.md).

---

## What is in progress (M.3)

| Track | State |
|-------|--------|
| Foundation types / traits | Large parts landed; `OutputFormatter` as separate crate module not present — `OutputFormat` enum used instead. |
| `TestableCommand` | Present in [test_harness.rs](../../../../xtask/src/test_harness.rs). |
| Command stubs + tests | Present; strict TDD “fail until impl” not uniform (see test matrix). |
| M.5 planning | Draft complete: [m5-planning.md](./m5-planning.md) → [design/m6-planning.md](./design/m6-planning.md). |

---

## Key decisions (unchanged intent; codebase nuance)

- **Architecture:** Executor pattern + resources (Proposal 3) per [architecture-decision.md](./design/architecture-decision.md).
- **Errors:** Spec targets `ploke_error::Error`; **implementation today** is local `XtaskError` — track as M.4.
- **Tests:** Proof-oriented style where written; expand per [test-design-requirements.md](./design/test-design-requirements.md).

---

## Next steps

1. **M.4:** Implement `parse` / `db` (and add transform/ingest/pipeline as scoped), wire `main` to `Cli::run` (or agreed dispatch), integrate `UsageTracker` in the CLI path, adopt `ploke_error` per §D, add tracing on wrapped paths.
2. **M.5:** Execute [design/m6-planning.md](./design/m6-planning.md) for A.5–A.6.
3. **Docs:** Keep [command matrix](./2026-03-25-command-matrix.md) `xtask` impl column updated as commands ship.

---

## Future milestones (README-aligned)

| Milestone | Description | Status |
|-----------|-------------|--------|
| M.4 | Full implementation of A.1–A.4 commands + README B/D gaps + binary wiring | Not started |
| M.5 | ploke-tui headless + direct tools (A.5–A.6) | Not started (planning draft exists) |

---

## Risk assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Dual CLI confusion | Medium | Documented in progress + summary; single entrypoint in M.4. |
| Survey “done” vs impl | Medium | Command matrix now has **`xtask` impl** column. |
| `ploke_error` migration | Medium | Plan explicit conversions in M.4. |
| Async executor `todo!` | Medium | Tokio integration when first async command ships. |

---

## Open questions

1. **`TEST_OPENROUTER_API_KEY`:** Required for agent ingest command per README A.3; no dedicated xtask ingest command yet.
2. **Executor vs direct `cmd.execute`:** CLI bypasses `CommandExecutor`; decide whether M.4 unifies paths for usage/tracing.
3. **Persistent DB in `CommandContext`:** `todo!()` — behavior and tests in M.4.
4. **Table output format:** Returns error until implemented ([`OutputFormat::Table`](../../../../xtask/src/commands/mod.rs)).

---

## Documentation status

| Document | Status |
|----------|--------|
| [README.md](./README.md) | Spec (agent-edited per banner) |
| [progress.md](./progress.md) | Updated 2026-03-25 alignment pass |
| [TABLE_OF_CONTENTS.md](./TABLE_OF_CONTENTS.md) | Updated 2026-03-25 |
| [2026-03-25-command-matrix.md](./2026-03-25-command-matrix.md) | Survey + `xtask` impl columns |
| [2026-03-25-test_matrix.md](./2026-03-25-test_matrix.md) | Populated (seed) |
| [m5-planning.md](./m5-planning.md) | M.5 entry pointer |
| [design/m6-planning.md](./design/m6-planning.md) | M.5 body (filename historical) |

---

## References

- [README.md](./README.md) — sections A–G  
- [TASK_ADHERENCE_PROMPT.md](./TASK_ADHERENCE_PROMPT.md)  
- [sub-agents/cross-crate-commands.md](./sub-agents/cross-crate-commands.md)  
- [task_adherence/m1-review-report.md](./task_adherence/m1-review-report.md), [m2-review-report.md](./task_adherence/m2-review-report.md)

---

*Last updated: 2026-03-25 — coordination doc / codebase alignment pass.*
