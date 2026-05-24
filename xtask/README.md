# xtask helper crate

The `xtask` pattern gives us a typed place to stash repo automation so every
developer/agent runs the same commands via `cargo xtask <command>`.

## Project status (paused work)

The original multi-command `xtask` expansion spec is currently **paused** after a partial implementation and a detour into real-world parser debugging. The canonical wrap-up/status document is:

- [`docs/active/agents/2026-03-26-xtask-wrapup-paused-work.md`](../docs/active/agents/2026-03-26-xtask-wrapup-paused-work.md)

## Available commands

- `cargo xtask setup-fixtures`
  - One-command setup path for ignored/generated local assets after a fresh
    clone.
  - Recreates missing or invalid active backup DB fixtures, stages the RAG
    fixture into the config-dir load path, clones pinned GitHub parser
    checkouts, materializes missing OpenRouter/Google model catalog fixtures,
    and finishes with `cargo xtask verify-fixtures`.
  - Network is only needed when the OpenRouter catalog, embedding model fixture,
    or GitHub checkout fixtures are absent or need refresh.
- `cargo xtask verify-fixtures`
  - Confirms required local assets exist before running costly tests.
  - Currently checks for:
    - `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
      (AppHarness + apply_code_edit tests).
    - `fixtures/openrouter/embeddings_models.json` (OpenRouter embeddings tests).
    - `crates/ploke-tui/data/models/all_raw.json` and derived model slices
      under `crates/ploke-tui/data/models/` used by `ploke-llm` model catalog
      tests. Generate these via `cargo xtask regen-model-catalog`.
    - `crates/ploke-tui/data/models/google_raw.json` and derived Google slices
      under `crates/ploke-tui/data/models/` used for direct Google route setup
      diagnostics. Generate these via `cargo xtask regen-google-model-catalog`.
    - `tests/fixture_github_clones/corpus/{serde,axum}`, pinned GitHub
      checkouts used by `syn_parser` workspace/config regression tests.
      Generate them via `cargo xtask setup-github-fixtures`.
- `cargo xtask verify-backup-dbs`
  - Validates the registered backup DB fixtures tracked in
    [docs/testing/BACKUP_DB_FIXTURES.md](../docs/testing/BACKUP_DB_FIXTURES.md).
  - By default checks the active fixtures used by tests.
  - Supports `--fixture <id>` to validate a single fixture.
  - Validation goes beyond file presence:
    - imports the backup using the registry-configured import mode
    - enforces embedding/index expectations via the shared fixture helper
    - saves a temporary roundtrip backup and re-imports it
- `cargo xtask recreate-backup-db --fixture <id>`
  - Recreates a registered backup fixture when that fixture has an automated
    regeneration path.
  - Writes a dated backup filename under `tests/backup_dbs/`.
  - If a fixture is not hermetically automatable yet, prints exact
    fixture-specific manual recreation steps instead of failing silently.
- `cargo xtask repair-backup-db-schema --fixture <id>`
  - Repairs a stale backup fixture in place when it predates the new
    `workspace_metadata` relation.
  - Restores the backup as-is, runs the real `workspace_metadata` schema create
    script, and writes the backup back to the registered fixture path.
  - This is intentionally narrow; it does not try to recreate the full schema on
    an existing backup.
- `cargo xtask setup-rag-fixtures`
  - Copies the canonical local `fixture_nodes` backup into the config-dir load path used by
    `ploke_db::multi_embedding::db_ext::load_db` (`$XDG_CONFIG_HOME/ploke/data` or
    `~/.config/ploke/data`).
  - Moves any other `fixture_nodes_*` backups in that directory into a quarantine folder so prefix
    matching cannot silently select a different embedding model during `ploke-rag` or TUI tests.
- `cargo xtask setup-github-fixtures`
  - Clones the ignored GitHub checkout fixtures required by parser tests.
  - Currently stages `serde-rs/serde` and `tokio-rs/axum` under
    `tests/fixture_github_clones/corpus/` at pinned commits, so fresh-clone
    setup does not drift with upstream repositories.
- `cargo xtask regen-embedding-models`
  - Fetches `https://openrouter.ai/api/v1/embeddings/models` and rewrites
    `fixtures/openrouter/embeddings_models.json`, updating the integrity metadata
    alongside it. Requires network access; no auth header is needed for this
    endpoint.
- `cargo xtask regen-model-catalog`
  - Fetches `https://openrouter.ai/api/v1/models` and rewrites the model catalog
    fixtures under `crates/ploke-tui/data/models/`, including raw, ids,
    architecture, top-provider, and pricing slices. Requires network access; no
    auth header is needed for this endpoint.
- `cargo xtask regen-google-model-catalog`
  - Materializes the direct Google model catalog currently exposed by
    `ploke-llm` into fixtures under `crates/ploke-tui/data/models/`, including
    raw, parsed registry rows, ids, architecture, top-provider, and pricing
    slices. This command does not hit a live Google model-list endpoint; the
    current direct Google integration intentionally models a local catalog.

If a file is missing the command prints a remediation hint and exits non-zero,
making it safe to gate test runs or CI hooks on this helper.

## Backup fixture registry

Backup DB lifecycle commands use the shared registry in
[crates/test-utils/src/fixture_dbs.rs](../crates/test-utils/src/fixture_dbs.rs).
That registry is the source of truth for:

- fixture ids
- backup paths
- import mode and embedding expectations
- regeneration strategy
- manual recreation instructions when automation is not hermetic yet
- the strict repair path for stale backups missing `workspace_metadata`

When a backup fixture changes, update the registry and
[docs/testing/BACKUP_DB_FIXTURES.md](../docs/testing/BACKUP_DB_FIXTURES.md)
together.

For the operator-facing workflow, see
[docs/how-to/recreate-backup-db-fixtures.md](../docs/how-to/recreate-backup-db-fixtures.md).

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
