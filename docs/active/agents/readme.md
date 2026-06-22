# Active Agent Docs

**Thread (2026-05-08):** Day-to-day topical notes that used to accumulate here now live under [`docs/archive/agents/2026-04/`](../../archive/agents/2026-04/README.md) and [`docs/archive/agents/2026-05/`](../../archive/agents/2026-05/README.md). This directory keeps only the shared **restart spine** plus living indexes.

If you read three things after a long break: [`CURRENT_FOCUS.md`](../CURRENT_FOCUS.md), [`workflow/handoffs/recent-activity.md`](../workflow/handoffs/recent-activity.md), and the in-repo Cursor plan for your current implementation thread (e.g. `.cursor/plans/prototype1_history_admitted_evaluation_*.plan.md`).

## Status guide

- **Current contract:** treat as a restart or implementation entry point, but verify live implementation claims against code and run artifacts before acting.
- **Active plan:** intent and next slices; not proof that the implementation already enforces the claimed invariant.
- **Review/report:** evidence from a bounded pass. Use it to route investigation, not as current authority after nearby code or handoffs have changed.
- **Historical baseline:** useful context only. Prefer newer handoffs, current code, and typed records when claims conflict.

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
- [`2026-05-12_broad-bounded-surface-transition-plan.md`](2026-05-12_broad-bounded-surface-transition-plan.md) — broad-harness transition plan, now annotated with the 2026-05-13 broad fanout implementation state and the remaining `SurfaceGrant -> CheckedProposal` proof direction.
- [`2026-05-12_loop-readiness-review-wave/`](2026-05-12_loop-readiness-review-wave/README.md) — review/report packet for loop readiness blockers, invariants, readiness, and style. Re-check current code before treating findings as still live.
- [`2026-05-12_tui-adapter-boundary-review/`](2026-05-12_tui-adapter-boundary-review/README.md) — review/report packet for adapter boundaries, prompt evidence/oracles, and patch-validation failures.
- [`2026-05-12_tui-adapter-implementation-wave/`](2026-05-12_tui-adapter-implementation-wave/README.md) — implementation-wave reports for review readiness, Rust style, structural invariants, and retainer notes.
- [`2026-05-09_ploke-records-protocol-handoff.md`](2026-05-09_ploke-records-protocol-handoff.md) — restart spine for the separate `ploke-records` / `ploke-tree` passive schema thread, including the no-public-opaque-JSON rule, real-run verification, and next protocol module split.
- [`2026-05-09_records-emission-clean-sweep-handoff.md`](2026-05-09_records-emission-clean-sweep-handoff.md) — restart spine for normalizing Prototype 1 persisted record emission around shared `ploke-records` schemas while preserving the clean-sweep rule against legacy projection-control reads.
- [`2026-05-12_agent-turn-record-projection-handoff.md`](2026-05-12_agent-turn-record-projection-handoff.md) — current restart packet for `agent-turn` persisted-schema ownership, including the actual `ploke-eval::runner` writer boundary and the new live-to-record projection rule.
- [`2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`](2026-05-12_hyperagents-broad-harness-orchestration-handoff.md) — orchestration plan for the HyperAgents-style broad harness, now annotated after the 2026-05-13 request-batch/headless-TUI fanout slice.
- [`2026-05-12_hyperagents-broad-harness-progress-summary.md`](2026-05-12_hyperagents-broad-harness-progress-summary.md) — user-facing summary of the BroadHarness orchestration wave, updated to mark admission-binding and broad complete-mode fanout as implemented while preserving remaining proof/durability gaps.
- [`2026-05-12_hyperagents-broad-harness-agent-handoff.md`](2026-05-12_hyperagents-broad-harness-agent-handoff.md) — restart handoff for the next BroadHarness orchestrator, including board state, accepted invariants, failed backend admission review, and the next safe lane split.
- [`2026-05-15_hyperagents-context-building-handoff.md`](2026-05-15_hyperagents-context-building-handoff.md) — restart note for the broad-harness prompt/RAG lesson, the context-mode Off adapter patch, and the next separate-worktree multi-generation smoke campaign.
- [`2026-05-16_prototype1-control-review.md`](2026-05-16_prototype1-control-review.md) — review of the new config-driven `prototype1-doctor` / `prototype1-continue` / `prototype1-step` control path, including resume correctness gaps around failed children, selection/handoff continuity, and setup/profile compatibility.
- [`2026-05-15_turn-boundary-apply-correctness-review.md`](2026-05-15_turn-boundary-apply-correctness-review.md) — review/report for turn-boundary apply correctness. Verify current apply/session code before using its findings as live bug status.
- [`2026-05-15_turn-boundary-apply-performance-review.md`](2026-05-15_turn-boundary-apply-performance-review.md) — review/report for turn-boundary apply performance and allocation risk.
- [`2026-05-09_run-playback-coarse-history-handoff.md`](2026-05-09_run-playback-coarse-history-handoff.md) — restart spine for the coarse sealed-History playback slice, including implemented playback vocabulary/projection work and the current real-run `SealedBlockRecord` schema mismatch blocker.
- [`2026-05-09_run-playback-typed-observability-plan.md`](2026-05-09_run-playback-typed-observability-plan.md) — plan for typed, iterable `RunPlayback` / `RunPlaybackRef` projections that can feed CLI debugging, `ploke-tree`, and a future UI/WebAssembly observability front end.
- [`2026-05-09_egui-wasm-observability-plan.md`](2026-05-09_egui-wasm-observability-plan.md) — plan for building an interactive egui/WASM frontend. Phases 0-1 done, Phase 2 (egui crate) next.
- [`2026-05-09_egui-wasm-observability-handoff.md`](2026-05-09_egui-wasm-observability-handoff.md) — cold-restart handoff for the egui/WASM observability thread.
- [`2026-05-11_063230_ploke-egui-dependency-leverage-review.md`](2026-05-11_063230_ploke-egui-dependency-leverage-review.md) — review/report for dependency leverage in `ploke-egui`; use as background for UI work, not as a benchmark result.
- [`2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](2026-05-11_ploke-egui-graph-import-boundary-handoff.md) — current restart spine for consolidating Prototype 1 run data around `ploke-tree::Graph` and keeping egui/browser surfaces as projections.
- [`2026-05-12_ploke-egui-artifact-view-handoff.md`](2026-05-12_ploke-egui-artifact-view-handoff.md) — restart packet for the current artifact-first egui graph thread: `ploke-tree::graph::Graph` stays authoritative, egui stays reference/derivation-only, and the first visual goal is a clean artifact tree with minimal patch labels and later typed drilldown.
- [`ploke-egui-task-readability/`](ploke-egui-task-readability/README.md) — coordination area for the artifact-view task-readability wave: sub-agent lanes, `xtask orchestrate` workflow, diagnostics plan, review policy, and inventory refresh scope.
- [`ploke-ui-task-readability/`](ploke-ui-task-readability/README.md) — current broader coordination area for the next UI readability wave: six-slot orchestration posture, mixed-board policy, stale-inventory refresh scope, worker packets, and the bridge from current `ploke-egui` readability to future typed drilldowns.
- [`ploke-ui-task-readability/artifact-tree-default/`](ploke-ui-task-readability/artifact-tree-default/README.md) — source-of-truth note for the current `ploke-egui` default tree graph: Artifact nodes, patch/derivation edges, History as reveal/highlight state, and debug/drilldown detail kept out of the default canvas.
- [`ploke-egui-candidate-comparison-reviews/`](ploke-egui-candidate-comparison-reviews/README.md) — review/report area for candidate-comparison UI and debug-output surfaces.
- [`skill-plans/`](skill-plans/README.md) — tracked improvement plans for Codex skills and their repo-local helper tools, starting with `orchestrator-conveyor` and `xtask orchestrate`.
- [`2026-05-11_ploke-tree-graph-ingestion-inventory.md`](2026-05-11_ploke-tree-graph-ingestion-inventory.md) — companion tracker mapping accepted typed-persistence surfaces to current `ploke-tree::Graph` ingestion status.
- [`2026-05-15_prototype1-record-pipeline-map.md`](2026-05-15_prototype1-record-pipeline-map.md) — end-to-end table for Prototype 1 persisted records from `ploke-eval` emitters through `ploke-records`, `ploke-tree`, and `ploke-egui`.
- [`2026-05-17-selection-metrics-graph-handoff.md`](2026-05-17-selection-metrics-graph-handoff.md) — restart note for selection-time `imp@k` metrics: `ploke-eval` computation/scoring, schema v4 persistence, passive `ploke-records` DTOs, and `ploke-tree::Graph` metric indexing before egui rendering.
- [`2026-05-21_runtime-playback-observability-handoff.md`](2026-05-21_runtime-playback-observability-handoff.md) — current design restart packet for graph-backed runtime playback, agent-turn timelines, shared playback cursor state, and implementation guardrails.
- [`2026-05-21_runtime-playback-implementation-handoff.md`](2026-05-21_runtime-playback-implementation-handoff.md) — current code-facing restart packet for the first `ploke-tree` runtime playback implementation slice and agent-turn drilldown follow-on.
- [`2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md`](2026-05-21_prototype1-self-edit-vs-eval-replay-orientation.md) — compact orientation for the replay CLI target, dirty vs clean benchmark workspace, and the distinction between self-edit patches and eval patches.
- [`2026-05-23_prototype1-closure-observability.md`](2026-05-23_prototype1-closure-observability.md) — active design note for surfacing baseline closure, eval, protocol, and tool-call evidence in `ploke-eval`, `ploke-tree`, `ploke-egui`, and repo-local agent skills.
- [`2026-05-24_prototype1-orchestrator-loop-notes.md`](2026-05-24_prototype1-orchestrator-loop-notes.md) — active orchestration notes for the long-running Prototype 1 diagnostic loop, including board setup, sub-agent dispatches, blockers, and workflow observations.
- [`2026-06-02_prototype1-state-loop-walkthrough/`](2026-06-02_prototype1-state-loop-walkthrough/README.md) — documentation packet for `ploke-eval loop prototype1-state`, including the detailed implementation walkthrough and companion [`terminology-conflicts.md`](2026-06-02_prototype1-state-loop-walkthrough/terminology-conflicts.md) audit.
- [`2026-06-02_prototype1-evalops-pipeline.md`](2026-06-02_prototype1-evalops-pipeline.md) — active plan for turning Prototype 1 campaign artifacts into deterministic inventory, dry-run board planning, coverage verification, quality-gated run reviews, synthesis, and backlog handoff.
- [`2026-06-04_scoring-mechanism-comparison-notes.md`](2026-06-04_scoring-mechanism-comparison-notes.md) — working notes comparing Prototype 1 selection/scoring with formal scoring mechanisms from the wiki campaign.
- [`2026-06-04_q2-ledger-vs-q1-findings.md`](2026-06-04_q2-ledger-vs-q1-findings.md) — review/report comparing Q2 canonical scoring ledgers against accepted Q1 shard findings.
- [`2026-06-05_selection-score-ploke-applicability.md`](2026-06-05_selection-score-ploke-applicability.md) — applicability map for which `ploke-selection-score` mechanisms are direct Ploke correlates versus analogy-only candidates.
- [`2026-06-05_ploke-eval-selection-score-metrics.md`](2026-06-05_ploke-eval-selection-score-metrics.md) — metrics needed in `ploke-eval` before selection-score mechanisms can become useful for reporting or selection.
- [`2026-06-06_parent-successor-handoff-regression/`](2026-06-06_parent-successor-handoff-regression/README.md) — active investigation log for repeated post-June-2 Prototype 1 failures before child self-evaluation / parent-successor handoff.
- [`2026-06-07_prototype1-live-run-status/`](2026-06-07_prototype1-live-run-status/README.md) — run-status report for `p1-historyfix-handoff-5g1x2-a2-20260607-155010`, including liveness, timings, protocol review evidence, selection scoring, and successor artifact-prep failure analysis.
- [`2026-06-07_prototype1-loop-termination-reports/`](2026-06-07_prototype1-loop-termination-reports/README.md) — current reports for recent Prototype 1 loop campaign termination status, timing evidence, and clean/failed/live classification.
- [`2026-06-08_01_prototype1-guided-edit-surface/`](2026-06-08_01_prototype1-guided-edit-surface/README.md) — ADR and changelog for the planned protocol-informed planning stage, graph-restricted edit surface, multi-instance loop support, Cozo records mirror, and selection-score missing-data tracking.
- [`2026-06-09_prototype1-state-api-surface-refactor-proposal.md`](2026-06-09_prototype1-state-api-surface-refactor-proposal.md) — design proposal for cleaning up the `tui_adapter` / `prototype1-state` candidate-generation API surface (collapse the three parallel edit-surface models, make the `Harness` trait the real port, structured boundary errors, `EmitRecord` choke point) so the five guided-edit-surface features become localized additions; includes GitNexus blast-radius and a staged PR sequence.
- [`2026-06-09_prototype1-state-api-surface-migration/`](2026-06-09_prototype1-state-api-surface-migration/README.md) — living migration log for the PR1–4 enabling cleanup; tracks per-PR internal API changes and downstream impact on `ploke-tree` / `ploke-egui` record serde shapes.
- [`2026-06-16_walk-server.md`](2026-06-16_walk-server.md) — cold-restart orientation for the debug-only local typestate walk server, including commands, module map, Zellij-inspired lifecycle choices, guardrails, known sharp edges, and next slices.
- [`2026-06-17_walk-command-guide.md`](2026-06-17_walk-command-guide.md) — short operator guide for trying the `walk` commands, including status/start/step/show/files/reset/stop examples and feedback prompts.
- [`2026-06-17_typestate-loop-driver-plan.md`](2026-06-17_typestate-loop-driver-plan.md) — active plan for promoting the Prototype 1 typestate pipeline into the durable loop driver while keeping `walk` as the operator interface.
- [`2026-06-17_typestate-loop-driver-worklog/`](2026-06-17_typestate-loop-driver-worklog/README.md) — running slice log for the typestate loop driver implementation, including test failures, recovery notes, and commit checkpoints.
- [`2026-06-20_p1-walk30g5c-live-run/`](2026-06-20_p1-walk30g5c-live-run/README.md) — live `loop walk` run packet for the 30-generation / 5-child Prototype 1 run, including tool-loop reviews, parent-cycle reviews, and ten-cycle synthesis reports.
- [`2026-06-22_prototype1-eval-store-data-model/`](2026-06-22_prototype1-eval-store-data-model/README.md) — active planning packet for Prototype 1 backend-agnostic persisted state: storage plan, typestate persistence ledger, passive mirror/trace inventory, and relational entity/fact/ref model.
- [`expected-failing-regression-tests.md`](expected-failing-regression-tests.md) — tracker for expected-failing regression tests and bug-pinning reproducers marked with `regr:<name>:DD-MM-YY_HH-MM`.
- [`ploke-tree-graph-ingestion/`](ploke-tree-graph-ingestion/README.md) — coordination packet for sub-agent lanes, edit boundaries, retry rules, and module organization around `ploke-tree::Graph` ingestion.
- [`2026-05-11_mbe-instance-patch-provenance/`](2026-05-11_mbe-instance-patch-provenance/README.md) — review/report area for MBE instance patch provenance, oracle targets, run-record lookup, and evidence files.
- [`2026-05-11_mbe-oracle-calibration-handoff.md`](2026-05-11_mbe-oracle-calibration-handoff.md) — restart packet for MBE/oracle calibration over Prototype 1 loop outputs, including gold/empty controls and candidate patch-export questions.
- [`2026-05-11_mbe-oracle-calibration-plan.md`](2026-05-11_mbe-oracle-calibration-plan.md) — active plan for child-owned MBE instance targets, patch projection provenance, cleanup, and oracle eligibility gating.
- [`run-reviews/`](run-reviews/README.md) — review/report area for specific Prototype 1 runs. Treat these as run-scoped evidence, not global current implementation state.
- [`run-review-negative-examples/`](run-review-negative-examples/README.md) — quarantined low-quality run-review outputs kept only for skill/prompt/adjudication improvement; do not cite as active run reviews.
- [`operator-logs/`](operator-logs/README.md) — fresh OODA-style operator logs for current-session Prototype 1 work. Use these to avoid relying on stale orchestration notes without revalidation.
- [`mode-survey/`](mode-survey/README.md) — review/report area for model/mode survey notes.
- [`death-by-slice/`](death-by-slice/README.md) — ledger for narrow implementation slices that later fail because policy, identity, authority, or state was not preserved across runtime boundaries.
- [`collaboration-incidents/`](collaboration-incidents/README.md) — durable ledger for agent-caused trust failures, frustration triggers, boundary overreach, secret-handling failures, model-communication failures, semantic naming failures, and workflow-order failures to see whether memory and workflow changes reduce repeats.
- [`merges/`](merges/README.md) — active merge journals and restart notes for branch integrations.
- [`open-questions.md`](open-questions.md) — agent-to-agent questions (not direct user prompts).
- [`notable-inconsistencies.md`](notable-inconsistencies.md) — durable inconsistencies worth tracking.

## Conventions

- When creating a **new** topical agent doc, prefer a dated name (`yyyy-mm-dd_…`) under `docs/active/agents/` only if it is genuinely part of the current restart spine; otherwise add it under the appropriate `docs/archive/agents/YYYY-MM/` bucket as work completes.
- Headers should include date, title, short description, and related planning files (with archive-resolved paths once moved).
- Workspace rule set: [`AGENTS.md`](../../../AGENTS.md).
