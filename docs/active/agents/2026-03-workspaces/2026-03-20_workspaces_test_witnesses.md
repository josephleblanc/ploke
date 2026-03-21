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

## Update rule

When a new acceptance-relevant test is added or changed:

1. add or update the row in the witness table
2. name the exact acceptance item it supports
3. explain why the test satisfies the criterion language
4. note any limits if the test is only partial evidence
5. remove stale reasoning if a test no longer matches the acceptance target
