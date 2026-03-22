# Indexing State Coherence Survey

Backlink:
[docs/active/reports/2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)

This survey checks whether the current `/index`-style paths keep TUI state,
database contents, indexing state, and IO roots coherent. They do not yet do so
for a loaded workspace; the implementation is still effectively single-focus.

## Observed Gaps

- `/index` mutates `SystemStatus` and IO roots before parse success, and it does
  not restore the previous roots on parse failure. It also prefers
  `focused_crate_root()` over the command argument, so a later `/index <path>`
  can reuse the old focus instead of the requested workspace target.
  See [crates/ploke-tui/src/app_state/handlers/indexing.rs#L52](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L52)
  and [crates/ploke-tui/src/app_state/handlers/indexing.rs#L70](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L70).
- `SystemStatus` only stores one `crate_focus` plus per-crate version/invalidation
  data. `derive_path_policy(...)` and `focused_crate_stale()` are both
  focus-only, so there is no first-class loaded-workspace membership to compare
  against a manifest.
  See [crates/ploke-tui/src/app_state/core.rs#L383](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L383)
  and [crates/ploke-tui/src/app_state/core.rs#L500](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L500).
- `save_db` still names snapshots from the focused crate, and `load_db` restores
  focus from a single `crate_context` lookup by crate name prefix. That is not a
  workspace identity, and it can only restore one crate root into TUI state.
  See [crates/ploke-tui/src/app_state/database.rs#L130](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L130)
  and [crates/ploke-tui/src/app_state/database.rs#L234](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L234).
- The embedder sends BM25 `IndexBatch` updates before DB embeddings are written,
  and `FinalizeSeed` only runs on clean completion. A failed or cancelled index
  can therefore leave BM25 ahead of DB state, with no rollback path in the
  failure branch.
  See [crates/ingest/ploke-embed/src/indexer/mod.rs#L770](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs#L770)
  and [crates/ingest/ploke-embed/src/indexer/mod.rs#L550](/home/brasides/code/ploke/crates/ingest/ploke-embed/src/indexer/mod.rs#L550).
- `RagService` search paths are still scope-free. Dense search queries all
  primary node types and truncates after merge; BM25 is global actor state; and
  hybrid search just fuses the two result sets.
  See [crates/ploke-rag/src/core/mod.rs#L234](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L234),
  [crates/ploke-rag/src/core/mod.rs#L489](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L489),
  and [crates/ploke-rag/src/core/mod.rs#L656](/home/brasides/code/ploke/crates/ploke-rag/src/core/mod.rs#L656).
- `parse_workspace(...)` validates the current manifest selection, but there is
  no status/update/index path that re-compares a loaded member set against later
  manifest drift. The transform layer records the current member list into
  `workspace_metadata`, but nothing consumes that row to detect removed or
  added members later.
  See [crates/ingest/syn_parser/src/lib.rs#L66](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L66)
  and [crates/ingest/ploke-transform/src/transform/workspace.rs#L14](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L14).

## Acceptance Claims To Tighten

- `/index` must be atomic with respect to focus and IO roots: either the parsed
  workspace succeeds, or the previous roots remain in place.
- Loaded workspace state must include the member set, not just one focused
  crate, and `status`/`update` must fail or mark stale when the manifest member
  set no longer matches DB-loaded membership.
- BM25 readiness must not be exposed as workspace-ready unless the same batch
  of embeddings has committed to the DB. Failed or cancelled indexing must not
  leave BM25 ahead of DB.
- Workspace save/load must be keyed by workspace identity and membership, not by
  a single focused crate name or prefix lookup.
- Retrieval claims should stay limited to scope correctness on controlled
  fixtures until explicit scope parameters exist in `ploke-rag`/`ploke-db`.

## Current Coverage

- `load_db_restores_saved_embedding_set_and_index` covers single-crate restore,
  not workspace membership coherence.
- `test_update_embed` covers focused-crate stale detection, not multi-crate
  workspace drift.
- `test_transform_parsed_workspace` currently smoke-tests the transform; it
  does not assert `workspace_metadata` contents.
