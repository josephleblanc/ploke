# Workspaces Survey 2026-03-20

Primary focus for this pass: the ingestion pipeline needed to reach basic
workspace-wide indexing in `ploke-tui`, while keeping the follow-on search/edit
requirements in view.

## Subagent Reports

- [Ingestion pipeline survey](../agents/2026-03-workspaces/ingestion-pipeline-survey.md)
- [DB/RAG workspace survey](./2026-03-20_db-rag-workspace-survey.md)
- [TUI workspace survey](./tui-workspace-survey.md)

## Executive Summary

The critical ingestion primitives already exist:

- `syn_parser` can parse a Cargo workspace and validate selected members via
  [`crates/ingest/syn_parser/src/lib.rs`](../../../crates/ingest/syn_parser/src/lib.rs).
- `ploke-transform` can insert `workspace_metadata` and transform every parsed
  crate in that workspace via
  [`crates/ingest/ploke-transform/src/transform/workspace.rs`](../../../crates/ingest/ploke-transform/src/transform/workspace.rs).
- The embed/index stage is already DB-wide once nodes are present, via
  [`crates/ingest/ploke-embed/src/indexer/mod.rs`](../../../crates/ingest/ploke-embed/src/indexer/mod.rs).

The main blocker is orchestration in `ploke-tui`, not missing parser/transform
support. `/index start` and the current `IndexWorkspace` handler still resolve a
single target directory, set a single crate focus, and run a crate-oriented
parse/import flow instead of a manifest-driven workspace flow. Save/load and
RAG/tool behavior are still centered on one focused crate as well.

## What Is Already In Place

### Ingestion

- [`crates/ingest/syn_parser/src/lib.rs`](../../../crates/ingest/syn_parser/src/lib.rs)
  exposes `parse_workspace(...)`, validates selected members against
  `[workspace].members`, and returns both workspace metadata and per-crate parse
  outputs.
- [`crates/ingest/ploke-transform/src/transform/workspace.rs`](../../../crates/ingest/ploke-transform/src/transform/workspace.rs)
  persists `workspace_metadata` and then transforms each parsed crate graph.
- [`crates/ingest/ploke-transform/src/schema/mod.rs`](../../../crates/ingest/ploke-transform/src/schema/mod.rs)
  already includes the workspace schema in full schema creation.

### Embeddings and Indexes

- The embed/index phase already operates across all relevant DB rows once they
  exist. That means a correctly populated multi-crate DB can already drive
  embeddings and HNSW creation without inventing a second per-crate indexing
  engine.
- `ploke-db` already has multi-embedding support and per-set HNSW relations, but
  the public runtime still behaves as if one embedding set is globally active in
  a loaded DB. The main touchpoints are
  [`crates/ploke-db/src/database.rs`](../../../crates/ploke-db/src/database.rs)
  and
  [`crates/ploke-db/src/multi_embedding/db_ext.rs`](../../../crates/ploke-db/src/multi_embedding/db_ext.rs).
- BM25 exists, but it is rebuilt from the whole DB into one actor-owned in-memory
  sparse index rather than tracked per workspace. See
  [`crates/ploke-db/src/bm25_index/mod.rs`](../../../crates/ploke-db/src/bm25_index/mod.rs)
  and
  [`crates/ploke-db/src/bm25_index/bm25_service.rs`](../../../crates/ploke-db/src/bm25_index/bm25_service.rs).

## Required Changes Before Workspace-Wide `/index`

### 1. Replace crate-root indexing orchestration with workspace-root orchestration

Current behavior in
[`crates/ploke-tui/src/app_state/handlers/indexing.rs`](../../../crates/ploke-tui/src/app_state/handlers/indexing.rs)
still:

- resolves one `target_dir`
- sets one focused crate from that path
- calls `run_parse(...)` for that target
- then calls the indexer with a single workspace string

Before `/index <workspace>` and bare `/index` can behave as requested, the TUI
needs an explicit manifest-driven workspace indexing path that:

- resolves the workspace root and `Cargo.toml`
- enumerates member crates from workspace metadata rather than `current_dir()`
- calls `parse_workspace(...)`
- calls `transform_parsed_workspace(...)`
- then runs embeddings, HNSW setup, and optional BM25 finalization against the
  populated DB

This is the highest-priority gap. The parser and transform layers do not appear
to need major new capability for the initial version.

### 2. Add workspace inventory/state in TUI

[`crates/ploke-tui/src/app_state/core.rs`](../../../crates/ploke-tui/src/app_state/core.rs)
already has useful ingredients:

- `workspace_roots`
- `crate_focus`
- crate version/dependency tracking
- stale markers

But behavior still depends on one focused crate and one derived path policy. For
workspace indexing and follow-on commands, `SystemStatus` needs to become the
source of truth for:

- loaded workspace root
- loaded crate set
- per-crate root path and identity
- per-crate freshness/hash data for status and update
- current focus within a loaded workspace, separate from the loaded workspace
  itself

### 3. Expand command surface from crate-scoped to workspace-scoped

