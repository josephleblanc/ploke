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

## Update rule

When a new acceptance-relevant test is added or changed:

1. add or update the row in the witness table
2. name the exact acceptance item it supports
3. explain why the test satisfies the criterion language
4. note any limits if the test is only partial evidence
5. remove stale reasoning if a test no longer matches the acceptance target
