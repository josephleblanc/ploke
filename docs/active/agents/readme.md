# Active Agent Docs

**Thread (2026-05-08):** Day-to-day topical notes that used to accumulate here now live under [`docs/archive/agents/2026-04/`](../../archive/agents/2026-04/README.md) and [`docs/archive/agents/2026-05/`](../../archive/agents/2026-05/README.md). This directory keeps only the shared **restart spine** plus living indexes.

If you read three things after a long break: [`CURRENT_FOCUS.md`](../CURRENT_FOCUS.md), [`workflow/handoffs/recent-activity.md`](../workflow/handoffs/recent-activity.md), and the in-repo Cursor plan for your current implementation thread (e.g. `.cursor/plans/prototype1_history_admitted_evaluation_*.plan.md`).

## Start here

- [`CURRENT_FOCUS.md`](../CURRENT_FOCUS.md) — primary restart pointer (code + workflow thread).
- [`workflow/handoffs/recent-activity.md`](../workflow/handoffs/recent-activity.md) — rolling activity log (links into archive where needed).
- [`workflow/handoffs/2026-04-17_protocol-design-reset.md`](../workflow/handoffs/2026-04-17_protocol-design-reset.md) — compact eval/protocol design pivot (historical baseline; follow-on work is often in archive + `ploke-eval` inner handoff).

## Archived topical material (trajectory)

- **April 2026 (eval infra, protocol checkpoints, History/Crown review trees, phase-1 audit):** [`docs/archive/agents/2026-04/`](../../archive/agents/2026-04/README.md) — includes e.g. [`2026-04-12_eval-infra-sprint/`](../../archive/agents/2026-04/2026-04-12_eval-infra-sprint/README.md), [`2026-04-17_eval-failure-and-protocol-audit/`](../../archive/agents/2026-04/2026-04-17_eval-failure-and-protocol-audit/README.md), and `history-*-review-*` / `prototype1-architecture-gap-audit-*` folders.
- **May 2026 (edit surface + Prototype 1 handoffs/reviews):** [`docs/archive/agents/2026-05/`](../../archive/agents/2026-05/README.md) — `2026-05-07-*` reviews, handoffs, and related History startup reviews.

The former `2026-05-06-prototype1-*` execution-surface inventory links that appeared in older TOCs are **not present in this tree**; use archive + git history if you need that exact filename set.

## Still in this folder

