# Eval Infra Sprint Control Plane

**Date:** 2026-04-12
**Task Title:** Eval infrastructure sprint control plane
**Task Description:** Active orchestration document for closing the Phase 1 P0 audit gaps in replay, inspection, and setup recording without losing design intent or workflow context.
**Related Planning Files:** `docs/active/plans/evals/eval-design.md`, `docs/active/plans/evals/phased-exec-plan.md`, `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/README.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/phase-1-audit/AUDIT_SYNTHESIS.md`, `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md`

## Status

- active_since: 2026-04-12
- owning_branch: refactor/tool-calls
- review_cadence: update after every packet disposition or workflow-pointer change
- update_trigger: update when packet state changes, dependencies shift, or a permission gate blocks progress

## Purpose

This is the active control-plane document for the eval-infra sprint. It
supersedes the Phase 1 audit synthesis as the active planning doc while keeping
that audit as the evidence base and gap inventory.

Sprint objective:

- close the P0 audit gaps blocking trustworthy replay and inspection
- keep all work aligned with `eval-design.md` Phase 1 expectations
- run active sidecar lanes in parallel for code quality, longitudinal metrics, and workflow/process adherence
- avoid new false confidence by requiring packetized acceptance and explicit evidence

## Lane Model

This sprint uses one blocking primary lane plus active non-blocking sidecar
lanes. Sidecar lanes are not optional backlog; they are in-scope concurrent work
that should continue when they do not interfere with the primary lane.

### Primary Lane

- lane_id: `PRIMARY-P0`
- purpose: close the replay/inspection/setup P0 gaps from the audit
- blocking: yes
- review cadence: after every packet disposition

### Sidecar Lanes

- lane_id: `S1-COHERENCE`
  - purpose: audit `ploke-eval` API shape, code quality, repetition, and trivially passing test risk
  - blocking: no, but active
  - review cadence: after every primary-lane packet disposition until the first audit gap list stabilizes
- lane_id: `S2-LONGITUDINAL`
  - purpose: define and begin a central change-over-time reporting layer for run metrics and validity health
  - blocking: no, but active
  - review cadence: weekly synthesis plus a lightweight check when a new result batch lands
- lane_id: `S3-META-PROCESS`
  - purpose: audit workflow, skills, and orchestration adherence so process drift is visible and correctable
  - blocking: no, but active
  - review cadence: weekly plus a spot-check after each major packet batch

## Primary Lane Acceptance Boundary

The blocking lane is complete only when all of the following are true:

1. `RunRecord.phases.setup` is populated for successful runs and no longer emits `null` by default.
2. `SetupPhase` records which crates were indexed in a queryable, serialized form.
3. Historical DB queries can execute against a recorded turn timestamp.
4. `turn.db_state().lookup(name)` exists and answers symbol existence at the turn snapshot.
5. `replay_query(turn, query)` exists and can execute an arbitrary query against the recorded turn snapshot.
6. The workflow record and active focus docs reflect the actual sprint state rather than stale "Phase 1 complete" claims.

## Sidecar Milestones

These do not block the primary lane, but they are expected outputs for the
current sprint window and should not disappear from the active program:

1. `S1A` produces a first-pass `ploke-eval` coherence audit with prioritized findings.
2. `S2A` produces a first-pass longitudinal metrics/reporting design with a proposed central location and minimal metric set.
3. `S3A` produces a first-pass workflow and skills adherence audit with concrete process/documentation adjustments.

## Audit-Fed Packet Additions

The first sidecar reports fed additional packet needs back into the programme:

1. `P0F` addresses turn-record fidelity and replay-state reconstruction inside `ploke-eval`.
2. `S2B` turns the accepted longitudinal-metrics design into a central workflow artifact.
3. `S3B` tightens reusable control-plane and handoff/report templates so the protocol is easier to follow consistently.

Additional sidecar planning notes from restart review:

4. `S2C` explores a lightweight ingestion and auto-rollup path so the longitudinal metrics ledger can update from newly available formal runs.
5. `S3C` inventories meta-level workflow/process evidence sources and frames exploratory hypotheses for protocol effectiveness.
6. `S1B` promotes the accepted coherence audit into a bounded `ploke-eval` cleanup track now that the P0 lane is accepted.
7. `S1C` audits the inspect-oriented `ploke-eval` CLI as a real UX/bootstrap surface for quick internal eval checks.

