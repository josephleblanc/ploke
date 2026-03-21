# Workspace Progress Tracker 2026-03-20

Backlinks:
- [2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)
- [2026-03-20_workspaces_test_witnesses.md](/home/brasides/code/ploke/docs/active/agents/2026-03-workspaces/2026-03-20_workspaces_test_witnesses.md)

Use this as the current implementation status document for workspace rollout.
Update it whenever a readiness item or phase changes state. Keep entries short:
status, evidence, and next step.

Status legend: `not started` | `in progress` | `blocked` | `done`

## Current summary

- Overall status: `Phase 3 in progress`
- Current gate: readiness and Phases 1-2 are complete; Phase 3 `C2` has targeted witnesses but remains blocked on broader `ploke-tui` validation and a diagnosed relative-target regression in `test_update_embed`
- Cross-phase obligations to keep in view: `G1` coherent session state, `G2`
  explicit membership authority and manifest drift handling

## Global obligations

| Item | Status | Notes |
| --- | --- | --- |
| `G1` coherent session state across TUI/DB/search/IO | `not started` | Acceptance obligation spanning Phases 2-8 |
| `G2` explicit membership authority and manifest drift | `not started` | Acceptance obligation spanning Phases 2-8 |

## Readiness gate

| Item | Status | Evidence / next step |
| --- | --- | --- |
| `R1` committed multi-member workspace fixture exists | `done` | `ws_fixture_01` is now a committed two-member workspace with nested member path `nested/member_nested`, and parser/discovery tests consume it directly |
| `R2` registered workspace backup fixture exists | `done` | Added active fixture `ws_fixture_01_canonical` backed by `tests/backup_dbs/ws_fixture_01_canonical_2026-03-21.sqlite`, registered it in the shared fixture registry/docs, and verified it with `cargo xtask verify-backup-dbs --fixture ws_fixture_01_canonical` |
| `R3` `workspace_metadata` transform is asserted | `done` | Added `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture`, which parses committed fixture `ws_fixture_01`, transforms it into a fresh DB, queries `workspace_metadata`, and asserts `id`, `namespace`, `root_path`, `members`, `exclude`, `resolver`, and `package_version` |
| `R4` strict backup verification passes | `done` | Full `cargo xtask verify-backup-dbs` passed for all active registered fixtures, including `ws_fixture_01_canonical` with strict roundtrip verification |

## Phase status

| Phase | Status | Exit target / next step |
| --- | --- | --- |
| Phase 1 `C0` fixture and ingestion baseline | `done` | Registered fixture witness proves restored `workspace_metadata.members` equals restored `crate_context.root_path` membership and restored identity matches `WorkspaceId::from_root_path(...)` |
| Phase 2 `C1` explicit loaded-workspace state in `ploke-tui` | `done` | `SystemStatus` now carries explicit `LoadedWorkspaceState`, path policy roots come from loaded membership rather than focus alone, and DB-backed restore hydrates that state from `workspace_metadata` |
| Phase 3 `C2` manifest-driven indexing | `in progress` | Targeted `C2` witnesses pass for revised target resolution and failure-state preservation, but broader `ploke-tui` validation is still red: `app_state::database::test::test_update_embed` now fails fast with `initial IndexWorkspace emitted AppEvent::Error ... Failed to normalize target path ... crates/ploke-tui/tests/fixture_crates/fixture_update_embed`; fix the relative-target regression before marking Phase 3 done |
| Phase 4 `C3` workspace status and update | `not started` | Make stale detection and update behavior operate per loaded crate |
| Phase 5 `C4` workspace save/load registry | `not started` | Restore by workspace identity with consistent snapshot metadata |
| Phase 6 `C5` shared retrieval scope model | `not started` | Enforce scope before dense/BM25/hybrid truncation and fusion |
| Phase 7 `C6` namespace-scoped subset DB operations | `not started` | Add explicit export/import/remove primitives before subset commands |
| Phase 8 `C7` workspace-aware tools with strict edit safety | `not started` | Expand read/context behavior without widening edit permissions |

## Test matrix

