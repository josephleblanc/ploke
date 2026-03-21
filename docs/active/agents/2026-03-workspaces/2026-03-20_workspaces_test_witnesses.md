# Workspace Test Witnesses 2026-03-20

Backlinks:
- [2026-03-20_workspaces_progress_tracker.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/2026-03-20_workspaces_progress_tracker.md)
- [2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)

Use this document to record why specific tests count as witness evidence for a
readiness item or phase acceptance criterion. Keep entries short and tied to
the criterion language, not just to the implementation details.

## Current witness coverage

| Acceptance item | Test | Why this counts as witness evidence |
| --- | --- | --- |
| `R1` | `parse_workspace_committed_fixture_uses_multi_member_workspace` in [lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L745) | Uses the committed on-disk fixture `ws_fixture_01`, calls `parse_workspace(...)` directly, and asserts the normalized two-member workspace set including nested `nested/member_nested`, which matches the R1 invariant that the fixture is a real Cargo workspace consumable directly by parser code |
| `R1` | `committed_workspace_fixture_locates_nested_members` in [workspace.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L516) | Uses the same committed fixture without tempdir generation, calls `locate_workspace_manifest(...)` from the nested member path, and asserts the discovered workspace root plus normalized member paths, which matches the R1 invariant and the parser/discovery shared-fixture contract |
| `R2` | `backup_db_fixture_lookup_returns_registered_workspace_fixture` in [fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L406) | Resolves the workspace fixture through the shared registry by ID and asserts the active filename, parsed target, and plain-backup mode, which matches the R2 requirement that the workspace backup exist as a registry-backed fixture rather than an ad hoc file on disk |
| `R2` | `workspace_backup_fixture_loads_via_registry_and_has_workspace_metadata` in [fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L423) | Loads the registered workspace fixture through the strict registry-backed helper and asserts that the restored DB contains both `workspace_metadata` and two `crate_context` rows, which matches the R2 requirement that the fixture load through the same strict paths used by shared consumers |
| `R3` | `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture` in [workspace.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L170) | Parses the committed multi-member fixture `ws_fixture_01`, transforms it into a fresh DB, queries `workspace_metadata`, and asserts the exact persisted identity and manifest-derived fields required by R3 |
| Phase 3 `C2` | `resolve_index_target_prefers_crate_root_when_pwd_is_crate_root` in [parser.rs](/home/brasides/code/ploke/crates/ploke-tui/src/parser.rs#L257) | Proves the revised `/index` target semantics choose crate mode when the supplied path is itself a crate root, instead of silently escalating that exact path to workspace mode |
| Phase 3 `C2` | `resolve_index_target_finds_workspace_when_pwd_is_not_crate_root` in [parser.rs](/home/brasides/code/ploke/crates/ploke-tui/src/parser.rs#L269) | Proves the revised target resolution falls back to the nearest ancestor workspace when the supplied path is not a crate root, which is the documented replacement for the earlier bare-workspace-only behavior |
| Phase 3 `C2` | `resolve_index_target_reports_missing_crate_or_workspace` in [parser.rs](/home/brasides/code/ploke/crates/ploke-tui/src/parser.rs#L287) | Proves non-Cargo targets fail as recoverable errors with explicit user guidance rather than being silently accepted as valid indexing roots |
| Phase 3 `C2` | `index_workspace_resolves_ancestor_workspace_from_nested_path` in [index_workspace_targets.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/index_workspace_targets.rs#L40) | Drives the real `ploke-tui` indexing handler from a nested path inside `ws_fixture_01` and proves successful parse/transform commits multi-member loaded-workspace state and member-scoped path policy roots |
| Phase 3 `C2` | `index_workspace_failure_keeps_previous_loaded_workspace_state` in [index_workspace_targets.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/index_workspace_targets.rs#L84) | Proves an invalid target records failure but preserves the previously loaded workspace root and member set instead of publishing partial state |
| Phase 3 `C2` | `index_workspace_anchors_repo_relative_target_to_loaded_state_when_cwd_differs` in [index_workspace_targets.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/index_workspace_targets.rs#L154) | Proves the real indexing handler does not silently reinterpret a repo-relative target from process cwd when loaded app state already identifies the absolute crate root, which is the regression that previously surfaced through `test_update_embed` |
| Phase 4 `C3` | `workspace_status_and_update_operate_per_loaded_crate` in [workspace_status_update.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/workspace_status_update.rs#L103) | Drives the real workspace status/update path on the committed multi-member fixture, proves one changed member is marked stale while the untouched member stays fresh, then proves `/workspace update` returns both to fresh without dropping a seeded unchanged-member embedding |
| Phase 4 `C3` | `workspace_status_reports_workspace_member_drift` in [workspace_status_update.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/workspace_status_update.rs#L178) | Mutates the committed workspace manifest after load and proves `/workspace status` reports removed-member drift explicitly instead of silently absorbing the changed member set |
| Phase 5 `C4` | `load_db_restores_saved_embedding_set_and_index` in [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1649) | Saves a crate-backed singleton workspace snapshot through `/save db`, asserts a workspace-registry entry was written, then reloads through the registry-backed `/load` path and proves active embedding-set metadata plus HNSW restore still roundtrip |
| Phase 5 `C4` | `load_db_requires_workspace_registry_entry_instead_of_prefix_lookup` in [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1752) | Proves `/load` no longer scans backup filenames by prefix: a stray prefix-matching backup file is insufficient without a registry entry |
| Phase 5 `C4` | `load_db_rejects_first_populated_embedding_fallback_for_workspace_registry_loads` in [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1778) | Proves workspace restore rejects the legacy `FirstPopulated` embedding-set fallback when snapshot metadata is missing, which is a required `C4` failure mode |
| Phase 5 `C4` | `load_db_fails_when_registry_metadata_disagrees_with_restored_snapshot` in [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1868) | Proves registry/snapshot disagreement fails explicitly instead of silently preferring stale registry metadata over restored snapshot metadata |
| Phase 6 `C5` | `bm25_specific_crate_scope_filters_before_top_k_truncation` in [bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs#L1006) | Builds a two-namespace in-memory BM25 corpus where the stronger out-of-scope document wins unscoped `top_k=1`, then proves `SpecificCrate(CrateId)` still returns the weaker in-scope document at `top_k=1`, which is direct evidence that scope is applied before BM25 truncation |
| Phase 7 `C6` | `workspace_fixture_namespace_inventory_matches_crate_context_membership` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Loads `ws_fixture_01_canonical`, enumerates `crate_context` rows, builds a per-namespace inventory from `crate_context.namespace -> file_mod.namespace -> syntax_edge` descendant closure, and proves each loaded crate has a non-empty graph inventory rooted inside the committed workspace fixture |
| Phase 7 `C6` | `workspace_fixture_namespaces_remain_distinct_in_subset_inventory` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Proves the two workspace members in `ws_fixture_01_canonical` produce distinct namespace inventories with disjoint root file modules and disjoint descendant graph ids, which is the first direct evidence that later subset operations can key off explicit namespace authority rather than crate-name or whole-DB assumptions |
| Phase 7 `C6` | `remove_namespace_removes_only_target_namespace_and_invalidates_search_state` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Seeds dense/BM25 state for one crate namespace inside `ws_fixture_01_canonical`, removes that namespace through `Database::remove_namespace(...)`, and proves the sibling namespace survives while target `crate_context`, `workspace_metadata.members`, graph rows, vectors, BM25 rows, and HNSW registration are all reconciled to the post-mutation dataset |
| Phase 7 `C6` | `export_namespace_artifact_contains_only_target_namespace_rows` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Seeds dense/BM25 plus active embedding metadata for one crate namespace inside `ws_fixture_01_canonical`, exports that namespace through `Database::export_namespace(...)`, and proves the artifact contains only the target namespace’s `crate_context`, `file_mod`, rooted graph rows, vector rows, BM25 rows, embedding-set metadata, and pruned `workspace_metadata.members` |
| Phase 7 `C6` | `import_namespace_restores_exported_namespace_into_populated_db_and_invalidates_search_state` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Exports one namespace from `ws_fixture_01_canonical`, removes it from a second loaded copy of that workspace, recreates HNSW on the surviving namespace, then imports the artifact back through `Database::import_namespace(...)` and proves the namespace returns without whole-DB replacement while `workspace_metadata.members`, vectors, BM25 rows, and HNSW availability are reconciled to the post-import dataset |
| Phase 7 `C6` | `import_namespace_reports_duplicate_namespace_name_and_root_conflicts` in [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs) | Attempts to import an exported namespace artifact back into the unchanged source workspace DB and proves `Database::import_namespace(...)` rejects it with an explicit conflict report naming the duplicate namespace, crate name, and root path instead of silently replacing or merging conflicting data |

## Reasoning by acceptance item

### `R1` committed multi-member workspace fixture exists

Criterion text:
- requires a committed fixture under `tests/fixture_workspace/`
- requires at least two member crates
- requires at least one nested member path
- requires direct consumption by `parse_workspace(...)` and
  `locate_workspace_manifest(...)`
- requires parser and discovery tests to consume the same fixture without ad
  hoc tempdir generation

Current witness reasoning:
- [ws_fixture_01/Cargo.toml](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_01/Cargo.toml#L1)
  defines the committed fixture with members `member_root` and
  `nested/member_nested`
- [lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L745)
  proves parser-side direct consumption on the committed fixture
- [workspace.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L516)
  proves discovery-side direct consumption on the same committed fixture

What a passing witness proves:
- if `parse_workspace_committed_fixture_uses_multi_member_workspace` passes,
  then the committed fixture can be parsed directly by `parse_workspace(...)`
  as a real on-disk workspace and yields the expected normalized two-member set
  including the nested member path
- if `committed_workspace_fixture_locates_nested_members` passes, then
  `locate_workspace_manifest(...)` can start from the nested member crate,
  resolve the committed workspace root, and recover the expected normalized
  workspace member list from that same fixture
- if both tests pass, then the repo has one committed multi-member fixture that
  both parser and discovery code consume directly without tempdir generation,
  which is the acceptance witness required for `R1`

Scope note:
- this witness set is sufficient for `R1`
- it does not count as evidence for `R2`, `R3`, `R4`, or later phase criteria

### `R2` registered workspace backup fixture exists

Criterion text:
- requires at least one workspace backup fixture registered in the shared
  `FixtureDb` registry
- requires a matching inventory entry in
  `docs/testing/BACKUP_DB_FIXTURES.md`
- requires the fixture to load through the same strict registry-backed paths
  as existing fixtures
- forbids permissive import behavior as a workaround for schema drift

Current witness reasoning:
- [fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L231)
  defines `WS_FIXTURE_01_CANONICAL` as an active registry-backed fixture with
  a concrete backup path and parsed target
- [BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md)
  documents the same registered fixture in the backup fixture inventory
- [fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L423)
  proves the fixture loads through `fresh_backup_fixture_db(...)`, which is the
  strict shared registry-backed import path

What a passing witness proves:
- if `backup_db_fixture_lookup_returns_registered_workspace_fixture` passes,
  then the workspace fixture is not merely present on disk; it is registered in
  the shared fixture registry with the expected ID, target, and import mode
- if `workspace_backup_fixture_loads_via_registry_and_has_workspace_metadata`
  passes, then the registered fixture can be restored through the strict shared
  helper and the restored DB includes persisted workspace metadata together
  with the expected workspace-member crate rows
- if both tests pass, then the repo has at least one registered
  registry-backed workspace fixture satisfying the core `R2` witness
  requirement

Scope note:
- this witness set is sufficient for the code-side portion of `R2`
- the tracker still records separate doc and scoped `verify-backup-dbs`
  evidence because `R2` also requires registry/doc inventory alignment
- it does not count as evidence for `R3`, `R4`, or later phase criteria

### `R3` `workspace_metadata` transform is asserted

Criterion text:
- requires a transform test that queries `workspace_metadata`
- requires persisted `workspace_metadata.id` and `.namespace` to be derived
  from `WorkspaceId::from_root_path(...)`
- requires `root_path`, `members`, `exclude`, `resolver`, and
  `package_version` to match the parsed manifest

Current witness reasoning:
- [workspace.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L170)
  parses the committed fixture `ws_fixture_01`, transforms it into a fresh DB,
  queries the persisted `workspace_metadata` row, and asserts every field named
  by the acceptance criterion

What a passing witness proves:
- if
  `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture`
  passes, then `transform_parsed_workspace(...)` does more than return `Ok(())`:
  it persists a `workspace_metadata` row whose `id` and `namespace` match
  `WorkspaceId::from_root_path(...)`, whose `root_path` matches the parsed
  workspace root, whose `members` list matches the canonical workspace-root
  joined member set, whose `exclude` value is null when absent in the manifest,
  and whose `resolver` and `package_version` match the parsed manifest

Scope note:
- this witness is sufficient for `R3`
- it does not yet prove the stronger Phase 1 `C0` coherence requirement that
  restored `crate_context` membership equals `workspace_metadata.members`

### `R4` strict backup verification passes

Criterion text:
- requires backup import to remain strict
- requires schema drift to be handled by regeneration or documented repair, not
  silent tolerance
- requires the workspace fixture set to pass `cargo xtask verify-backup-dbs`

Current witness reasoning:
- `R4` is backed by command evidence rather than a unit test
- the full strict verification command succeeded for every active registered
  fixture, including the workspace fixture `ws_fixture_01_canonical`

What a passing witness proves:
- if `cargo xtask verify-backup-dbs` passes, then the active registered backup
  fixtures roundtrip through the repo's strict verification path without
  needing permissive import behavior
- if that command passes for `ws_fixture_01_canonical`, then the newly added
  workspace fixture satisfies the same strict backup verification contract as
  the existing registered fixtures

Scope note:
- this is sufficient evidence for `R4`
- it does not by itself prove Phase 1 `C0`, which still needs fixture-backed
  coherence assertions between `workspace_metadata.members` and restored
  `crate_context` membership

### Phase 1 `C0` workspace snapshot coherence has fixture-backed witness evidence

Criterion text:
- requires fixture-backed equality assertions between
  `workspace_metadata.members` and restored multi-member crate rows
- requires restored workspace identity and root path to agree with
  `WorkspaceId::from_root_path(...)`
- requires backup/restore not to change the represented workspace member set

Current witness reasoning:
- [fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L441)
  loads the registered backup fixture `ws_fixture_01_canonical` through the
  strict registry-backed restore path, queries both `workspace_metadata` and
  `crate_context`, and compares the restored membership sets directly

What a passing witness proves:
- if `workspace_backup_fixture_roundtrips_coherent_membership_and_identity`
  passes, then the restored workspace snapshot contains one
  `workspace_metadata` row whose `id` and `namespace` match
  `WorkspaceId::from_root_path(...)`, whose `root_path` matches the committed
  fixture root, and whose `members` set equals the set of restored
  `crate_context.root_path` rows
- if that test passes, then the registered workspace snapshot preserves both
  workspace identity and member-set coherence across the transform/backup/restore
  path required by Phase 1 `C0`

Scope note:
- this is sufficient regression evidence for Phase 1 `C0`
- it is not a universal proof over all possible workspaces, which matches the
  acceptance criterion's stated scope

### Phase 2 `C1` loaded workspace state is first-class in `ploke-tui`

Criterion text:
- requires a first-class loaded-workspace structure rather than focus-only
  state
- requires focused crate to be absent or a member of the loaded workspace
- requires path policy roots to derive from loaded workspace membership, not
  only the focused crate root
- requires loaded workspace membership to come from parsed or restored
  workspace state rather than ad hoc focus inference

Current witness reasoning:
- [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L388)
  introduces `LoadedWorkspaceState` and stores it explicitly on
  `SystemStatus`
- [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs#L569)
  proves that loaded membership controls both focus validity and derived path
  policy roots
- [load_db_crate_focus.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/load_db_crate_focus.rs#L73)
  proves that restored `workspace_metadata` and `crate_context` hydrate loaded
  workspace state from the DB-backed restore path rather than from focus alone

What a passing witness proves:
- if `loaded_workspace_membership_controls_focus_and_path_policy` passes, then
  `SystemStatus` can represent a multi-member loaded workspace, focus one of
  its members, and derive read roots from the full loaded member set rather
  than only the focused crate root
- if `set_focus_from_root_preserves_existing_loaded_workspace_membership`
  passes, then changing focus within a loaded workspace does not collapse the
  member set to a focus-only singleton and the focused crate remains a member of
  the loaded workspace
- if `workspace_restore_assigns_loaded_workspace_membership_from_db` passes,
  then restored `workspace_metadata` drives loaded workspace hydration on the
  `ploke-tui` side and produces a member-scoped path policy from the persisted
  workspace state
- if all three pass, then `ploke-tui` has first-class loaded workspace state
  with explicit membership authority and member-scoped IO roots, satisfying
  Phase 2 `C1`

Scope note:
- this is sufficient evidence for Phase 2 `C1`
- manifest-driven indexing behavior remains Phase 3 `C2`

### Phase 3 `C2` manifest-driven indexing

Criterion text:
- requires `/index` target resolution to be explicit rather than inferred from
  focus-only state
- requires non-Cargo targets to fail explicitly with helpful feedback
- requires successful indexing to publish loaded workspace membership and IO
  roots only after parse/transform success
- requires failure to preserve the previously coherent loaded workspace state

Current witness reasoning:
- [parser.rs](/home/brasides/code/ploke/crates/ploke-tui/src/parser.rs#L30)
  introduces explicit index-target resolution that distinguishes crate-root,
  ancestor-workspace, and recoverable no-target cases
- [indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs#L33)
  resolves the target before parsing, anchors relative targets to loaded
  app-state authority when possible, and only publishes `set_loaded_workspace`
  plus derived IO roots after `run_parse_resolved(...)` succeeds
- [index_workspace_targets.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/index_workspace_targets.rs#L40)
  drives the real handler and asserts both successful workspace publication and
  failure-path state preservation

What a passing witness proves:
- if `resolve_index_target_prefers_crate_root_when_pwd_is_crate_root` passes,
  then `/index` on an exact crate root uses crate-mode indexing rather than
  silently broadening that exact target to workspace scope
- if `resolve_index_target_finds_workspace_when_pwd_is_not_crate_root` passes,
  then `/index` on a non-crate path inside a workspace resolves to the nearest
  ancestor workspace and carries the full canonical member set needed for
  multi-member indexing
- if `resolve_index_target_reports_missing_crate_or_workspace` passes, then
  non-Cargo targets are rejected explicitly with recoverable guidance instead
  of being silently accepted as indexing roots
- if `index_workspace_resolves_ancestor_workspace_from_nested_path` passes,
  then the real `ploke-tui` indexing handler can start from a nested path,
  parse/transform the ancestor workspace, and publish loaded workspace
  membership plus member-scoped path policy only after success
- if `index_workspace_failure_keeps_previous_loaded_workspace_state` passes,
  then a failed target resolution/indexing attempt does not overwrite the
  previously loaded workspace state, which is the Phase 3 `C2` witness for the
  failure-preserves-state half of `G1`
- if `index_workspace_anchors_repo_relative_target_to_loaded_state_when_cwd_differs`
  passes, then the real handler no longer reproduces the `test_update_embed`
  regression where a repo-relative target string was silently re-resolved from
  `crates/ploke-tui` cwd despite already-loaded absolute crate state
- if all six tests pass, then the revised `C2` command semantics and the
  handler-level atomic publish contract both have direct regression witnesses

Scope note:
- this is sufficient evidence for the implemented Phase 3 `C2` behavior
- the accepted runtime semantics now follow the user-approved rule "crate if
  the supplied path is a crate root, otherwise nearest ancestor workspace,"
  which is intentionally narrower than the original plan text that treated bare
  `/index` as always workspace-oriented
- helper-level regression tests in
  [indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs)
  additionally isolate the loaded-state anchoring bug, but they are supporting
  repro coverage rather than the primary acceptance witness
- later phases still need stronger whole-session `G1` witnesses covering
  embedding/HNSW/BM25 coherence after mutating commands

### Phase 4 `C3` workspace status and update are per-crate, not focus-only

Criterion text:
- requires `/workspace status` to report every loaded crate, not only the
  focused crate
- requires stale detection to key off loaded crate roots and DB file metadata
- requires `/workspace update` to converge stale crates back to fresh without
  dropping unchanged embeddings
- requires member-set drift to be surfaced explicitly rather than silently
  ignored

Current witness reasoning:
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1200)
  adds per-loaded-crate freshness collection plus explicit workspace drift
  comparison against the current manifest
- [workspace_status_update.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/workspace_status_update.rs#L103)
  drives the real status/update path against committed fixture `ws_fixture_01`
  with one changed member and one unchanged member
- [workspace_status_update.rs](/home/brasides/code/ploke/crates/ploke-tui/tests/workspace_status_update.rs#L178)
  mutates the loaded workspace manifest and proves drift is surfaced in
  user-visible status output

What a passing witness proves:
- if `workspace_status_and_update_operate_per_loaded_crate` passes, then
  `ploke-tui` computes freshness across all loaded workspace members rather
  than only the focused crate, records the changed member as stale while the
  untouched member remains fresh, and `/workspace update` returns the loaded
  member set to a fresh state without clearing a seeded unchanged-member
  embedding
- if `workspace_status_reports_workspace_member_drift` passes, then
  `/workspace status` re-compares the loaded member set against the current
  manifest and explicitly reports removed-member drift instead of silently
  accepting the mismatch
- if both tests pass, then Phase 4 `C3` has direct witness evidence for
  per-crate status/update behavior and explicit workspace-member drift
  surfacing

Scope note:
- this witness set is sufficient for Phase 4 `C3`
- the `workspace_status_and_update_operate_per_loaded_crate` harness still
  emits handled `Cozo embeddings not implemented` logging from the mock-backed
  indexer path, but the test and broader `ploke-tui --tests` suite complete
  successfully
- registry-backed save/load identity remains Phase 5 `C4`

### Phase 5 `C4` workspace save/load uses registry-backed workspace identity

Criterion text:
- requires `/save db` to persist both a whole-workspace snapshot and registry
  metadata
- requires `/load <workspace>` to resolve by exact workspace identity instead
  of filename-prefix lookup
- requires restored membership to come from snapshot/DB metadata
- requires active embedding-set metadata to roundtrip from authoritative
  snapshot metadata rather than the legacy `FirstPopulated` fallback
- requires registry/snapshot disagreement to fail explicitly

Current witness reasoning:
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L398)
  now writes a `WorkspaceRegistryEntry` on save and resolves exact
  workspace-name or workspace-id entries on load
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1649)
  proves registry creation plus active embedding-set/HNSW restore on the
  registry-backed load path
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1752)
  proves legacy prefix-based filename discovery is no longer accepted as a
  load mechanism
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1778)
  proves the workspace restore path refuses `FirstPopulated` fallback when
  authoritative active embedding metadata is missing
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs#L1868)
  proves stale registry metadata is not silently preferred over the restored
  snapshot metadata

What a passing witness proves:
- if `load_db_restores_saved_embedding_set_and_index` passes, then `/save db`
  creates a registry-backed snapshot entry and `/load` can restore that saved
  workspace by registry identity while preserving authoritative active
  embedding-set metadata and recreating the active set's HNSW index
- if `load_db_requires_workspace_registry_entry_instead_of_prefix_lookup`
  passes, then `/load` no longer treats filename-prefix discovery as an
  acceptable locator for workspace snapshots
- if `load_db_rejects_first_populated_embedding_fallback_for_workspace_registry_loads`
  passes, then workspace restore does not silently accept the legacy
  `FirstPopulated` embedding-set fallback as workspace-correct behavior
- if `load_db_fails_when_registry_metadata_disagrees_with_restored_snapshot`
  passes, then disagreement between registry metadata and restored snapshot
  metadata fails explicitly instead of silently preferring stale registry data
- if all four tests pass, then Phase 5 `C4` has direct witness evidence for
  registry-backed save/load identity, strict snapshot metadata authority, and
  explicit mismatch handling

Scope note:
- this witness set is sufficient for the implemented Phase 5 `C4` behavior
- the restored-membership equality requirement is additionally supported by the
  earlier Phase 1 `C0` witness
  `workspace_backup_fixture_roundtrips_coherent_membership_and_identity`
- whole-session `G1` coherence across DB, HNSW, BM25, TUI state, and IO roots
  still remains a separate cross-phase obligation

### Phase 6 `C5` shared retrieval scope model

Criterion text:
- requires one shared scope model across dense, BM25, hybrid, and context
  assembly entrypoints
- requires scope to be enforced before BM25 `top_k`, dense `:limit`, dense
  fallback, and hybrid fusion
- rejects late caller-side filtering as sufficient proof

Current witness reasoning:
- [workspace.rs](/home/brasides/code/ploke/crates/ploke-core/src/workspace.rs#L43)
  now defines shared `RetrievalScope`
- [bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs#L672)
  now applies scope filtering inside `Bm25Indexer::search(...)` before
  truncating to `top_k`
- [bm25_index/mod.rs](/home/brasides/code/ploke/crates/ploke-db/src/bm25_index/mod.rs#L1006)
  proves the pre-`top_k` filter on a two-namespace corpus where the stronger
  out-of-scope document would otherwise win
- [hnsw_ext.rs](/home/brasides/code/ploke/crates/ploke-db/src/multi_embedding/hnsw_ext.rs#L1096)
  proves the dense pre-`:limit` filter on a workspace-backed graph where the
  stronger out-of-scope vector would otherwise win
- [unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L410)
  proves hybrid search does not fuse an out-of-scope dense/BM25 winner into the
  final scoped result set
- [unit_tests.rs](/home/brasides/code/ploke/crates/ploke-rag/src/core/unit_tests.rs#L676)
  proves `get_context(...)` only materializes IDs and file paths from the
  allowed crate scope, even when the out-of-scope candidate is the stronger
  unscoped semantic match

What a passing witness proves:
- if `bm25_specific_crate_scope_filters_before_top_k_truncation` passes, then
  BM25 crate scope is not being applied as late caller-side filtering after
  `top_k`
- if that test passes, then a `SpecificCrate(CrateId)` scope can still return
  the weaker in-scope document at `top_k=1` even when an out-of-scope document
  would win the unscoped search, which is direct evidence that scope is
  enforced before BM25 truncation
- if `search_similar_for_set_specific_crate_scope_filters_before_limit` passes,
  then dense/HNSW search does not apply crate scope after HNSW `:limit`
- if that dense test passes, then a `SpecificCrate(CrateId)` scope can still
  return the weaker in-scope vector at `limit=1` even when the stronger
  out-of-scope vector would win the unscoped search, which is direct evidence
  that dense scope is enforced before truncation
- if `hybrid_specific_crate_scope_excludes_out_of_scope_candidates_before_fusion`
  passes, then hybrid search is not fusing differently scoped dense and BM25
  candidate sets into the final result
- if that hybrid test passes, then a scoped hybrid query returns only the
  in-scope crate result even when the out-of-scope candidate is the stronger
  unscoped semantic match, which is direct evidence that hybrid fusion is built
  from already-scoped candidate sets
- if `get_context_specific_crate_scope_does_not_materialize_out_of_scope_ids`
  passes, then `get_context(...)` is not materializing out-of-scope nodes after
  retrieval
- if that `get_context(...)` test passes, then scoped context assembly only
  emits IDs and file paths from the allowed crate namespace, which is direct
  evidence that context assembly respects the shared retrieval scope
- if all four tests pass, then Phase 6 `C5` has direct witness evidence that
  BM25, dense, hybrid, and `get_context(...)` share one scope model and enforce
  it before truncation, fusion, and context materialization

Scope note:
- this witness set is sufficient for the named Phase 6 `C5` scope requirements
- `ploke_db_primary` was refreshed to
  `tests/backup_dbs/ploke_db_primary_2026-03-21.sqlite`, resolving the earlier
  `get_code_edges_regression` freshness blocker
- broader `cargo test -p ploke-tui --tests -- --nocapture` passed via
  sub-agent after the fixture refresh and witness additions, so Phase 6 `C5`
  now has both targeted witness coverage and broader regression validation

### Phase 7 `C6` namespace-scoped subset DB operations

Criterion text:
- requires namespace-scoped DB primitives before `/load crates ...` or
  `/workspace rm <crate>`
- requires subset operations to use explicit namespace authority rather than
  whole-DB replacement
- requires explicit conflict handling and post-mutation membership/search-state
  reconciliation

Current witness reasoning:
- [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
  now exposes `list_crate_context_rows(...)` and
  `collect_namespace_inventory(...)`, which build a DB-side inventory from
  `crate_context.namespace`, `file_mod.namespace`, and `syntax_edge`
  descendant closure
- [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
  now also exposes `remove_namespace(...)`, which retracts one namespace's
  descendant graph rows, removes its `crate_context` and `file_mod` roots,
  prunes `workspace_metadata.members`, retracts vector/BM25 rows for removed
  node ids, and explicitly invalidates active HNSW registration
- [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
  now also exposes `export_namespace(...)`, which builds a structured
  namespace-scoped artifact from explicit `crate_context`/`file_mod` authority
  and carries only the target crate's graph rows, search rows, embedding-set
  metadata, and pruned `workspace_metadata`
- [database.rs](/home/brasides/code/ploke/crates/ploke-db/src/database.rs)
  now also exposes `import_namespace(...)`, `NamespaceImportConflictReport`,
  and `NamespaceImportError`, which import one exported namespace artifact into
  a populated DB only after explicit duplicate namespace/name/root validation
- [2026-03-21_c6_subset_db_design_notes.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/2026-03-21_c6_subset_db_design_notes.md)
  records why this is the first safe seam for `C6` and why current whole-backup
  APIs are insufficient

What a passing witness proves:
- if `workspace_fixture_namespace_inventory_matches_crate_context_membership`
  passes, then the committed multi-member workspace fixture already exposes a
  per-crate namespace inventory in `ploke-db` derived from persisted DB
  authority rather than from crate-name lookup or cwd-derived behavior
- if `workspace_fixture_namespaces_remain_distinct_in_subset_inventory`
  passes, then the two crates in `ws_fixture_01_canonical` remain distinct at
  the namespace/root/descendant-id level inside the DB, which is the minimal
  precondition for later subset export/import/remove primitives
- if `remove_namespace_removes_only_target_namespace_and_invalidates_search_state`
  passes, then `ploke-db` can remove one explicit crate namespace from a loaded
  multi-member workspace fixture without behaving like whole-DB replacement
- if that `remove_namespace(...)` test passes, then the target namespace's
  `crate_context`, `file_mod` roots, rooted graph rows,
  `workspace_metadata.members` entry, vector rows, BM25 metadata rows, and
  active HNSW registration are reconciled to the post-mutation dataset while
  the sibling namespace remains intact
- if that `remove_namespace(...)` test passes, then the operation is also
  directly proven to act on the exact namespace inventory derived from
  `crate_context` plus `file_mod`, because the test asserts that the returned
  removed root modules and descendant-id set exactly match the pre-removal
  inventory for the requested namespace
- if `export_namespace_artifact_contains_only_target_namespace_rows` passes,
  then `ploke-db` can export one explicit crate namespace from a loaded
  multi-member workspace fixture without including sibling `crate_context`,
  `file_mod`, rooted graph rows, or search rows
- if that `export_namespace(...)` test passes, then the exported artifact is
  directly proven to preserve only the target namespace's `crate_context`,
  `file_mod` roots, rooted graph rows, vector rows, BM25 rows,
  `active_embedding_set` metadata, referenced `embedding_set` rows, and a
  pruned `workspace_metadata.members` list containing only the exported root
- if all four tests pass, then `C6` now has direct evidence for namespace
- authority plus real namespace-scoped removal and export primitives in
  `ploke-db`
- if `import_namespace_restores_exported_namespace_into_populated_db_and_invalidates_search_state`
  passes, then `ploke-db` can import one exported crate namespace into an
  already populated DB without behaving like whole-DB replacement, while
  restoring the target namespace's `crate_context`, rooted graph rows, vectors,
  BM25 rows, and merged `workspace_metadata.members`
- if that import test passes, then a live HNSW registration for the pre-import
  dataset is explicitly invalidated after the subset mutation, which is direct
  evidence that the DB primitive does not leave stale search readiness behind
- if `import_namespace_reports_duplicate_namespace_name_and_root_conflicts`
  passes, then duplicate namespace/name/root cases are surfaced explicitly by
  `import_namespace(...)` instead of being silently merged or replaced
- if all six tests pass, then `C6` now has direct DB-level evidence for
  namespace authority plus real namespace-scoped removal, export, import, and
  duplicate conflict validation in `ploke-db`

Scope note:
- this is still partial evidence only
- it now proves real subset removal, export, import, and duplicate conflict
  validation primitives, but it does not yet prove the later end-to-end
  TUI/runtime membership/focus/IO update path
- `C6` remains `in progress` until namespace-scoped subset export/import and
  explicit conflict validation exist end to end through the command/runtime
  surface

## Update rule

When a new acceptance-relevant test is added or changed:

1. add or update the row in the witness table
2. name the exact acceptance item it supports
3. explain why the test satisfies the criterion language
4. note any limits if the test is only partial evidence
5. remove stale reasoning if a test no longer matches the acceptance target