Pre-implementation survey addition from restart review:

6. `P0C0` surveys the existing query-builder and raw-query surface before committing to the `P0C` historical-query implementation path.

## Current-State Table

| task_id | lane | status | owner | layer_workstream | packet_link | latest_report_link | next_action |
|---------|------|--------|-------|------------------|-------------|--------------------|-------------|
| `P0A` | `PRIMARY-P0` | `accepted` | worktree | `A5` | [P0A](./2026-04-12_P0A_setupphase-schema.md) | [scope review](./2026-04-12_P0AB_scope-separation-review.md) | keep future setup UX questions separate from replay/query acceptance |
| `P0B` | `PRIMARY-P0` | `accepted` | worktree | `A5` | [P0B](./2026-04-12_P0B_setupphase-capture.md) | [scope review](./2026-04-12_P0AB_scope-separation-review.md) | keep failure-path setup questions separate from replay/query acceptance |
| `P0C0` | `PRIMARY-P0` | `accepted` | Russell | `A5` | [P0C0](./2026-04-12_P0C0_query-builder-survey.md) | [report](./2026-04-12_P0C0_query-builder-survey-report.md) | use the narrow `raw_query_at_timestamp()` / `DbState` path for `P0C`, not `QueryBuilder` |
| `P0C` | `PRIMARY-P0` | `accepted` | Poincare | `A5` | [P0C](./2026-04-12_P0C_historical-db-query-support.md) | [report](./2026-04-12_P0C_report.md) | keep acceptance scoped to the explicit `raw_query_at_timestamp()` contract/test slice, not the whole dirty `database.rs` diff |
| `P0D` | `PRIMARY-P0` | `accepted` | Fermat | `A5` | [P0D](./2026-04-12_P0D_turn-db-state-lookup.md) | [verification report](./2026-04-12_P0DE_verification_report.md) | keep future lookup-hardening scoped to ambiguity handling and richer matching, not this acceptance slice |
| `P0E` | `PRIMARY-P0` | `accepted` | Fermat | `A5` | [P0E](./2026-04-12_P0E_replay-query.md) | [verification report](./2026-04-12_P0DE_verification_report.md) | keep future replay-hardening scoped to differential evidence and error taxonomy cleanup, not this acceptance slice |
| `P0F` | `PRIMARY-P0` | `accepted` | worktree | `A5` | [P0F](./2026-04-12_P0F_turn-record-fidelity.md) | [retry report](./2026-04-12_P0F_retry-report.md) | keep future replay-fidelity follow-up scoped to real turn timestamps rather than reopening this packet |
| `S1A` | `S1-COHERENCE` | `accepted` | Galileo | `A5` | [S1A](./2026-04-12_S1A_ploke-eval-coherence-audit.md) | [report](./2026-04-12_S1A_ploke-eval-coherence-audit-report.md) | use findings to drive `P0F` and later `ploke-eval` cleanup packets |
| `S1B` | `S1-COHERENCE` | `ready` | unassigned | `A5` | [S1B](./2026-04-12_S1B_ploke-eval-cleanup.md) | none | promote the accepted coherence audit into a bounded cleanup implementation track inside `ploke-eval` |
| `S1C` | `S1-COHERENCE` | `ready` | unassigned | `A5` | [S1C](./2026-04-12_S1C_inspect-cli-ux-audit.md) | none | audit the inspect CLI as a quick-touch internal UX/bootstrap surface and frame concrete follow-up work |
| `S2A` | `S2-LONGITUDINAL` | `accepted` | Goodall | `H0` | [S2A](./2026-04-12_S2A_longitudinal-metrics-design.md) | [report](./2026-04-12_S2A_longitudinal-metrics-report.md) | create the central metrics ledger packet and define formulas/denominators |
| `S3A` | `S3-META-PROCESS` | `accepted` | Dewey | `A4` | [S3A](./2026-04-12_S3A_workflow-adherence-audit.md) | [report](./2026-04-12_S3A_workflow-adherence-audit-report.md) | create follow-up packet(s) for control-plane template and handoff/report template tightening |
| `S2B` | `S2-LONGITUDINAL` | `accepted` | Peirce | `H0` | [S2B](./2026-04-12_S2B_longitudinal-metrics-ledger.md) | [report](./2026-04-12_S2B_longitudinal-metrics-ledger-report.md) | backfill a small sample of formal runs and then open capture/aggregation follow-up work |
| `S3B` | `S3-META-PROCESS` | `accepted` | Franklin | `A4` | [S3B](./2026-04-12_S3B_control-plane-and-handoff-template-tightening.md) | [report](./2026-04-12_S3B_control-plane-and-handoff-template-tightening-report.md) | decide whether a short orchestration skill note is still needed after one more sprint |
| `S2C` | `S2-LONGITUDINAL` | `ready` | unassigned | `H0` | [S2C](./2026-04-12_S2C_metrics-ingestion-bootstrap.md) | none | define the lightest-weight discovery, storage, and roll-up path from formal runs into the central metrics ledger |
| `S3C` | `S3-META-PROCESS` | `ready` | unassigned | `A4` | [S3C](./2026-04-12_S3C_meta-observability-inventory.md) | none | inventory available workflow/process evidence sources and propose a small exploratory hypothesis set |

