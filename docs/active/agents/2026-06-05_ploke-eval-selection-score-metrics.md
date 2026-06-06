---
title: Ploke-eval metrics needed for selection-score mechanisms
created: 2026-06-05
updated: 2026-06-05
type: agent-report
tags: [ploke, ploke-eval, selection-score, metrics, prototype1]
sources:
  - crates/ploke-selection-score/docs/text/formal-scoring.md
  - crates/ploke-selection-score/src/papers/
  - crates/ploke-selection-score/src/protocols/
  - crates/ploke-eval/src/operational_metrics.rs
  - crates/ploke-records/src/selection.rs
  - docs/active/agents/2026-06-04_scoring-mechanism-comparison-notes.md
  - docs/active/agents/2026-06-05_selection-score-ploke-applicability.md
confidence: medium
contested: true
---

# Ploke-eval metrics needed for selection-score mechanisms

## Scope

This note lists the extra telemetry `ploke-eval` would need before the mechanisms in `ploke-selection-score` can become useful for Ploke reporting or selection. The immediate recommendation is report-only instrumentation. Selector-driving use should wait until the new metrics are tied to replayable run evidence, oracle/protocol outcomes, and stable eval-set identities.

## Already present carriers

`ploke-eval` already records useful operational fields through `OperationalRunMetrics`:

- Tool call totals and failed calls.
- Patch attempted state, patch apply state, partial patch failures, and same-file retry pressure.
- Submission artifact state and patch projection check state.
- Abort state, repair-loop abort state, convergence, and oracle eligibility.
- A current `selection_quality_points()` profile that rewards oracle eligibility, convergence, usable patches, and patch attempts while penalizing failed calls, partial patch failures, aborts, and repair-loop aborts.

`ploke-records` already has a persisted selection metric envelope:

- `MetricPolicy` persists metrics by default and uses `ScoreProfile::OperationalQualityV1`.
- `ImpAtKPolicy` is enabled by default with `budget_k = 50`, `ArchiveScope::SelectionScope`, `score_points_per_imp_point = 0`, and `require_for_score = false`.
- `MetricCandidate` carries candidate occurrence and membership ids plus optional `ImpAtK`.
- `ImpAtK` can identify start node, branch, generation, parent node, runtime, evaluator, eval set, budget, baseline score, best descendant score, improvement, descendant counts, scored descendant counts, and incomplete reasons.

These carriers are a good base for operational quality and longitudinal improvement, but they do not yet expose cost-aware routing, evaluator trust propagation, route entropy, calibrated ordinal rubrics, or policy-training traces.

## Metric requirements by mechanism