The command/help surface in
[`crates/ploke-tui/src/app/commands/mod.rs`](../../../crates/ploke-tui/src/app/commands/mod.rs),
[`crates/ploke-tui/src/app/commands/exec.rs`](../../../crates/ploke-tui/src/app/commands/exec.rs),
and
[`crates/ploke-tui/src/app_state/commands.rs`](../../../crates/ploke-tui/src/app_state/commands.rs)
still exposes:

- `index start [directory]`
- `load crate <name>`
- `save db`
- `update`

The requested workflow needs new structured commands and handlers for:

- `/index <workspace>` and bare `/index` in a workspace
- `/save db` for the whole loaded workspace
- `/load <workspace>`
- `/load crates <crate1> <crate2> ...`
- `/workspace status`
- `/workspace update`
- `/workspace rm <crate>`

The command change is not just syntax. Each command needs backing state and
conflict validation.

## Required Changes Before Workspace Save/Load Works

[`crates/ploke-tui/src/app_state/database.rs`](../../../crates/ploke-tui/src/app_state/database.rs)
is still built around one focused crate:

- `save_db(...)` names the backup from `focused_crate_name()`
- `load_db(...)` finds one backup by crate-name prefix
- restoring the DB also restores one active embedding set keyed by crate name

Before the requested save/load behavior works, a workspace registry/config layer
is needed in the user config dir. It should record at least:

- workspace name
- workspace root
- crate names in the workspace
- backup path for each crate or for the workspace snapshot strategy chosen
- embedding-set metadata needed at restore time

It also needs validation rules for:

- duplicate crate names
- duplicate root paths
- loading crates into a DB that already contains overlapping crates
- remove/update behavior when backups do or do not exist

The current prefix-based backup lookup is too weak for multi-crate workflows.

## Required Changes Before Workspace Semantic Search/Edit Works

### Search scope

`ploke-rag` currently searches globally over the loaded DB:

- dense search iterates all primary node types in
  [`crates/ploke-rag/src/core/mod.rs`](../../../crates/ploke-rag/src/core/mod.rs)
- BM25 search talks to one actor-backed sparse index over the current DB
- `get_context(...)` assumes retrieval already produced the right scope

For workspace support, the search layer needs explicit scope plumbing so calls
can mean one of:

- current workspace
- specific crate inside the workspace
- later, possibly fully global across loaded workspaces if that is ever allowed

The initial design target from this task should remain: search the loaded
workspace by default, with optional crate restriction.

### Edit safety

`ploke-tui` still gates tools on `focused_crate()` and resolves file IO relative
to the focused crate root. Verified examples:

- [`crates/ploke-tui/src/llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs)
  injects a prompt hint that tools operate on the focused crate.
- [`crates/ploke-tui/src/rag/context.rs`](../../../crates/ploke-tui/src/rag/context.rs)
  tells the user to `index start <path>` or `load crate <name>` when nothing is
  focused.
- [`crates/ploke-tui/src/tools/ns_read.rs`](../../../crates/ploke-tui/src/tools/ns_read.rs),
  [`crates/ploke-tui/src/tools/code_edit.rs`](../../../crates/ploke-tui/src/tools/code_edit.rs),
  and related tools require `focused_crate_root()`.

Workspace semantic edit therefore needs two changes:

- retrieval and disambiguation must identify the correct crate/file within the
  loaded workspace
- tool path scoping must allow workspace-aware access without weakening the path
  safety model

The safe near-term model is: workspace-wide retrieval, but explicit crate/file
resolution before any edit is applied.

## Suggested Implementation Order

1. Add a workspace-aware indexing orchestration path in `ploke-tui` that uses
   `parse_workspace(...)` plus `transform_parsed_workspace(...)`.
2. Add TUI workspace state that distinguishes loaded workspace membership from
   the current focused crate.
3. Add a workspace registry/config file for `/save db` and `/load <workspace>`.
4. Add `/workspace status`, `/workspace update`, and `/workspace rm <crate>`
   using file-hash or equivalent per-crate freshness data.
5. Thread search scope through `ploke-rag` and related DB query helpers so
   dense/BM25/hybrid retrieval can be restricted to a workspace or crate.
6. Update tool gating, prompt hints, and path policy so tools can operate inside
   the loaded workspace while still using an explicit focused crate when needed
   for ambiguous actions.

## Correctness Risks To Preserve

- Do not make workspace parsing permissive when member selection or manifests do
  not match. The current validation in `syn_parser` should continue failing
  loudly on mismatch.
- Do not silently relax backup/import semantics for workspace save/load. The
  current DB restore path is intentionally strict apart from the documented
  legacy `active_embedding_set` metadata absence.
- Do not let workspace-wide retrieval bypass file/module validation before
  semantic edit. Symbol collisions across crates are a real risk.
- Do not treat `current_dir()` as durable workspace identity. Save/load/status
  should key off persisted workspace metadata and stable crate roots.

## Bottom Line

The project is closer to workspace indexing than the current UI suggests. The
parser, transform, schema, and DB-wide embedding/indexing pieces are largely in
place. The missing work is mostly in `ploke-tui` orchestration and state, plus
scope-aware search/edit plumbing in `ploke-rag` and the TUI tool layer.

If the next step is implementation, the best first slice is to replace the
current single-directory `/index start` flow with a manifest-driven workspace
index flow and then build save/load/status/update behavior around that state
model.
