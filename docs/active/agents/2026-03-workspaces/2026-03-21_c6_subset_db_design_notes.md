# C6 Subset DB Design Notes 2026-03-21

Backlinks:
- [2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)
- [2026-03-20_workspaces_progress_tracker.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/2026-03-20_workspaces_progress_tracker.md)

## Current finding

Phase 7 `C6` must begin in `ploke-db`, not in `ploke-tui`.

The current code has:

- whole-backup import through `Database::import_backup_with_embeddings(...)` in
  [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
- global relation removal through `Database::clear_relations(...)` in
  [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
- embedding-only partial removal through `Database::retract_embedded_files(...)`
  in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)

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
[2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)
Phase 7 `C6`.

## Existing seam to build from

The strongest current namespace seam is:

- `crate_context.namespace`
- `file_mod.namespace`

The most useful current helper in this area is `Database::get_crate_files(...)`
in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs),
which already joins `crate_context.namespace -> file_mod.namespace`.

This is a better starting point than:

- crate-name-only lookup
- backup filename lookup
- focused-crate or cwd-derived logic

## Immediate implementation direction

Start with a DB-level namespace inventory/query layer in
[database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs).

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

