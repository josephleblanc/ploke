# C6 Subset DB Design Notes 2026-03-21

Backlinks:
- [2026-03-20_workspaces_implementation_plan.md](../../reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](../../reports/2026-03-20_workspaces_acceptance_criteria.md)
- [2026-03-20_workspaces_progress_tracker.md](2026-03-20_workspaces_progress_tracker.md)

## Current finding

Phase 7 `C6` must begin in `ploke-db`, not in `ploke-tui`.

The current code has:

- whole-backup import through `Database::import_backup_with_embeddings(...)` in
  [database.rs](../../../../crates/ploke-db/src/database.rs)
- global relation removal through `Database::clear_relations(...)` in
  [database.rs](../../../../crates/ploke-db/src/database.rs)
- embedding-only partial removal through `Database::retract_embedded_files(...)`
  in [database.rs](../../../../crates/ploke-db/src/database.rs)

It does not yet have a namespace-scoped graph export/import/remove primitive.

## Why this matters

`C6` is not satisfied by wrapping current whole-DB backup/restore behavior in a
crate-targeted command. The acceptance criteria require real namespace-scoped
subset operations with:

- explicit conflict validation
- no silent whole-DB replacement
- authoritative membership updates after mutation
- search-state reconciliation after mutation

See
[2026-03-20_workspaces_acceptance_criteria.md](../../reports/2026-03-20_workspaces_acceptance_criteria.md)
Phase 7 `C6`.

## Existing seam to build from

The strongest current namespace seam is:

- `crate_context.namespace`
- `file_mod.namespace`

The most useful current helper in this area is `Database::get_crate_files(...)`
in [database.rs](../../../../crates/ploke-db/src/database.rs),
which already joins `crate_context.namespace -> file_mod.namespace`.

This is a better starting point than:

- crate-name-only lookup
- backup filename lookup
- focused-crate or cwd-derived logic

## Immediate implementation direction

Start with a DB-level namespace inventory/query layer in
[database.rs](../../../../crates/ploke-db/src/database.rs).

That layer should:

1. enumerate crate-context rows by namespace
2. enumerate root `file_mod.owner_id` values for a namespace
3. enumerate descendant graph node ids reachable from those roots via
   `syntax_edge.relation_kind == "Contains"`

This inventory is the narrowest safe precursor for:

- subset export
- subset import conflict validation
- subset removal

## Important caveat

A descendant-id inventory is necessary but not sufficient for full subset
mutation.

Some relations are keyed by:

- `id`
- `owner_id`
- `type_id`
- `function_id`
- `source_id` / `target_id`

So a later remove/export implementation must account for more than just primary
nodes and `file_mod`.

## Recommended first witness

Add a `ploke-db` fixture-backed test against `ws_fixture_01_canonical` proving
that the DB can derive a namespace inventory for each crate in the loaded
workspace and that the two namespaces remain distinct.

This is not the full `C6` witness, but it is the first direct proof that the
subset path is being built on namespace authority instead of crate-name or
whole-DB assumptions.

Update 2026-03-21:

- `Database::remove_namespace(...)` now exists in
  [database.rs](../../../../crates/ploke-db/src/database.rs) as
  the first real namespace-scoped subset mutation primitive.
- The fixture-backed witness
  `remove_namespace_removes_only_target_namespace_and_invalidates_search_state`
  now proves one crate namespace can be removed from `ws_fixture_01_canonical`
  without whole-DB replacement, while reconciling `workspace_metadata.members`,
  descendant graph rows, vector rows, BM25 metadata rows, and active HNSW
  availability.
- `C6` is still not complete: subset export/import and import conflict
  validation remain the next missing DB primitives.

Update 2026-03-21 (later):

- `Database::export_namespace(...)` now exists in
  [database.rs](../../../../crates/ploke-db/src/database.rs) as
  the first real namespace-scoped subset export primitive.
- `Database::import_namespace(...)` now exists in
  [database.rs](../../../../crates/ploke-db/src/database.rs) as
  the first real namespace-scoped subset import primitive, with explicit
  duplicate namespace/name/root conflict reporting.
- The fixture-backed witness
  `export_namespace_artifact_contains_only_target_namespace_rows` proves the
  exported artifact is built from explicit namespace inventory and contains
  only the target crate's `crate_context`, `file_mod`, rooted graph rows,
  vector rows, BM25 rows, embedding-set metadata, and a pruned
  `workspace_metadata.members` list.
- The fixture-backed witnesses
  `import_namespace_restores_exported_namespace_into_populated_db_and_invalidates_search_state`
  and `import_namespace_reports_duplicate_namespace_name_and_root_conflicts`
  now prove one exported namespace can be re-imported into a populated DB
  without whole-DB replacement, and that duplicate namespace/name/root cases
  fail explicitly.
- `C6` still remains incomplete overall: the DB primitives now exist, but the
  next missing layer is TUI/runtime subset command wiring so loaded membership,
  focus, IO roots, registry/snapshot metadata, and search availability update
  end to end.

Update 2026-03-21 (later still):

- `ploke-tui` now has the first end-to-end subset command path:
  `workspace rm <crate-name-or-exact-root>`.
- The runtime path is wired through structured command parsing/execution,
  `StateCommand::WorkspaceRemove`, the dispatcher, `handlers::db`, and
  `app_state::database::workspace_remove(...)`.
- `workspace_remove(...)` now reuses `Database::remove_namespace(...)` and
  then republishes loaded workspace membership, focus, and IO roots from the
  live DB snapshot rather than from pre-mutation state.
- The TUI-side restore helpers now query `workspace_metadata` and
  `crate_context` with `@ 'NOW'`; this turned out to be necessary because
  subset mutation correctness otherwise leaked historical rows back into the
  post-mutation runtime snapshot.
- The fixture-backed witness
  `workspace_remove_updates_runtime_membership_focus_and_snapshot_metadata`
  now proves one subset remove command updates surviving membership, focus,
  derived path policy, registry/snapshot metadata, and explicit search
  invalidation messaging together.
- `C6` still remains incomplete overall: subset import/export command wiring is
  still missing even though the DB primitives already exist and the first
  remove command path is now end to end.

Update 2026-03-21 (latest):

- `ploke-tui` now also has the end-to-end subset import command path
  `load crates <workspace-name-or-id> <crate-name-or-exact-root>`.
- The runtime path resolves an exact workspace registry entry, validates the
  source snapshot against that registry metadata, exports the requested
  namespace from a staging DB, imports it into the live DB through
  `Database::import_namespace(...)`, and then republishes loaded membership,
  focus, IO roots, and rewritten registry/snapshot metadata from the live DB.
- The new fixture-backed witnesses
  `workspace_load_crates_restores_removed_member_and_snapshot_metadata` and
  `workspace_load_crates_conflict_preserves_runtime_state` now prove:
  - successful subset import restores the missing member end to end without
    whole-DB replacement
  - conflict validation surfaces before runtime membership/focus/IO/search
    state changes are published
- With those witnesses plus the earlier DB-side remove/export/import/conflict
  witnesses and `/workspace rm` runtime witness, `C6` is now satisfied.
