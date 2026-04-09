# Save/Load/Status/Update Correctness Survey

Backlink: [docs/active/reports/2026-03-20_workspaces_implementation_plan.md](../../reports/2026-03-20_workspaces_implementation_plan.md)
and [docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md](../../reports/2026-03-20_workspaces_acceptance_criteria.md).

## Current source of truth

- `save_db` is still focused-crate driven: it names the snapshot from `crate_context` identity, then persists active embedding-set metadata keyed by crate name. See [crates/ploke-tui/src/app_state/database.rs#L90](../../../../crates/ploke-tui/src/app_state/database.rs#L90) and [crates/ploke-db/src/database.rs#L1187](../../../../crates/ploke-db/src/database.rs#L1187).
- `load_db` still uses prefix-based file discovery, restores the active embedding set from backup metadata, and then re-derives the focused crate and IO roots from `crate_context`. See [crates/ploke-tui/src/app_state/database.rs#L234](../../../../crates/ploke-tui/src/app_state/database.rs#L234), [crates/ploke-tui/src/utils/helper.rs#L1](../../../../crates/ploke-tui/src/utils/helper.rs#L1), and [crates/ploke-db/src/database.rs#L974](../../../../crates/ploke-db/src/database.rs#L974).
- `scan_for_change` is still single-crate only: it reads the focused crate, queries `get_crate_files(crate_name)`, and reindexes from that one crate root. See [crates/ploke-tui/src/app_state/database.rs#L499](../../../../crates/ploke-tui/src/app_state/database.rs#L499) and [crates/ploke-db/src/database.rs#L390](../../../../crates/ploke-db/src/database.rs#L390).
- Session staleness state is in-memory only. `SystemStatus.crate_versions` and `invalidated_crates` are caches, not persisted truth. See [crates/ploke-tui/src/app_state/core.rs#L383](../../../../crates/ploke-tui/src/app_state/core.rs#L383).
- The only persisted workspace membership record today is `workspace_metadata.members`, written by the transform layer. `crate_context` is per-crate identity, not a workspace registry. See [crates/ingest/ploke-transform/src/transform/workspace.rs#L14](../../../../crates/ingest/ploke-transform/src/transform/workspace.rs#L14) and [crates/ingest/ploke-transform/src/transform/workspace.rs#L60](../../../../crates/ingest/ploke-transform/src/transform/workspace.rs#L60).

## What can drift

- `current_dir()` can diverge from the loaded workspace. The current indexing path still falls back to it when no focus exists. See [crates/ploke-tui/src/app_state/handlers/indexing.rs#L36](../../../../crates/ploke-tui/src/app_state/handlers/indexing.rs#L36).
- Backup discovery can drift from intent because `find_file_by_prefix` is not a registry lookup; a prefix collision can select the wrong snapshot.
- Workspace membership can drift from the DB snapshot if files are added or removed on disk after indexing, because load/status/update do not currently revalidate `workspace_metadata.members` against a workspace registry.
- Active embedding metadata can drift from the loaded DB if the snapshot lacks metadata; `restore_embedding_set` currently falls back to the first populated set. See [crates/ploke-db/src/database.rs#L974](../../../../crates/ploke-db/src/database.rs#L974).
- Registry metadata can drift from the backup file itself; that should fail loudly under `cargo xtask verify-backup-dbs`, not be papered over by permissive import. See [docs/testing/BACKUP_DB_FIXTURES.md#L1](../../../testing/BACKUP_DB_FIXTURES.md#L1).

## Acceptance propositions to add

- The authoritative loaded-workspace membership set must come from a registry-backed workspace snapshot, with `workspace_metadata.members` and `crate_context` agreeing for every loaded member. `crate_context` may restore focus and IO roots, but it must not discover membership on its own.
- `/workspace status` and `/workspace update` must enumerate all loaded workspace members from loaded-workspace state, not from `current_dir()` or the focused crate alone.
- If a workspace save/load cycle encounters added or removed members on disk, the mismatch must be reported as drift. The implementation must not silently merge, drop, or reuse a stale member list.
- If a registry entry points at a missing or schema-drifted backup, load must fail until the fixture is regenerated or repaired explicitly. No prefix-based fallback should be treated as correct workspace restore behavior.
- Missing active-set metadata may remain a legacy crate-backup fallback, but it is not sufficient for workspace restore correctness. Workspace acceptance should require the active set to round-trip with the snapshot, or fail explicitly.

## Failure states to codify

- Only the focused crate is restored, indexed, or reported.
- `status` omits loaded members or reports stale/fresh state from the wrong root.
- `update` clears or preserves stale markers without reconciling the loaded member set.
- Backup lookup chooses the wrong file because of name prefix overlap.
- Load succeeds while workspace membership or active-set metadata is stale or missing.

