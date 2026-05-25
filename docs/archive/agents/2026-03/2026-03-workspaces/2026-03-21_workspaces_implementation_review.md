# Workspace Implementation Review 2026-03-21

Backlinks:
- [2026-03-20_workspaces_progress_tracker.md](2026-03-20_workspaces_progress_tracker.md)
- [2026-03-20_workspaces_implementation_plan.md](../../reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](../../reports/2026-03-20_workspaces_acceptance_criteria.md)
- [2026-03-20_workspaces_test_witnesses.md](2026-03-20_workspaces_test_witnesses.md)

Use this as the shortest practical re-entry doc for the workspace rollout.

## Current state

- Readiness `R1-R4`: `done`
- Phases `C0-C6`: `done`
- Phase `C7`: `not started`
- Global obligations still tracked explicitly: `G1` coherent session state and `G2` explicit membership authority / manifest drift handling
- Last verified broad regression state: full `cargo test -- --nocapture` green after the `Bm25Cmd::Search.scope` compile fix, and `cargo test -p ploke-tui --tests -- --nocapture` green after the subset command work

## What changed

### Readiness and fixture baseline

- Added committed multi-member workspace fixture:
  [ws_fixture_01/Cargo.toml](../../../../tests/fixture_workspace/ws_fixture_01/Cargo.toml)
- Added canonical workspace backup fixture registration:
  [fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs)
- Added assertion-level `workspace_metadata` transform coverage:
  [workspace.rs](../../../../crates/ingest/ploke-transform/src/transform/workspace.rs)

Validation links:
- [lib.rs](../../../../crates/ingest/syn_parser/src/lib.rs)
- [workspace.rs](../../../../crates/ingest/syn_parser/src/discovery/workspace.rs)
- [fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs)

### `ploke-tui` workspace state and indexing

- Introduced explicit loaded-workspace state and member-scoped path policy:
  [core.rs](../../../../crates/ploke-tui/src/app_state/core.rs)
- Reworked indexing to resolve exact crate root first, otherwise nearest ancestor workspace:
  [parser.rs](../../../../crates/ploke-tui/src/app/commands/parser.rs)
  [indexing.rs](../../../../crates/ploke-tui/src/app_state/handlers/indexing.rs)
- Added workspace status/update behavior:
  [database.rs](../../../../crates/ploke-tui/src/app_state/database.rs)

Validation links:
- [index_workspace_targets.rs](../../../../crates/ploke-tui/tests/index_workspace_targets.rs)
- [load_db_crate_focus.rs](../../../../crates/ploke-tui/tests/load_db_crate_focus.rs)
- [workspace_status_update.rs](../../../../crates/ploke-tui/tests/workspace_status_update.rs)
- Bug report for the major `C2` regression that was fixed:
  [2026-03-21-indexworkspace-relative-target-regression.md](../../bugs/2026-03-21-indexworkspace-relative-target-regression.md)

### Save/load registry and retrieval scope

- Moved `/save db` and `/load` to exact workspace-registry identity:
  [database.rs](../../../../crates/ploke-tui/src/app_state/database.rs)
  [user_config.rs](../../../../crates/ploke-tui/src/user_config.rs)
- Added one shared retrieval-scope model across BM25, dense, hybrid, and context assembly:
  [workspace.rs](../../../../crates/ploke-core/src/workspace.rs)
  [bm25_service.rs](../../../../crates/ploke-db/src/bm25_index/bm25_service.rs)
  [hnsw_ext.rs](../../../../crates/ploke-db/src/multi_embedding/hnsw_ext.rs)
  [mod.rs](../../../../crates/ploke-rag/src/core/mod.rs)

Validation links:
- [unit_tests.rs](../../../../crates/ploke-rag/src/core/unit_tests.rs)
- [mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs)
- [database.rs](../../../../crates/ploke-tui/src/app_state/database.rs)

### Subset DB operations and runtime subset commands

- Added namespace inventory, remove, export, and import primitives:
  [database.rs](../../../../crates/ploke-db/src/database.rs)
- Added end-to-end workspace subset remove and subset import command paths in `ploke-tui`:
  [parser.rs](../../../../crates/ploke-tui/src/app/commands/parser.rs)
  [exec.rs](../../../../crates/ploke-tui/src/app/commands/exec.rs)
  [commands.rs](../../../../crates/ploke-tui/src/app_state/commands.rs)
  [dispatcher.rs](../../../../crates/ploke-tui/src/app_state/dispatcher.rs)
  [db.rs](../../../../crates/ploke-tui/src/app_state/handlers/db.rs)
  [database.rs](../../../../crates/ploke-tui/src/app_state/database.rs)

Validation links:
- [workspace_subset_remove.rs](../../../../crates/ploke-tui/tests/workspace_subset_remove.rs)
- [database.rs](../../../../crates/ploke-db/src/database.rs)
- [2026-03-21_c6_subset_db_design_notes.md](2026-03-21_c6_subset_db_design_notes.md)

## High-signal tests to inspect

- Fixture and ingestion:
  [lib.rs](../../../../crates/ingest/syn_parser/src/lib.rs#L745)
  [workspace.rs](../../../../crates/ingest/syn_parser/src/discovery/workspace.rs#L516)
  [workspace.rs](../../../../crates/ingest/ploke-transform/src/transform/workspace.rs#L170)
  [fixture_dbs.rs](../../../../crates/test-utils/src/fixture_dbs.rs#L441)
- TUI indexing/state:
  [index_workspace_targets.rs](../../../../crates/ploke-tui/tests/index_workspace_targets.rs)
  [workspace_status_update.rs](../../../../crates/ploke-tui/tests/workspace_status_update.rs)
- Save/load and retrieval scope:
  [database.rs](../../../../crates/ploke-tui/src/app_state/database.rs#L1649)
  [mod.rs](../../../../crates/ploke-db/src/bm25_index/mod.rs#L1006)
  [unit_tests.rs](../../../../crates/ploke-rag/src/core/unit_tests.rs)
- Subset operations:
  [database.rs](../../../../crates/ploke-db/src/database.rs#L3974)
  [workspace_subset_remove.rs](../../../../crates/ploke-tui/tests/workspace_subset_remove.rs)

## Commands you can run to sanity-check behavior

- `cargo test -p ploke-db --lib -- --nocapture`
- `cargo test -p ploke-rag -- --nocapture`
- `cargo test -p ploke-tui --tests -- --nocapture`
- `cargo test -- --nocapture`
- `cargo xtask verify-backup-dbs`

Manual TUI commands worth trying:
- `/index start`
- `/workspace status`
- `/workspace update`
- `/save db`
- `/load workspace <name-or-id>`
- `/workspace rm <crate-name-or-exact-root>`
- `/load crates <workspace-name-or-id> <crate-name-or-exact-root>`

## Recommended next step

Start Phase 8 `C7`: workspace-aware tools with strict edit safety.

The highest-value first pass is:
- audit current read/context/edit tool entrypoints
- make reads and retrieval workspace-aware by default
- keep edits explicit and crate/file-targeted
- update prompt/context text so the app describes loaded workspace state rather than implying one loaded crate

Use the progress tracker as the source of truth for status changes, and the witness doc as the source of truth for why a test counts as acceptance evidence.