| Acceptance item | Status | Witness tests | Notes |
| --- | --- | --- | --- |
| `R1` committed multi-member workspace fixture exists | `done` | `parse_workspace_committed_fixture_uses_multi_member_workspace`; `committed_workspace_fixture_locates_nested_members` | Witness reasoning is recorded in `2026-03-20_workspaces_test_witnesses.md` |
| `R2` registered workspace backup fixture exists | `done` | `backup_db_fixture_lookup_returns_registered_workspace_fixture`; `workspace_backup_fixture_loads_via_registry_and_has_workspace_metadata` | Scoped verification evidence: `cargo xtask verify-backup-dbs --fixture ws_fixture_01_canonical` |
| `R3` `workspace_metadata` transform is asserted | `done` | `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture` | Verified by sub-agent test run: `cargo test -p ploke-transform transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture -- --nocapture` |
| `R4` strict backup verification passes | `done` | none; command evidence only | Verified by sub-agent run: `cargo xtask verify-backup-dbs` passed for `fixture_nodes_canonical`, `fixture_nodes_local_embeddings`, `ploke_db_primary`, and `ws_fixture_01_canonical` |
| Phase 1 `C0` workspace snapshot coherence has fixture-backed witness evidence | `done` | `workspace_backup_fixture_roundtrips_coherent_membership_and_identity` | Verified by sub-agent test run: `cargo test -p ploke-test-utils workspace_backup_fixture_roundtrips_coherent_membership_and_identity -- --nocapture` |
| Phase 2 `C1` explicit loaded-workspace state in `ploke-tui` | `done` | `loaded_workspace_membership_controls_focus_and_path_policy`; `set_focus_from_root_preserves_existing_loaded_workspace_membership`; `workspace_restore_assigns_loaded_workspace_membership_from_db` | Verified by sub-agent test runs in `ploke-tui`; witness reasoning is recorded in `2026-03-20_workspaces_test_witnesses.md` |
| Phase 3 `C2` manifest-driven indexing | `in progress` | `resolve_index_target_prefers_crate_root_when_pwd_is_crate_root`; `resolve_index_target_finds_workspace_when_pwd_is_not_crate_root`; `resolve_index_target_reports_missing_crate_or_workspace`; `index_workspace_resolves_ancestor_workspace_from_nested_path`; `index_workspace_failure_keeps_previous_loaded_workspace_state` | Targeted `C2` witnesses passed via sub-agent, but broader validation remains blocked by `app_state::database::test::test_update_embed`, which now fails fast after test hardening and proves the current tree emits an early `AppEvent::Error` for a wrongly normalized relative target path |

## Handoff Notes

Use this section only for compact-handoff context that should survive a
conversation compaction. Keep it short and replace it wholesale when it is
updated.

- Current implementation state: readiness and Phases 1-2 are complete; Phase 3
  `C2` remains `in progress`.
- Targeted `C2` witnesses still pass, but the broader blocker is now diagnosed
  rather than merely described as a stall.
- `app_state::database::test::test_update_embed` was hardened to subscribe
  before `IndexWorkspace` and fail fast on `AppEvent::Error`; on the current
  tree it now proves an early error from wrong relative-path normalization:
  `/home/brasides/code/ploke/crates/ploke-tui/tests/fixture_crates/fixture_update_embed`
  does not exist.
- The regression source is the uncommitted `index_workspace(...)` refactor in
  [indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs),
  which stopped anchoring relative targets to app-state authority and instead
  re-resolves them from process cwd.
- ADR draft
  [ADR-023-refactor-crate-focus.md](/home/brasides/code/ploke/docs/design/adrs/proposed/ADR-023-refactor-crate-focus.md)
  was audited against current code (`a4f139ba`, 2026-03-21) and now records
  that `crate_focus` is already `Option<CrateId>` with derived root accessors,
  but remains semantically overloaded.
- Next target after compaction: fix the `C2` relative-target regression in
  `index_workspace(...)`, keep the new fast-fail behavior in `test_update_embed`,
  then rerun broader `ploke-tui` validation before advancing to `C3`.

## Update rule

When work starts or lands:

1. change the item status
2. add the concrete evidence that changed it
3. update the next blocking step
4. update the test matrix if witness coverage changed
5. update `2026-03-20_workspaces_test_witnesses.md` when a test is added,
   replaced, or re-scoped as acceptance evidence
6. when preparing for conversation compaction, update `Handoff Notes` with the
   minimum current-state context needed for the next agent to resume
7. when asked to update `Handoff Notes`, replace the previous handoff content
   instead of appending to it

If a phase is partially landed, keep the phase `in progress` until its linked
acceptance criterion is satisfied.