## Dependency Notes

- `P0A` and `P0B` are fully inside `crates/ploke-eval/` and can proceed without additional permission.
- `P0C0` is a read-only survey packet meant to choose the right `P0C` shape before implementation touches `crates/ploke-db/`.
- `P0C` is the main permission gate because it touches `crates/ploke-db/`.
- `P0D` and `P0E` are intentionally held until the historical-query mechanism is explicit, to avoid speculative APIs.
- `P0F` is a primary-lane packet created from the accepted `S1A` audit because turn-record fidelity is a prerequisite for trustworthy replay even before new query helpers are accepted.
- The current local worktree already mixes `P0A/P0B` with partial `P0D/P0E` implementation; use the verification note to keep acceptance boundaries explicit.
- `P0B` should not broaden into manifest or metrics work; keep it strictly on setup capture.
- `P0C0` exists because the current builder surface appears partial and a lot of live queries bypass it; we should choose whether to extend it, wrap it, or bypass it deliberately before landing `P0C`.
- `S1A`, `S2A`, and `S3A` are read-heavy sidecar packets and should not block primary-lane execution.
- `S1B` should treat accepted P0 behavior as substrate and focus on coherence/cleanup rather than reopening replay/inspection correctness unless it finds concrete contradicting evidence.
- `S1C` should evaluate the inspect CLI as a user/agent surface, not just as code structure, and should prefer representative tasks/questions over abstract CLI critique.
- `S2C` should stay design/bootstrap scoped and avoid sneaking in an ingestion implementation before the current sprint decides the desired durable format.
- `S3C` is explicitly exploratory: it should inventory signals and frame hypotheses, not turn into a broad process rewrite.
- Sidecar findings should feed back into the control plane as new packets or prioritization notes, not silent background context.

## Immediate Orchestrator Guidance

1. The blocking primary P0 lane is accepted end-to-end: `P0A`, `P0B`, `P0C0`, `P0C`, `P0D`, `P0E`, and `P0F` all have explicit reports and acceptance boundaries.
2. Keep the pre/post workspace baseline as the regression reference for the `P0C` slice: no timeout, no new failures, same two pre-existing `ploke-tui` integration failures before and after.
3. Do not reopen the accepted P0 packets casually; create a narrow hardening packet if lookup ambiguity, replay differential evidence, or error-taxonomy cleanup is desired.
4. Promote the next active work deliberately from the sidecar queue: `S1B`, `S1C`, `S2C`, and `S3C` are all now explicit ready packets.
5. Move to the next eval-design phase only after choosing whether any of those sidecar packets should run first.
6. Treat `S2B` and `S3B` as accepted and keep their artifacts in the restart path.

## Resume Path

If replacing the orchestrator:

1. read `docs/active/CURRENT_FOCUS.md`
2. read `docs/active/workflow/README.md`
3. read `docs/active/workflow/readiness-status.md`
4. read `docs/active/workflow/handoffs/recent-activity.md`
5. read `docs/active/plans/evals/phased-exec-plan.md`
6. read this file
7. open the highest-priority non-accepted packet in the current-state table