| Mechanism family | Metrics to add in `ploke-eval` | Existing partial carrier | Main benefit | Caution |
| --- | --- | --- | --- | --- |
| RASER route utility | Route id, chosen model/provider/tool path, predicted benefit, cost components, lambda, token count, latency, retry count, model price, and route outcome. | Operational tool failures and retry pressure; run profile/model routing records may carry some provenance. | Enables explicit `benefit - lambda * cost` routing for generation and adjudication. | Cost must stay separate from correctness so cheap invalid runs are not rewarded. |
| Interactive benchmark status | Episode status, success/failure/timeout/format/tool-error class, turn or step count, max turns, success rate, average successful turns, and efficiency. | Abort state, tool failures, patch states, convergence, oracle eligibility. | Separates invalid, timed-out, failed, and successful runs before efficiency is computed. | Efficiency should only be computed over successful runs or reported alongside status. |
| AXIOM trust-first verification | Attempted non-abstain count, wrong attempted count, abstain count, abstain reason, verified route, translator/checker status, and oracle eligibility. | Oracle eligibility, submission artifact state, patch projection check state. | Gives a trust metric that punishes wrong verified attempts without treating abstention as correctness. | Verification must point at concrete Ploke evidence, not model confidence. |
| Harness-1 state predicates | Kept evidence ids, answer-found flag, budget-exhausted flag, relevant-doc recall, answer-doc recall, trajectory-relevant recall, trajectory-answer recall, tool diversity, and turn penalty. | Protocol artifacts and persisted run evidence, but not this reward component breakdown. | Makes evidence retention and stop conditions auditable as separate reward components. | The reward components should stay typed; do not hide them inside one scalar too early. |
| AGENTCL gains | Baseline, first-pass, second-pass, held-out scores, task id, eval-set id, generation id, and rerun lineage. | `ImpAtK` baseline, best descendant, improvement, generation, evaluator, and eval-set fields. | Measures plasticity, stability, and generalization instead of only candidate-local quality. | Requires stable task and eval-set identities across runs. |
| Deliberative trust | Evaluator success/failure counts, reviewer interaction matrix entries, pretrusted prior vector, epsilon, gamma, tier thresholds, interaction counts, sanction flags, active precision, and active recall. | Evaluator ids in `ImpAtK`; operational success/failure facts. | Weights evaluator/reviewer authority by reliability and propagated trust. | Use for evaluator authority, not direct patch selection, until identity and Sybil assumptions are explicit. |
| VESTA metric-directed selection | Candidate id, metric id, metric direction, metric value, selected id, and adjudicator/model provenance. | Metric candidate ids and score profile. | Keeps model or adjudicator selection tied to metric semantics. | Only compare values that share the same metric definition and direction. |
| Expected Value Alignment | Five anchor logits or probabilities, expected score, ground-truth rubric score, SFT loss, EVA loss, and alpha. | No direct carrier beyond score profile. | Supports calibrated ordinal rubric reporting for evaluator outputs. | Requires a real five-anchor rubric; arbitrary scalar scores should not be forced into anchor space. |
| CSA self/delegate routing | Repeated probe outcomes, correctness labels, self-solve or delegate decision, reward group, policy ratio, advantage, and optional KL. | Route/model provenance may exist elsewhere, but not policy-training traces. | Trains or evaluates when a model should solve, delegate, or ask for review. | Offline/training-only until rollout labels and reward groups are stable. |
| Weak Critics / O-PCD | Retained feedback triples, weak-critic outcome booleans, rubric booleans, student/teacher token distributions, and KL divergence. | Protocol review artifacts, but not token distributions. | Filters weak critiques before trusting or distilling them. | Token-distribution losses need provider/model access that may not be available in persisted records. |
| TRACE risk | Trajectory id, unsafe/risky label, unsafe probability, BCE, compressor evidence ids, and long-context token budget. | Run conversations and operational abort/tool-failure fields. | Turns long-run risk into reportable triage evidence. | Learned compression is not implemented; start with labels and evidence ids only. |
| Entropy dynamics | Scheduler probability distribution per step, route/agent/tool alternatives, entropy, entropy rate, task pressure, and context drift proxy. | No direct carrier. | Detects routing collapse, indecision, or over-concentrated orchestration. | Entropy is diagnostic unless validated against run outcomes. |
| Planning cost partitioning | Budget allocation per heuristic/tool/route, remaining budget after saturation, max single-change improvement, and valid allocation flag. | IMP@K budget and operational retry pressure. | Makes search and tool-budget accounting explicit. | Classical planning guarantees do not transfer directly to Ploke runtime behavior. |
| Trait-vector behavior | Before/after artifact embeddings, normalized diff vector, trait weights, trait score, sign accuracy, and Spearman agreement. | Candidate artifacts and archive records can identify before/after objects, but not vectors. | Adds a side-channel for behavior or style shifts across edits. | Treat as analysis until validated against oracle/protocol outcomes. |
| HLL / MCP-Persona / Iteris gates | Trace validity, barrier satisfied, persona consistency, simulation fidelity, human-verified flag, output-kind flag, and success predicate. | Protocol review and run artifacts may contain analogous facts. | Provides benchmark-style gates for special eval protocols. | Mostly domain-specific benchmark wrappers, not general selector inputs. |

## Near-term instrumentation slices

1. Route cost ledger
   - Record route id, model/provider, tokens, latency, retry count, edit/tool count, estimated price, and chosen-route outcome.
   - This directly enables RASER-style reporting without changing selection.

2. Episode status ledger
   - Normalize run outcomes into success, failure, timeout, format error, tool error, and incomplete.
   - This makes interactive-benchmark and AXIOM-style metrics less dependent on one operational point score.

3. Longitudinal eval-set snapshot
   - Persist baseline, first-pass, second-pass, held-out score, task id, eval-set id, evaluator id, and generation.
   - This extends current IMP@K observability toward AGENTCL-style gain decomposition.

4. Evaluator trust ledger
   - Persist evaluator/reviewer success and failure counts, interaction edges, active flags, and trust parameters.
   - This should remain evaluator-authority metadata until identity and role gates are designed.

5. Artifact embedding diff
   - Persist before/after artifact ids, embedding model id, vector hash, normalized diff metadata, and trait score outputs.
   - This is useful for report analysis before it has selector authority.

6. Scheduler distribution capture
   - Persist the probability distribution over route/tool/agent alternatives when the orchestrator chooses among them.
   - This unlocks entropy diagnostics and makes route-selection uncertainty auditable.

## Reporting posture

The safe migration path is:

1. Add typed metrics as report-only fields.
2. Keep current operational score and IMP@K defaults unchanged.
3. Compare new metrics against oracle/protocol outcomes in run reviews.
4. Promote only the metrics that repeatedly explain downstream improvement without weakening evidence gates.

The wrong migration path is to add another aggregate score that mixes cost, trust, correctness, embeddings, and protocol evidence before those signals have independent validation.
