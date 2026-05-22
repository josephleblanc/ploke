# Repository Guidelines

## Project Structure & Module Organization
This is a Rust workspace. User-facing application code is in `crates/ploke-tui`, the workspace default member. Supporting crates live under `crates/`, including ingest/parsing crates in `crates/ingest/`, graph and records crates such as `crates/ploke-tree` and `crates/ploke-records`, and internal tooling in `crates/ploke-eval` and `crates/ploke-protocol`. Procedural macros are under `proc_macros/`. Integration fixtures, malformed crates, backup DBs, and workspace test inputs live under `tests/`, `fixtures/`, and `fixture_test_crate/`. Durable design notes and active work records are in `docs/`; static images are in `assets/`.

## Build, Test, and Development Commands
- `cargo build` builds the default member, currently `ploke-tui`.
- `cargo run -p ploke-tui` runs the terminal UI locally.
- `cargo check --workspace` checks all workspace crates quickly.
- `cargo test --workspace` runs the full workspace test suite.
- `cargo test -p ploke-tui` runs focused TUI tests.
- `cargo xtask verify-fixtures` verifies required local test assets before costly fixture-backed tests.
- `cargo fmt --all` formats all Rust code.
- `cargo clippy --all-targets -- -D warnings` runs lint checks when Clippy is available.

## Coding Style & Naming Conventions
Use Rust 2024 with the workspace Rust version from `Cargo.toml`. Follow standard Rust naming: `snake_case` for functions/modules, `PascalCase` for types and traits, and `SCREAMING_SNAKE_CASE` for constants. Prefer typed boundaries, structured errors, and focused modules that match existing crate layout. Run `cargo fmt --all` before handing off code.

## Testing Guidelines
Place unit tests beside the code they cover and integration tests under each crate’s `tests/` directory. Fixture-backed tests should declare their asset requirements through `xtask` when practical. Gate live provider or network tests behind existing feature flags such as `live_api_tests` or explicit ignored tests, and document required environment variables in the test or nearby README.

## Commit & Pull Request Guidelines
Recent history uses short imperative subjects and occasional Conventional Commit prefixes, for example `Fix headless TUI timeout-after-apply admission`, `test: add live google harness coverage`, and `docs: log workflow order incident`. Keep commits scoped and describe behavior, not just files. PRs should include a concise summary, linked issue or task when relevant, test evidence with exact commands, and screenshots or recordings for visible UI changes.

## Agent-Specific Instructions
Follow the most specific `AGENTS.md` in scope; for example, work inside `crates/ploke-tui` must also follow `crates/ploke-tui/AGENTS.md`. Do not edit generated fixtures, backup DBs, or active run artifacts unless the task explicitly asks for it.
