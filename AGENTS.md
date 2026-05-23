# Repository Guidelines

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

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
Place unit tests beside the code they cover and integration tests under each crate’s `tests/` directory. Fixture-backed tests should declare their asset requirements through `xtask` when practical. Gate live provider or network tests behind existing feature flags such as `live_api_tests` or explicit ignored tests, and document required environment variables in the test or nearby README.

## Commit & Pull Request Guidelines
Recent history uses short imperative subjects and occasional Conventional Commit prefixes, for example `Fix headless TUI timeout-after-apply admission`, `test: add live google harness coverage`, and `docs: log workflow order incident`. Keep commits scoped and describe behavior, not just files. PRs should include a concise summary, linked issue or task when relevant, test evidence with exact commands, and screenshots or recordings for visible UI changes.

## Agent-Specific Instructions
Follow the most specific `AGENTS.md` in scope; for example, work inside `crates/ploke-tui` must also follow `crates/ploke-tui/AGENTS.md`. Do not edit generated fixtures, backup DBs, or active run artifacts unless the task explicitly asks for it.
