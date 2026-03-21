# ploke-tui Workspace Phase Survey

Related plan: [docs/active/reports/2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)

## Snapshot
`ploke-tui` is still crate-focused. The parser/help surface only knows `model load`/`model save` and the legacy executor still routes `index start`, `load crate`, `save db`, and `update` as single-target flows, with `/index start` defaulting to `.` if no path is passed. See [`parser.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/parser.rs#L23), [`commands/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/mod.rs#L18), and [`exec.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs#L852).

## Current State
- `StateCommand` already has the plumbing for `IndexWorkspace`, `SaveDb`, `LoadDb`, `ScanForChange`, and `RecordIndexCompleted`, and the dispatcher handles them asynchronously. See [`commands.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/commands.rs#L118) and [`dispatcher.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/dispatcher.rs#L144).
- `index_workspace` resolves one target root, sets one crate focus, optionally calls crate-only parsing, then hands off to the DB-wide indexer. See [`indexing.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L36).
- `SystemStatus` stores `workspace_roots`, one `crate_focus`, per-crate versions, dependency invalidation, and parse timestamps; `derive_path_policy` is still derived from the focused crate root only. See [`core.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L383) and [`core.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500).
- `scan_for_change` is focus-only today: it errors without a focus, reads one crate’s files from the DB, and emits a single `ReIndex` event. See [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L499).

## Backup Path
- `save_db` writes one snapshot under the config-dir `ploke/data` path, keys it from `focused_crate_name`, persists active embedding-set metadata first, and then calls `backup_db`. See [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L90) and [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L169).
- `load_db` finds one backup by prefix, imports the whole backup, restores the active embedding set, recreates the HNSW index, and then resets focus plus IO roots from `crate_context.root_path`. See [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L234), [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L283), and [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L407).
- Backup fixtures are strict and schema-coupled; the fixture docs require explicit regeneration or repair when schema drift appears, not permissive import. See [`docs/how-to/recreate-backup-db-fixtures.md`](/home/brasides/code/ploke/docs/how-to/recreate-backup-db-fixtures.md#L22) and [`docs/how-to/recreate-backup-db-fixtures.md`](/home/brasides/code/ploke/docs/how-to/recreate-backup-db-fixtures.md#L43).

## Existing Coverage
- `load_db_restores_saved_embedding_set_and_index` proves a backup roundtrip can restore a non-default embedding set, re-create the HNSW relation, and restore crate focus from backup metadata. See [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L999).
- `crate_focus_assigns_absolute_root_from_db` proves DB-stored `root_path` is treated as absolute and not rejoined with the current directory. See [`load_db_crate_focus.rs`](/home/brasides/code/ploke/crates/ploke-tui/tests/load_db_crate_focus.rs#L11).
- `index_start_does_not_block_state_manager` and `indexing_completed_event_does_not_block_input_when_system_read_held` prove the index command is dispatched off-thread and does not stall the app loop. See [`indexing_non_blocking.rs`](/home/brasides/code/ploke/crates/ploke-tui/tests/indexing_non_blocking.rs#L20) and [`indexing_freeze_app_loop.rs`](/home/brasides/code/ploke/crates/ploke-tui/tests/indexing_freeze_app_loop.rs#L194).
- `test_update_embed` exercises the current change-detection path for one focused crate and confirms changed nodes are retracted and reindexed after `scan_for_change`. See [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1476) and [`database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1973).
- `conversation_only_prompt_and_persistent_tip_without_workspace` proves the current no-workspace behavior is a fallback tip, not a workspace registry. See [`no_workspace_fallback.rs`](/home/brasides/code/ploke/crates/ploke-tui/tests/no_workspace_fallback.rs#L28).

## Disprovable Acceptance Criteria
- `/index` must resolve a workspace manifest and index every member crate; bare `/index` inside a workspace must use the workspace root, not `current_dir()`. It fails if only one crate is indexed, if a non-workspace directory is silently accepted, or if IO roots still point at the previous crate.
- `/save db` must create a restorable workspace snapshot plus a workspace registry entry, and it must preserve active embedding-set metadata. It fails if the snapshot exists without registry state, if the registry points at a missing snapshot, or if reload loses the active set.
- `/load <workspace>` must resolve by workspace identity, not by prefix-only backup lookup, and it must restore workspace state, focus, and IO roots atomically. It fails if the DB contents load but the TUI still points at the old root or if the wrong backup is chosen because names share a prefix.
- `/workspace status` must list every loaded crate and its stale state. It fails if it only reports the focused crate, if it omits a loaded crate, or if it marks a fresh crate stale.
- `/workspace update` must converge stale crates back to a fresh state without silently dropping unchanged embeddings or leaving stale rows behind. It fails if changed crates are not reembedded, if untouched crates lose embeddings, or if stale markers remain after a successful run.
- `/workspace rm <crate>` must only remove a crate that is already loaded and must not leave dangling focus or IO roots. It fails if it removes an unrelated crate, if it partially removes data while the UI still claims the crate is loaded, or if it silently ignores a missing target.

## Failure States To Catch
- Silent fallback to `current_dir()` for workspace discovery.
- Save/load success that does not also update `SystemStatus`, `crate_focus`, and `IoManager` roots.
- Prefix collision selecting the wrong backup file.
- Multi-crate update that leaves stale rows or stale versions behind.
- Removal that widens IO access or leaves focus on a deleted crate.
- Backup import that tolerates missing `workspace_metadata` instead of forcing regeneration or repair.

## Gaps
- There is no direct coverage yet for workspace registry persistence, workspace status output, or workspace removal semantics.
- There is no test yet that exercises duplicate crate-path/name conflict handling for save/load/update/remove.
- There is no test yet that proves multi-crate stale detection or per-crate status reporting.
