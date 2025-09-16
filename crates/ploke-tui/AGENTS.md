# Repository Guidelines

## Project Structure & Module Organization
- Source: `src/` (TUI, LLM/OpenRouter, app state, IO). Integration tests in `tests/`; unit tests colocated in modules.
- Benches: `benches/` (Criterion). Data/logs: `data/`, `logs/`, and `test-logs/`.
- Docs: `docs/` for plans and reports; see `docs/plans/agentic-system-plan/` and related OpenRouter/API notes.
- This crate depends on sibling workspace crates under `../` (e.g., `ploke-io`, `ploke-db`, `ploke-core`).

## Build, Test, and Development Commands
- Build: `cargo build -p ploke-tui --all-targets`.
- Run TUI: `cargo run -p ploke-tui`.
- Unit/Integration tests: `cargo test -p ploke-tui`.
- Live API tests: `cargo test -p ploke-tui --features live_api_tests` (requires `OPENROUTER_API_KEY`). Treat skips as not validated.
- Benches: `cargo bench -p ploke-tui`.
- Lint/format: `cargo fmt --all && cargo clippy --workspace -- -D warnings`.

## Coding Style & Naming Conventions
- Rust 2024, 4‑space indent, `rustfmt` required. Clippy warnings must be fixed.
- Strong typing at boundaries: use `Serialize`/`Deserialize` structs/enums; numeric fields as numeric types (e.g., `u32`, `f64`). Prefer enums/newtypes over ad‑hoc maps.
- Prefer static dispatch; minimize dynamic dispatch. Make invalid states unrepresentable.
- Safety-first editing: stage edits with verified file hashes; write via the `IoManager` atomically; never write on hash mismatch.

## Testing Guidelines
- Snapshots: use `insta`; update intentionally with `INSTA_UPDATE=auto cargo test`.
- Unit tests: small unit tests in files, module-level tests in a `<module>/tests/` directory
  - (new convention, may not be many tests like this yet)
- Write tests within `src` behind `#[cfg(test)]`, `#[test]`, and `#[tokio::test]`

## Commit & Pull Request Guidelines
- Commit style: Conventional Commits (e.g., `feat: ...`, `fix: ...`, `refactor: ...`, `test: ...`, `style: ...`).
- PRs must include: clear description, linked issues, test summary (counts + highlights), and screenshots for UI changes. Reference or update docs under `docs/` when behavior or contracts change.

## Security & Configuration Tips
- Configure env via `.env` or shell (e.g., `export OPENROUTER_API_KEY=...`).
- Do not log secrets. Validate external inputs early; treat loosely-typed JSON as errors at boundaries.
