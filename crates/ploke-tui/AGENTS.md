# Repository Guidelines

## Project Structure & Module Organization
- Workspace lives under `crates/`; this crate is `crates/ploke-tui`.
- Source code: `crates/ploke-tui/src/` (e.g., OpenRouter code in `src/llm/openrouter/`).
- Tests: `crates/ploke-tui/tests/` and module-adjacent `*_test.rs`.
- Docs and plans: `crates/ploke-tui/docs/` (plans in `docs/plans/agentic-system-plan/`, reports in `docs/reports/`).

## Build, Test, and Development Commands
- Build: `cargo build -p ploke-tui`
- Run TUI: `cargo run -p ploke-tui`
- Format: `cargo fmt --all` (CI checks with `-- --check`)
- Lint: `cargo clippy --all-targets -- -D warnings`
- Unit/integ tests (offline): `cargo test -p ploke-tui`
- E2E test harness (gated): `cargo test -p ploke-tui --features test_harness`
- Live API tests (require env + network): `cargo test -p ploke-tui --features live_api_tests -- --nocapture`
  - Record notable artifacts under `target/test-output/` and reference them in summaries.
- Quick pass/fail summary (quiet): `cargo test -p ploke-tui --lib 2>&1 | tail -n 20`
- Include warnings (verbose): `cargo test -p ploke-tui --lib`

## Coding Style & Naming Conventions
- Rust 2021; enforce `rustfmt` + `clippy`.
- Strong typing at boundaries: prefer enums/tagged unions; all OpenRouter structs derive `Serialize`/`Deserialize`; numeric fields use numeric types.
- Avoid stringly-typed JSON; validate early and surface actionable errors.
- Prefer static dispatch; minimize dynamic dispatch; use macros where they reduce boilerplate.
- Naming (Rust conventions): `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for consts.

## Testing Guidelines
- Place unit tests near code; integration tests in `tests/`.
- Gate live tests behind `live_api_tests`; do not report green unless the live path was exercised (tool calls observed, proposal staged/applied, file delta verified).
- For e2e, prefer `test_harness` feature; include brief pass/fail/ignored counts and artifact paths in PRs.

## Commit & Pull Request Guidelines
- Use Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`. Example: `feat(openrouter): add tool trait for request_more_context`.
- PRs include: clear description, linked issues, screenshots for UI changes, and evidence-based test summary (counts + `target/test-output/...` references). Avoid unrelated refactors.

## Security & Configuration Tips
- Never commit secrets. Provide `OPENROUTER_API_KEY` via env for live tests.
- Follow safety-first editing patterns (staged edits with verified file hashes via IoManager). Avoid writing outputs to docs by default; link artifacts instead.


## Serena MCP Memories
- Before starting work, load the `table_of_contents` memory via Serena MCP (Memories → read `table_of_contents`). It links to key docs, plans, and paths to explore next.
