---
title: Ploke applicability map for selection-score mechanisms
created: 2026-06-05
updated: 2026-06-05
type: agent-report
tags: [ploke, selection-score, scoring, prototype1, applicability]
sources:
  - crates/ploke-selection-score/docs/text/formal-scoring.md
  - crates/ploke-selection-score/src/papers/
  - crates/ploke-selection-score/src/protocols/
  - crates/ploke-selection-score/src/ploke/
  - docs/active/agents/2026-06-04_scoring-mechanism-comparison-notes.md
confidence: medium
contested: true
---

# Ploke applicability map for selection-score mechanisms

## Scope

This note maps the implemented `ploke-selection-score` mechanisms to possible Ploke uses. It is not a proposal to collapse Ploke selection into one scalar score. The safer integration shape is a typed score profile: hard gates, operational metrics, protocol evidence, oracle evidence, route cost, and history/frontier signals stay separate until an explicit selector chooses how to combine them.

Current implementation surface:

- Source mechanism registry: `crates/ploke-selection-score/src/catalog/registry.rs`.
- Paper formula modules: `crates/ploke-selection-score/src/papers/`.
- Protocol and benchmark predicates: `crates/ploke-selection-score/src/protocols/`.
- Ploke adapters and current selector fragments: `crates/ploke-selection-score/src/ploke/`.
- Existing Prototype 1 comparison notes: `docs/active/agents/2026-06-04_scoring-mechanism-comparison-notes.md`.

## Direct conceptual correlates

| Mechanism | Direct Ploke correlate | How it could apply | Main caveat |
| --- | --- | --- | --- |
| Harness-1 state predicates and reward components | Prototype 1 externalized run state, curated evidence, protocol artifacts, and branch records | Use as the conceptual model for keeping selector inputs authority-bounded: keep only relevant evidence linked to a run/candidate, stop when answer found or budget exhausted, and report reward components separately. | The paper does not provide exact scalar weights; `crates/ploke-selection-score/src/protocols/harness1.rs` should remain a component/predicate layer. |
| AXIOM trust-first verification | Oracle eligibility, verified-vs-abstain branch outcomes, MBE/oracle checks | Treat unverifiable outcomes as abstentions rather than partial successes; track trust over attempted non-abstain answers. | Verification must be tied to concrete Ploke evidence, not model confidence. |
| RASER cost-aware routing | Model/provider/tool route selection, broad TUI attempt budgeting, protocol-vs-parent-patcher routing | Use `predicted_benefit - lambda * cost` for route choice when Ploke can estimate token, latency, retry, or tool cost before escalation. | Requires first-class cost telemetry and calibrated route-specific benefit predictors. |
| Interactive executable-game status logic | Prototype 1 child run lifecycle: format/tool errors, success, failure, continue, timeout | Report status and efficiency as separate metrics: success rate, average successful turns/steps, and efficiency. | Do not let efficiency reward short invalid runs; status must distinguish format/tool failure from timeout and success. |
| AGENTCL gains | History graph comparisons across generations and reruns | Decompose progress into plasticity, stability, and generalization: first-pass gain, second-pass retention, held-out task gain. | Requires baseline/first/second/held-out task identities that are stable across runs. |
| Deliberative Curation reputation and EigenTrust | Evaluator/reviewer/model trust, protocol reviewer reliability, source authority weighting | Use Beta reliability, propagated trust, tier gates, and fixed newcomer weights for evaluator or reviewer authority, not for direct patch success. | The paper's whole governance protocol is not a single objective; tiering and role gates must stay explicit. |
| VESTA metric-directed selection | Choosing among protocol adjudicators, statistical reports, or selector variants | Select candidates by metric direction: minimize AIC/JSD-like badness or maximize ELPD/utility-like goodness. | Only applies when metric semantics are already validated and comparable. |
| Expected Value Alignment | Mapping ordinal evaluator logits or rubric anchors to scalar expected scores | Convert five anchor-token probabilities to an expected score and train/replay mean-squared alignment against ground truth. | Needs a real five-anchor rubric and calibrated probabilities; arbitrary scores should not be forced into this shape. |
| Behavioral trait-vector diff | Before/after behavior or code-artifact embedding deltas | Add a side-channel signal for directional change and rank agreement after edits. | Analytic only until validated against oracle/protocol outcomes; embedding direction can overfit style. |

## Indirect or analogy-only uses

| Mechanism | Analogy for Ploke | Why it is not direct |
| --- | --- | --- |
| CSA labels and GRPO objective | Teach or evaluate when a model should self-solve, delegate to a stronger model, or ask for oracle/protocol review. | The crate implements scalar label/reward/objective helpers, not policy training. Ploke would need rollout labels and reward groups before this becomes operational. |
| Weak Critics / O-PCD | Filter weak reviewer critiques before distilling or trusting them; use token-level KL as an analysis/training loss. | Ploke currently has protocol review artifacts, not token-distribution teacher/student traces. |
| TRACE trajectory risk compression | Compress long run trajectories into risk evidence for reporting or triage. | Requires learned compressor/reader representations that Ploke does not currently emit. |
| Entropy dynamics orchestration | Track scheduler uncertainty across agents/tools/routes as a diagnostic. | The entropy model is interpretive and theory-shaped, not a validated universal controller. |
| Planning cost partitioning | Treat budget allocations across heuristics/tools as admissibility-style accounting. | Classical planning SCP relies on transition-system heuristics; Ploke can borrow the budget discipline, not the theorem directly. |
| HLL, MCP-Persona, Iteris predicates | Benchmark wrappers for trace validity, simulation fidelity, persona consistency, and human verification. | Useful for designing evaluation gates, but these benchmark domains are not Ploke's runtime domain. |

## Recommended integration order

1. Evidence gates first: keep Harness-1 and AXIOM-style predicates as hard gates around replayable evidence, oracle eligibility, and protocol artifacts.
2. Observability before selection pressure: add AGENTCL gains, route cost, efficiency, trust, entropy, and trait-diff signals as report-only metrics first.
3. Cost-aware routing next: once token/latency/retry/model costs are recorded, use RASER-style route choice for candidate generation or adjudicator routing.
4. Trust/reputation only for evaluators: apply Deliberative Curation trust to reviewers, models, or evidence providers before letting it influence branch selection.
5. Learned objectives last: CSA, O-PCD, TRACE, and EVA should remain offline/replay/training analyses until Ploke has stable labels, distributions, and held-out validation.

## Explicit non-goals

- Do not promote unresolved or candidate-only mechanisms from `formal-scoring.md` into selectors without a deep-read repair.
- Do not treat paper benchmark scores as Ploke success metrics without Ploke evidence carriers.
- Do not replace History/Crown authority with a scalar score from `ploke-selection-score`.
- Do not make IMP@K or embedding-diff metrics selector-driving before replay evidence proves they improve downstream outcomes.
