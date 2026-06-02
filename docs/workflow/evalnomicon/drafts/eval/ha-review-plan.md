Verified surface: code/doc review only; no tests or live run were executed. I used sub-agents for docs, broad-harness code, and eval/split surfaces; the selection/archive explorer timed out, so I verified that lane locally against the code.

**Verdict**
The gap review is directionally strong, but it needs one important update: Prototype 1 is no longer merely “generation-local selection plus future archive traversal.” Current code has History-backed traversal wired into active selection, with all-admitted History plus current-generation candidates and a `score_child_prop` strategy. See [successor_selection/mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/successor_selection/mod.rs:8), [profile.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/profile.rs:313), and [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:8612).

**2026-05-17 Update: Selection-Time Metrics**
The first `imp@k` slice has now landed as selection-time evidence instead of a Graph-only or offline report. `ploke-eval::successor_selection::metrics` computes a metric set after History and current-generation candidates are merged and before successor selection chooses; `SelectionDecisionEntry` seals that evidence as schema v4; `ploke-records` mirrors the passive DTOs; and `ploke-tree::Graph` indexes the persisted metric set and candidate rows for downstream UI work.

The next loop run from a build that includes this change should create reusable metric fixtures as long as the run reaches successor selection and seals a decision. The evidence is persisted in the sealed selection payload, not merely printed as logging output. Later Graph/UI work should be able to read these runs through `Graph.metrics.sets`, `Graph.metrics.candidates`, and the `SelectionNode.metric_set_id` binding without recomputing `imp@k`.

Default profile behavior is persist-only: `[selection.metrics] persist = true`, `[selection.metrics.imp_at_k] enabled = true`, and `score_points_per_imp_point = 0`. That means the next validation run should preserve the previous selection behavior while producing metric evidence. A scoring experiment can set `score_points_per_imp_point` nonzero later; `require_for_score = true` should be reserved for a separate run because incomplete `imp@k` then excludes candidates from decision-grade scoring.

The remaining gaps are narrower and more concrete:

- Split-aware evaluation roles are absent. `target.instance` / `target.instances` collapse into one effective cohort through `eval_instances()` at [profile.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/profile.rs:104).
- Benchmark generalization is genuinely missing. `BenchmarkFamily` and `RunSource` are Multi-SWE-only at [target_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/target_registry.rs:17) and [spec.rs](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs:81).
- `Evaluator`, `EvalSet`, and `EvalPolicy` hooks exist, but they are not split-role semantics yet; they are identity/provenance carriers over the current evidence shape at [evaluation.rs](/home/brasides/code/ploke/crates/ploke-records/src/evaluation.rs:47).
- `imp@k` is now first-class selection evidence, but descendant-growth is still missing. The current score module remains a separate read-only projection over typed child evidence at [score/mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:1).
- Broad-harness prompt posture is mostly implemented now, not just planned: prompt rendering is minimal/evidence-root based at [harness_request.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs:1160), automatic context is disabled at [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:235), and terminal evidence waits for `ChatTurnFinished` at [tui_adapter.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:600).

**Solutions**
The review’s best recommendations are: split roles, benchmark adapters, improvement-memory/evidence digest, archive projections for `imp@k` and descendant growth, and protocol-upgrade-only privileged mutation. I would keep that direction, but stage it more sharply.

1. Define split roles before benchmark generalization.
   Add `TaskSet<Role>` / `EvalSplitRole` first, because this directly prevents held-out contamination and clarifies selection authority. Old `target.instances` can remain the default evolution/selection cohort.

2. Add an explicit child-visible evidence digest.
   The missing object is not another prompt rewrite. It is a typed `LatestEvidenceDigest` or `ImprovementMemory` with visibility rules: train/probe evidence may be exposed; validation maybe summarized depending on policy; held-out final must stay sealed until final reporting.

3. Use the landed `imp@k` evidence path before adding descendant growth.
   `imp@k` now has a selection-time persisted boundary with evaluator, eval-set, budget, archive scope, candidate-set root, and considered-order binding. The next step is to validate that boundary with a live loop run, then reuse the same metric-set boundary for descendant-growth rather than inventing a second report path.

4. Treat benchmark family support as an adapter survey before code.
   HyperAgents’ four domains force different contracts: patch tasks, paper classification, reward-function generation, and grading. The Rust shape should come after that survey, not from adding enum variants ad hoc.

5. Keep privileged mutation behind protocol upgrade.
   The current History docs are right: ordinary descendants must not mutate policy-bearing `ploke-eval`; deeper mutation needs an unlock/fork transition, not `allow_edit_ploke_eval = true`. See [history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/history.rs:171).

**Metrics To Add**
- `imp@k(generator, start, evaluator, eval_set, budget)`: implemented as sealed selection-time evidence; validate with the next loop run and then expose through UI/Graph drilldown.
- Descendant-growth `G_gamma`, with explicit archive scope and minimum descendant count.
- Valid/admitted child rate, separated from attempted, applied, rejected, compiled, and selected.
- Parent sampling entropy and over-expanded-parent pressure.
- Train/validation/final deltas, with final-only evidence never entering parent selection.
- Transfer score: selected archive node’s ability to produce gains on a new domain under fixed budget.
- Context quality: explicit tool-requested context vs harness-injected context, useful evidence roots inspected, rejected proposal reasons.
- Operational cost: wall time, provider failure rate, token/tool cost per admitted child, and runtime failure vs candidate-quality failure.

**Next Steps**
1. Update the review with a status label per gap: implemented, implemented-but-narrow, designed-but-unwired, absent, protocol-upgrade-only.
2. Add a split-role design sketch mapping paper roles to `ploke` carriers: profile config, `EvalSet`, selection input, History payload, final report.
3. Define `LatestEvidenceDigest` as a typed projection with visibility policy.
4. Run a small persist-only loop using the new selection metrics path and keep the resulting sealed selection entries as fixtures for Graph/UI work.
5. Run a small code survey for benchmark-family adapters before touching enums.

The biggest headache to avoid now is letting split roles become string conventions on `EvalSet.kind`. Make the role a typed carrier early, or held-out evaluation will become impossible to reason about once broad descendants can inspect richer evidence.