- [`2026-05-08_bounded-edit-surface-handoff.md`](2026-05-08_bounded-edit-surface-handoff.md) — restart spine for the current bounded edit-surface / parent-planning thread, including authoritative docs, blocker status, known code audits, and next questions.
- [`2026-05-08_bounded-edit-surface-implementation-orientation.md`](2026-05-08_bounded-edit-surface-implementation-orientation.md) — short operational packet for sub-agents implementing the bounded edit-surface plan, with core docs, invariants, task-stack ids, and slice prompt pattern.
- [`2026-05-12_broad-bounded-surface-transition-plan.md`](2026-05-12_broad-bounded-surface-transition-plan.md) — concrete plan for turning the current broad-harness prompt path into a typed `SurfaceGrant -> HarnessRequest -> CheckedProposal -> ChildPlan` transition, with module homes and splice tests.
- [`2026-05-09_ploke-records-protocol-handoff.md`](2026-05-09_ploke-records-protocol-handoff.md) — restart spine for the separate `ploke-records` / `ploke-tree` passive schema thread, including the no-public-opaque-JSON rule, real-run verification, and next protocol module split.
- [`2026-05-09_records-emission-clean-sweep-handoff.md`](2026-05-09_records-emission-clean-sweep-handoff.md) — restart spine for normalizing Prototype 1 persisted record emission around shared `ploke-records` schemas while preserving the clean-sweep rule against legacy projection-control reads.
- [`2026-05-12_agent-turn-record-projection-handoff.md`](2026-05-12_agent-turn-record-projection-handoff.md) — current restart packet for `agent-turn` persisted-schema ownership, including the actual `ploke-eval::runner` writer boundary and the new live-to-record projection rule.
- [`2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`](2026-05-12_hyperagents-broad-harness-orchestration-handoff.md) — active post-compaction orchestration plan for moving from deterministic edit-surface smoke runs to a HyperAgents-style broad harness with typed request/result/admission, isolated candidate workspaces, and History-backed authority.
- [`2026-05-12_hyperagents-broad-harness-progress-summary.md`](2026-05-12_hyperagents-broad-harness-progress-summary.md) — user-facing summary of the BroadHarness orchestration wave: reviewed request/publication fixes, structural parent request identity, failed backend review, and remaining authority-binding blocker.
- [`2026-05-12_hyperagents-broad-harness-agent-handoff.md`](2026-05-12_hyperagents-broad-harness-agent-handoff.md) — restart handoff for the next BroadHarness orchestrator, including board state, accepted invariants, failed backend admission review, and the next safe lane split.
- [`2026-05-09_run-playback-coarse-history-handoff.md`](2026-05-09_run-playback-coarse-history-handoff.md) — restart spine for the coarse sealed-History playback slice, including implemented playback vocabulary/projection work and the current real-run `SealedBlockRecord` schema mismatch blocker.
- [`2026-05-09_run-playback-typed-observability-plan.md`](2026-05-09_run-playback-typed-observability-plan.md) — plan for typed, iterable `RunPlayback` / `RunPlaybackRef` projections that can feed CLI debugging, `ploke-tree`, and a future UI/WebAssembly observability front end.
- [`2026-05-09_egui-wasm-observability-plan.md`](2026-05-09_egui-wasm-observability-plan.md) — plan for building an interactive egui/WASM frontend. Phases 0-1 done, Phase 2 (egui crate) next.
- [`2026-05-09_egui-wasm-observability-handoff.md`](2026-05-09_egui-wasm-observability-handoff.md) — cold-restart handoff for the egui/WASM observability thread.
- [`2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](2026-05-11_ploke-egui-graph-import-boundary-handoff.md) — current restart spine for consolidating Prototype 1 run data around `ploke-tree::Graph` and keeping egui/browser surfaces as projections.
- [`2026-05-12_ploke-egui-artifact-view-handoff.md`](2026-05-12_ploke-egui-artifact-view-handoff.md) — restart packet for the current artifact-first egui graph thread: `ploke-tree::graph::Graph` stays authoritative, egui stays reference/derivation-only, and the first visual goal is a clean artifact tree with minimal patch labels and later typed drilldown.
- [`ploke-egui-task-readability/`](ploke-egui-task-readability/README.md) — coordination area for the artifact-view task-readability wave: sub-agent lanes, `xtask orchestrate` workflow, diagnostics plan, review policy, and inventory refresh scope.
- [`2026-05-11_ploke-tree-graph-ingestion-inventory.md`](2026-05-11_ploke-tree-graph-ingestion-inventory.md) — companion tracker mapping accepted typed-persistence surfaces to current `ploke-tree::Graph` ingestion status.
- [`ploke-tree-graph-ingestion/`](ploke-tree-graph-ingestion/README.md) — coordination packet for sub-agent lanes, edit boundaries, retry rules, and module organization around `ploke-tree::Graph` ingestion.
- [`2026-05-11_mbe-oracle-calibration-handoff.md`](2026-05-11_mbe-oracle-calibration-handoff.md) — restart packet for MBE/oracle calibration over Prototype 1 loop outputs, including gold/empty controls and candidate patch-export questions.
- [`2026-05-11_mbe-oracle-calibration-plan.md`](2026-05-11_mbe-oracle-calibration-plan.md) — active plan for child-owned MBE instance targets, patch projection provenance, cleanup, and oracle eligibility gating.
- [`death-by-slice/`](death-by-slice/README.md) — ledger for narrow implementation slices that later fail because policy, identity, authority, or state was not preserved across runtime boundaries.
- [`open-questions.md`](open-questions.md) — agent-to-agent questions (not direct user prompts).
- [`notable-inconsistencies.md`](notable-inconsistencies.md) — durable inconsistencies worth tracking.

## Conventions

- When creating a **new** topical agent doc, prefer a dated name (`yyyy-mm-dd_…`) under `docs/active/agents/` only if it is genuinely part of the current restart spine; otherwise add it under the appropriate `docs/archive/agents/YYYY-MM/` bucket as work completes.
- Headers should include date, title, short description, and related planning files (with archive-resolved paths once moved).
- Workspace rule set: [`AGENTS.md`](../../../AGENTS.md).
