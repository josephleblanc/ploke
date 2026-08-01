# Quick Start

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


## Prerequisites

- Rust toolchain capable of the workspace package rust-version (`1.85`) and Rust 2024 crates. See [`Cargo.toml`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/Cargo.toml) and crate manifests such as [`crates/ploke-tui/Cargo.toml`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/Cargo.toml).
- `cargo` on `PATH`.
- Optional provider credentials for live LLM/embedding workflows. The TUI loads config from `~/.config/ploke/config.toml` and environment variables via [`try_main`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-tui/src/lib.rs).
- Optional local fixture assets for tests and RAG snapshots. Use `cargo xtask verify-fixtures` or `cargo xtask setup-fixtures`; details are in [`xtask/README.md`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/xtask/README.md).

## Build

From the repository root:

```bash
cargo build
```

The workspace default member is `crates/ploke-tui`, whose binary is named `ploke`.

## First Run

```bash
cargo run -p ploke-tui
```

Or install the release binary for the current user:

```bash
./install.sh
```

The installer builds release mode, copies `target/release/ploke` into `${INSTALL_DIR:-$HOME/.local/bin}`, and prints a PATH reminder if needed.

## Common Workflows

### Run the TUI in a Rust workspace

```bash
cargo run -p ploke-tui
```

Inside the app, use insert/normal/command modes. The existing TUI README notes `i` for insert mode, `Esc` for normal mode, `:` for command mode, and `/help` for commands.

### Prepare fixture-backed tests

```bash
cargo xtask verify-fixtures
cargo xtask setup-fixtures
```

Use `verify-fixtures` before costly fixture-backed tests; use `setup-fixtures` on a fresh clone or when local model/catalog/checkouts are missing.

### Run focused checks

```bash
cargo check --workspace
cargo test -p ploke-tui
cargo fmt --all
```

For full test runs, expect fixture and live-provider requirements to matter. Live API tests are feature-gated or environment-dependent.

### Regenerate model catalog fixtures

```bash
cargo xtask regen-model-catalog
cargo xtask regen-google-model-catalog
cargo xtask regen-embedding-models
```

The OpenRouter catalog commands require network access. The Google catalog command materializes the local direct-Google registry modeled by `ploke-llm`.

## Configuration

- `~/.config/ploke/config.toml` — user configuration loaded by `ploke-tui::try_main` through the `config` crate.
- Environment variables — loaded with `_` separators through `config::Environment` and `.env` via `dotenvy`.
- `OPENROUTER_API_KEY` — needed for live OpenRouter chat/embedding routes in normal runs/tests that hit the provider.
- `PLOKE_IO_FD_LIMIT` — optional override for `ploke-io` actor concurrency, clamped by the I/O actor.
- `PLOKE_PROTOCOL_DEBUG` — enables JSON debug lines for `ploke-llm` chat HTTP and `ploke-protocol` procedure debug surfaces.
- `PLOKE_DB_SNAPSHOT_FIXTURE_DIR` — optional override for shared DB snapshot fixture staging; see [`xtask/README.md`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/xtask/README.md).

## Where to Go Next

- Architecture: [Architecture Overview](./architecture/index.md)
- Module reference: [Crate Guide](./crate-guide/index.md#imported-crate-pages)
- TUI runtime: [ploke-tui](./crate-guide/ploke-tui.md)
- Ingestion pipeline: [syn_parser](./crate-guide/syn-parser.md) and [ploke-transform](./crate-guide/ploke-transform.md)
- Retrieval pipeline: [ploke-db](./crate-guide/ploke-db.md), [ploke-embed](./crate-guide/ploke-embed.md), and [ploke-rag](./crate-guide/ploke-rag.md)
