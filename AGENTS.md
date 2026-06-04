# Repository Guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## Rust version
We are using rust version 2024 in all crates.

## Shared Agent Documents
- When the user asks you to create a new document, you should use the `docs/active/agents` directory, unless directed otherwise.
- Shared agent documents are in `docs/active/agents`
- See `docs/active/agents/readme.md` for naming conventions of files and directories, and further details.
- Current type-resolution handoff docs are indexed at `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/README.md`; check these before resuming `typed_type_graph` or import/type-resolution work on this branch.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

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
- Before changing backup fixtures or tests that depend on them, check [docs/testing/BACKUP_DB_FIXTURES.md](docs/testing/BACKUP_DB_FIXTURES.md) for the current registry, fixture consumers, and regeneration instructions.
- If the fixture review date in [docs/testing/BACKUP_DB_FIXTURES.md](docs/testing/BACKUP_DB_FIXTURES.md) is more than 7 days old, remind the user and ask whether they want to start a fixture review now before making more backup-fixture changes.

## Test Execution
- When running tests, use a sub-agent to execute the test command and report the output back to the main agent.
- Use follow-up sub-agent test runs for retries or narrowed repros when needed, so the main thread keeps only the summarized result and next action.

### Fail-until-impl (strict tests)
- Do not use tautological assertions (`is_ok() || is_err()`, match arms that accept both outcomes with no further checks).
- For behavior tests that require real output, do not add `Err` branches that pass on placeholder or "not yet implemented" messages; assert success with `expect`/`unwrap` on `Ok` and real invariants, or use intentional negative tests with `assert!(result.is_err())` plus concrete error expectations.
- Prefer exercising production entrypoints (`Command::execute`, executor paths) rather than failing only inside the test with `todo!()`.
- Until implementation exists, failure may be a panic from `todo!()` in the code under test or an `expect` on `Ok` that is not yet satisfied; do not paper over that with stub-tolerant matches.

## Project Structure & Module Organization
This is a Rust workspace. User-facing application code is in `crates/ploke-tui`, the workspace default member. Supporting crates live under `crates/`, including ingest/parsing crates in `crates/ingest/`, graph and records crates such as `crates/ploke-tree` and `crates/ploke-records`, and internal tooling in `crates/ploke-eval` and `crates/ploke-protocol`. Procedural macros are under `proc_macros/`. Integration fixtures, malformed crates, backup DBs, and workspace test inputs live under `tests/`, `fixtures/`, and `fixture_test_crate/`. Durable design notes and active work records are in `docs/`; static images are in `assets/`.

## Build, Test, and Development Commands
- `cargo build` builds the default member, currently `ploke-tui`.
- `cargo run -p ploke-tui` runs the terminal UI locally.
- `cargo check --workspace 2>&1 | rg -A 8 E0*` checks all workspace crates quickly.
- `cargo test --workspace 2>&1 | rg -A 8 E0` runs the full workspace test suite.
- `cargo test -p ploke-tui 2>&1 | rg -A 8 E0` runs focused TUI tests.
- `./target/debug/xtask verify-fixtures 2>&1 | rg -A 8 E0` verifies required local test assets before costly fixture-backed tests.
- `cargo fmt --all` formats all Rust code.
- `cargo clippy --all-targets -- -D warnings 2>&1 | tail -n 40`

## Coding Style & Naming Conventions
Use Rust 2024 with the workspace Rust version from `Cargo.toml`. Follow standard Rust naming: `snake_case` for functions/modules, `PascalCase` for types and traits, and `SCREAMING_SNAKE_CASE` for constants. Prefer typed boundaries, structured errors, and focused modules that match existing crate layout. Run `cargo fmt --all` before handing off code.

## Testing Guidelines
Place unit tests beside the code they cover and integration tests under each crate’s `tests/` directory. Fixture-backed tests should declare their asset requirements through `xtask` when practical. Live OpenRouter embedding/API coverage in the default workspace lane is intentional when the relevant default features are enabled; do not remove, ignore, or convert those tests to offline-only behavior merely for determinism. `OPENROUTER_API_KEY` is an expected local/CI prerequisite for the fast embedding service path. Google live tests are different: keep them explicit/ignored and use `cargo xtask auth google --strict-live` before claiming that path is validated. Document required environment variables in the test or nearby README.

## Commit & Pull Request Guidelines
Recent history uses short imperative subjects and occasional Conventional Commit prefixes, for example `Fix headless TUI timeout-after-apply admission`, `test: add live google harness coverage`, and `docs: log workflow order incident`. Keep commits scoped and describe behavior, not just files. PRs should include a concise summary, linked issue or task when relevant, test evidence with exact commands, and screenshots or recordings for visible UI changes.

## Agent-Specific Instructions
Follow the most specific `AGENTS.md` in scope; for example, work inside `crates/ploke-tui` must also follow `crates/ploke-tui/AGENTS.md`. Do not edit generated fixtures, backup DBs, or active run artifacts unless the task explicitly asks for it.

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **ploke** (51223 symbols, 92366 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/ploke/context` | Codebase overview, check index freshness |
| `gitnexus://repo/ploke/clusters` | All functional areas |
| `gitnexus://repo/ploke/processes` | All execution flows |
| `gitnexus://repo/ploke/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
