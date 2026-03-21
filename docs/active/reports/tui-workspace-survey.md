# ploke-tui Workspace Survey

Related reports: none in this pass; this survey was completed directly.

## Summary
`ploke-tui` is still fundamentally crate-scoped. `/index start`, `/load crate`, and `/save db` all assume one focused crate, while the LLM/tooling layer disables code tools when no crate is focused. The current code can restore one backup and reindex one target, but it does not yet model a workspace as a set of crates that can be saved, loaded, updated, and queried together.

## Current State
- Command parsing and help text only expose `index start`, `load crate`, `save db`, and `update`; there are no `load <workspace>`, `load crates ...`, `workspace status`, `workspace update`, or `workspace rm` commands in [`crates/ploke-tui/src/app/commands/parser.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/parser.rs) and [`crates/ploke-tui/src/app/commands/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/mod.rs).
- The command executor routes `index start` to a single `StateCommand::IndexWorkspace`, `load crate` to a single `StateCommand::LoadDb`, and `save db` to a single `StateCommand::SaveDb` in [`crates/ploke-tui/src/app/commands/exec.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/exec.rs).
- `index_workspace` resolves one target directory, sets one focused crate, parses that directory, and passes one workspace string into the embed/index task in [`crates/ploke-tui/src/app_state/handlers/indexing.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs).
- `save_db` writes one backup based on `focused_crate_name`, while `load_db` imports one backup, restores one embedding set, and resets one root path/focus in [`crates/ploke-tui/src/app_state/database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs).
- `SystemStatus` already stores `workspace_roots`, versions, deps, and stale markers, but only one active `crate_focus` is used for behavior and path policy in [`crates/ploke-tui/src/app_state/core.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs).
- The LLM prompt adds a single focused-crate hint and explicitly says tools operate on the focused crate in [`crates/ploke-tui/src/llm/manager/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs), and the read/edit tools still hard-require a focused crate in [`crates/ploke-tui/src/rag/context.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs), [`crates/ploke-tui/src/tools/ns_read.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/ns_read.rs), [`crates/ploke-tui/src/tools/create_file.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/create_file.rs), [`crates/ploke-tui/src/tools/code_edit.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs), and [`crates/ploke-tui/src/tools/get_code_edges.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/tools/get_code_edges.rs).

## Missing Pieces
- A workspace registry/config layer is needed so `/save db` can persist multiple crate backups plus their names/paths, and `/load <workspace>` can resolve that saved set later.
- Indexing needs a workspace orchestrator that enumerates all crates in the workspace, parses each one, transforms them into the DB, generates embeddings, and builds HNSW/BM25 indexes for the full set rather than one focused crate.
- Load/update/remove flows need conflict checks for duplicate crate paths, stale file-hash detection, and a clear policy for replacing or removing an already loaded crate.
- Workspace-aware search/edit needs a way to keep a usable current focus while still searching the whole workspace, instead of relying on one crate root as the entire universe.

## Likely Touchpoints
- [`crates/ploke-tui/src/app_state/dispatcher.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/dispatcher.rs) and [`crates/ploke-tui/src/app/events.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/events.rs) for new command/event routing and reindex triggers.
- [`crates/ploke-tui/src/app_state/core.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs) for workspace inventory, status reporting, and per-crate freshness tracking.
- [`crates/ploke-tui/src/app_state/database.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs) for multi-crate save/load/update/remove plumbing.
- [`crates/ploke-tui/src/app/commands/parser.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/parser.rs) and [`crates/ploke-tui/src/app/commands/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/app/commands/mod.rs) for the new CLI surface and help text.
- [`crates/ploke-tui/src/llm/manager/mod.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs) plus [`crates/ploke-tui/src/rag/context.rs`](/home/brasides/code/ploke/crates/ploke-tui/src/rag/context.rs) and tool modules for workspace-scoped retrieval and edits.

## Open Risks
- Broadening focus from one crate to many can weaken path and tool scoping if IO roots are not refreshed per loaded workspace.
- Prefix-based backup lookup is fragile for multi-crate workspaces; it will need a real registry or manifest to avoid collisions and accidental wrong-loads.
- Workspace-wide stale detection will only be trustworthy if crate identity is stable across parse/load/save cycles, ideally via absolute root plus file hashes.
