# Build and Test

Run commands from the repository root unless a crate-specific guide says otherwise.

## Build the default member

```bash
cargo build
```

The default member is `crates/ploke-tui`.

## Run the TUI

```bash
cargo run -p ploke-tui
```

## Check the workspace

```bash
cargo check --workspace
```

## Format

```bash
cargo fmt --all
```

## Focused TUI tests

```bash
cargo test -p ploke-tui
```

## Fixture checks

Before costly fixture-backed runs:

```bash
cargo xtask verify-fixtures
```

Backup fixture databases under `tests/backup_dbs/` are schema-coupled fixtures. Regenerate or migrate them deliberately when schemas change; do not silently loosen import semantics to hide schema drift.

## Live provider tests

Live API tests require the relevant provider environment variables in the test process, such as `OPENROUTER_API_KEY` for OpenRouter routes. Gate and report live tests separately from offline checks.
