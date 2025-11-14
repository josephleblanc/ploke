# xtask helper crate

The `xtask` pattern gives us a typed place to stash repo automation so every
developer/agent runs the same commands via `cargo xtask <command>`.

## Available commands

- `cargo xtask verify-fixtures`
  - Confirms required local assets exist before running costly tests.
  - Currently checks for:
    - `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
      (AppHarness + apply_code_edit tests).
    - `crates/ploke-tui/data/models/all_pricing_parsed.json`
      (pricing tests that parse OpenRouter data). Generate this via
      `./scripts/openrouter_pricing_sync.py`.

If a file is missing the command prints a remediation hint and exits non-zero,
making it safe to gate test runs or CI hooks on this helper.

## Extending `verify-fixtures`

1. Open `xtask/src/main.rs`.
2. Append another entry to the `FIXTURE_CHECKS` array with:
   - A stable `id`, the workspace-relative `rel_path`, and a short
     `description`.
   - A concrete `remediation` string (copy/download/regenerate instructions).
3. Re-run `cargo xtask verify-fixtures` to confirm your new check behaves as
   expected.

Keeping everything in the `FIXTURE_CHECKS` table intentionally avoids
command-line flags or config files; extending the helper is just another code
change reviewed alongside the tests that rely on the asset.
