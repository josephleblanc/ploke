# Phase 1 Readiness: Ingest Fixtures

Backlink: [docs/active/reports/2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)

This note covers the fixture and test readiness needed before Phase 1 of the workspace-planning task can start. The parser and transform code already support workspaces; the gap is fixture coverage that proves the behavior on committed workspace targets and backup DB snapshots.

## What Already Exists

- Workspace discovery and parsing already resolve `[workspace]` manifests, normalize member and exclude paths, support optional member selection, and return a `ParsedWorkspace` with workspace metadata plus per-crate parse output in [crates/ingest/syn_parser/src/lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L66).
- Workspace manifest lookup already walks parent directories and normalizes members/exclude paths into absolute workspace-relative paths in [crates/ingest/syn_parser/src/discovery/workspace.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L144).
- Workspace transform already writes `workspace_metadata` and then transforms each parsed crate graph in [crates/ingest/ploke-transform/src/transform/workspace.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L14).
- Full schema creation already includes `WorkspaceMetadataSchema` in [crates/ingest/ploke-transform/src/schema/mod.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/schema/mod.rs#L84).

## Current Fixture Targets

- The only real workspace fixture target currently present is [tests/fixture_workspace/ws_fixture_00/Cargo.toml](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_00/Cargo.toml#L1), which defines a single member, `fixture_toml`, plus `workspace.package`, `workspace.dependencies`, `workspace.lints`, and a profile block.
- That member crate is minimal and compileable in [tests/fixture_workspace/ws_fixture_00/fixture_toml/Cargo.toml](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_00/fixture_toml/Cargo.toml#L1) and [tests/fixture_workspace/ws_fixture_00/fixture_toml/src/lib.rs](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_00/fixture_toml/src/lib.rs#L1).
- [tests/fixture_workspace/ws_fixture_01/Cargo.toml](/home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_01/Cargo.toml) exists as an empty placeholder, so it is not a usable workspace fixture yet.
- The active backup DB fixtures on disk are only [tests/backup_dbs/fixture_nodes_canonical_2026-03-20.sqlite](/home/brasides/code/ploke/tests/backup_dbs/fixture_nodes_canonical_2026-03-20.sqlite), [tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite](/home/brasides/code/ploke/tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite), and [tests/backup_dbs/ploke_db_primary_2026-03-20.sqlite](/home/brasides/code/ploke/tests/backup_dbs/ploke_db_primary_2026-03-20.sqlite).

## Backup Fixture Gaps

- The registry in [crates/test-utils/src/fixture_dbs.rs](/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs#L145) is still crate-centric: the active fixtures point at `tests/fixture_crates/fixture_nodes` or `crates/ploke-db`, not at any workspace-member fixture set.
- The registry still contains non-active legacy/orphaned entries, but those are not Phase 1-ready workspace targets and should not be treated as coverage for workspace indexing.
- There is no registered backup fixture that proves a workspace parse can round-trip into a backup and be reloaded with workspace metadata intact.
- There is no backup fixture that exercises a multi-member workspace boundary, nested member paths, or workspace-level dependency metadata.

## Test Coverage

- Parser coverage is good for behavior, but it is still tempdir-driven. `parse_workspace` tests cover member normalization, missing selections, mixed selection mismatch details, empty selection, and DTO population in [crates/ingest/syn_parser/src/lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L392).
- `locate_workspace_manifest` and `WorkspaceMetaBuilder` are also covered with nested workspace path normalization in [crates/ingest/syn_parser/src/discovery/workspace.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/workspace.rs#L351).
- There is a real workspace fixture regression check already in [crates/ingest/syn_parser/src/discovery/mod.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs#L447), which verifies inherited workspace versioning and membership lookup against `ws_fixture_00`.
- Transform tests currently only smoke-test that the workspace transform runs without error; they do not assert the persisted `workspace_metadata` row contents or query the resulting DB state in [crates/ingest/ploke-transform/src/transform/workspace.rs](/home/brasides/code/ploke/crates/ingest/ploke-transform/src/transform/workspace.rs#L140).

## Phase 1 Readiness Gates

1. Add at least one committed workspace fixture with two or more members, including one nested member path, so workspace membership and path normalization are tested on disk rather than only in tempdirs.
2. Add a workspace-backed backup DB fixture generated from that workspace fixture, and register it in `crates/test-utils/src/fixture_dbs.rs` and [docs/testing/BACKUP_DB_FIXTURES.md](/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md#L63).
3. Add an assertion-level transform test that verifies the `workspace_metadata` relation contents after `transform_parsed_workspace`, not just that the transform returns `Ok(())`.
4. Keep backup import strict. Do not relax missing-relation or schema-drift handling to make the new fixture load succeed; Phase 1 readiness should come from real fixture regeneration, not permissive fallback.
5. Ensure `cargo xtask verify-backup-dbs` passes for the new fixture set before Phase 1 starts.

## Recommended Fixture Additions

- A multi-member workspace fixture under `tests/fixture_workspace/` with nested member paths and explicit `workspace.package` metadata.
- A canonical plain-backup snapshot generated from that workspace fixture and registered as an active backup DB fixture.
- A local-embeddings variant of the same workspace snapshot if Phase 1 will exercise end-to-end indexing on the workspace target.
- A transform assertion test that inspects `workspace_metadata` rows instead of only checking that insertion does not fail.

