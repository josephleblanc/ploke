# Workspace Progress Tracker 2026-03-20

Backlinks:
- [2026-03-20_workspaces_implementation_plan.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_implementation_plan.md)
- [2026-03-20_workspaces_acceptance_criteria.md](/home/brasides/code/ploke/docs/active/reports/2026-03-20_workspaces_acceptance_criteria.md)

Use this as the current implementation status document for workspace rollout.
Update it whenever a readiness item or phase changes state. Keep entries short:
status, evidence, and next step.

Status legend: `not started` | `in progress` | `blocked` | `done`

## Current summary

- Overall status: `planning complete`, implementation not yet started
- Current gate: readiness items `R1-R4` must be complete before Phase 1 runtime
  implementation begins
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
| `R1` committed multi-member workspace fixture exists | `not started` | Add committed fixture under `tests/fixture_workspace/` with multiple members and one nested path |
| `R2` registered workspace backup fixture exists | `not started` | Add canonical workspace backup fixture and register it in fixture registry and docs |
| `R3` `workspace_metadata` transform is asserted | `not started` | Replace smoke-only coverage with assertion-level transform test |
| `R4` strict backup verification passes | `not started` | Run `cargo xtask verify-backup-dbs` after fixture registration work lands |

## Phase status

| Phase | Status | Exit target / next step |
| --- | --- | --- |
| Phase 1 `C0` fixture and ingestion baseline | `blocked` | Unblock by finishing `R1-R4`, then prove `workspace_metadata.members` and restored `crate_context` membership agree |
| Phase 2 `C1` explicit loaded-workspace state in `ploke-tui` | `not started` | Introduce first-class loaded workspace state and atomic IO-root updates |
| Phase 3 `C2` manifest-driven indexing | `not started` | Replace crate-root inference with manifest-driven workspace indexing |
| Phase 4 `C3` workspace status and update | `not started` | Make stale detection and update behavior operate per loaded crate |
| Phase 5 `C4` workspace save/load registry | `not started` | Restore by workspace identity with consistent snapshot metadata |
| Phase 6 `C5` shared retrieval scope model | `not started` | Enforce scope before dense/BM25/hybrid truncation and fusion |
| Phase 7 `C6` namespace-scoped subset DB operations | `not started` | Add explicit export/import/remove primitives before subset commands |
| Phase 8 `C7` workspace-aware tools with strict edit safety | `not started` | Expand read/context behavior without widening edit permissions |

## Update rule

When work starts or lands:

1. change the item status
2. add the concrete evidence that changed it
3. update the next blocking step

If a phase is partially landed, keep the phase `in progress` until its linked
acceptance criterion is satisfied.
