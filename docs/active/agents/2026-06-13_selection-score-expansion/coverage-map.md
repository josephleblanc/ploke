# 2026-06-13 selection-score expansion coverage map

Task: map the wiki equation bank into `crates/ploke-selection-score` without editing Rust.

Inputs inspected:
- `/home/brasides/wiki/queries/ploke/selection-scoring/index.md` lines 16-27, 43-58, 68-85
- `/home/brasides/wiki/queries/ploke/selection-scoring/ledgers/mechanism-ledger.md` lines 21-82
- `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md` lines 11-73, 73-112, 113-192
- `/home/brasides/wiki/queries/ploke/selection-scoring/current-mechanism.md` lines 19-40, reached through the index/ledger source anchors
- `/home/brasides/wiki/queries/ploke/selection-scoring/archive-parent-selection-mechanisms.md` lines 20-162, reached through the index/ledger source anchors
- `/home/brasides/code/ploke/crates/ploke-selection-score/src`

Repository/code anchors checked:
- Crate purpose: `crates/ploke-selection-score/src/lib.rs:1-19`
- Catalog/source metadata: `src/catalog/registry.rs:1-237`, `src/catalog/spec.rs:5-35`, `src/catalog/source.rs:3-12`
- Ploke modules: `src/ploke/mod.rs:1-12`, `src/ploke/current.rs:1-4`, `src/ploke/operational.rs:1-4`, `src/ploke/protocol.rs:1-4`, `src/ploke/evidence.rs:5-76`, `src/ploke/frontier.rs:5-38`, `src/ploke/oracle.rs:3-10`, `src/ploke/imp.rs:3-12`
- Paper modules: `src/papers/*.rs`, including `src/papers/hyperagents.rs:1-315`, plus `src/protocols/*.rs` and `src/common/*.rs`

Legend:
- Status `implemented`: executable Rust helper or catalog entry exists for the source mechanism.
- Status `partial`: a narrow helper or metadata anchor exists, but the full source mechanism is not implemented.
- Status `missing`: source mechanism has no Rust helper beyond generic utilities or placeholders.
- Status `unresolved`: source explicitly says candidate/prose-only/not-safely-formalizable, or the crate tracks it only as unresolved metadata.
- Exactness labels are source labels when quoted as `faithful-formalization`, `interpretive-compression`, or `not-safely-formalizable`; crate labels are `Exactness::{Faithful, Interpretive, ScopeLimited, NotFormalizable}`.

## 1. High-level crate boundary

`ploke-selection-score` is an experimental pure-helper crate, not the live Prototype 1 selector. Its crate docs say it keeps mechanisms small and pure so they can be replayed against candidate-history snapshots before promotion into Ploke proper (`src/lib.rs:1-4`). The public exports currently expose the catalog, common normalization/ranking helpers, RASER route helpers, evidence/evaluator gates, and frontier weights (`src/lib.rs:6-19`).

This matters for coverage: many rows are intentionally helper-level rather than live authority. A helper can be implemented while archive admission, child selection, evaluator authority, routing, or observability remain outside this crate.

## 2. Authority-separated coverage summary

| Authority class | Source anchor | Crate anchor | Status | Exactness label | Notes |
| --- | --- | --- | --- | --- | --- |
| Catalog/source grounding | index lines 16-27; mechanism-ledger lines 35-76; formal note lines 36-73, 73-112; archive page lines 28-39 | `src/catalog/registry.rs:44-227`; `src/catalog/spec.rs:5-35`; `src/catalog/source.rs:3-12` | implemented | crate `Exactness` enum | Registry stores stable ids, paper ids, kind, exactness, source path, and section keys. Paper-backed entries point at the formal note; SADN points at the SADN equation note; DGM-H/HyperAgents entries point at the archive parent-selection note. |
| Current selector identity | current-mechanism lines 21-22; mechanism-ledger line 25 | `src/ploke/current.rs:1-4` | partial | scope-limited metadata | Only a `SPEC_ID` anchor exists for `ploke-current-selection-score`; there is no full `Exec(e_sel, x_sel, s_history)` implementation in this crate. |
| Selectable candidate evidence filter | current-mechanism lines 23-24 | `src/ploke/evidence.rs:5-60` | partial | scope-limited | `Lane`, `Evidence::ready`, `is_ready`, and `evidence_gate` implement a generic lane readiness gate. They do not encode the full `SelectionInput`, `TraversalDecision`, `PerformanceRecorded`, and `OracleAdmissible` predicates. |
| Evaluator reliability/authority gate | formal mechanisms lines 18-21, 33-35; design-delta theme in index lines 50-54 | `src/ploke/evidence.rs:44-76` | implemented helper | scope-limited | `EvalEvidence`, `reliability`, and `eval_gate` separate evaluator calibration/versioning/replayability from selector score. This is evaluator authority, not archive admission. |
| Operational run score `B_op`, `Q_op` | current-mechanism lines 25-26; mechanism-ledger line 25 | `src/ploke/operational.rs:1-4` | missing/placeholder | source `faithful-formalization`, crate metadata-only | The source formula is not implemented. The file only has `SPEC_ID = "ploke-current-operational-score"`. |
| Protocol evidence score and deltas `Q_pr`, `Q_pr^Delta` | current-mechanism lines 27-28 | `src/ploke/protocol.rs:1-4` | missing/placeholder | source `faithful-formalization`, crate metadata-only | No reviewed-call/reviewed-segment/mismatch/delta arithmetic exists here yet. |
| Candidate performance score `P(c)` / outcome `O(c)` | current-mechanism lines 29-30 | none beyond `imp_delta`, evidence, and placeholders | missing | source `faithful-formalization` | No composition of outcome points, child run scores, protocol deltas, and IMP@K exists. |
| IMP@K score contribution | current-mechanism lines 29-30; index lines 79-85; mechanism-ledger line 67 | `src/ploke/imp.rs:3-12` | partial | source report-only/default-nondriving; crate scope-limited | `imp_delta` applies score*points when enabled and returns zero when disabled. It does not compute IMP@K or persist metric rows. |
| Relative oracle score `Omega(c)` | current-mechanism lines 31-32 | `src/ploke/oracle.rs:3-10` | implemented helper | source `faithful-formalization`, crate scope-limited | `oracle_rate(resolved, configured)` maps `configured == 0` to absent `None`. It does not implement target matching, duplicate/mismatch rejection, or ordering tie-breaks. |
| `HistoryFrontierMax` lexicographic selector | current-mechanism lines 33-34 | no direct module; generic `argmax_finite` in `src/common/selection.rs:3-19` is scalar-only | missing | source `faithful-formalization` | No tuple `(Omega, P, F, H, G)` ordering, frontier delta, child-count pressure tuple, or SHA replay tie key exists in this crate. |
| `HistoryScoreChildProp` parent/child weighting | current-mechanism lines 35-36; archive page lines 40-96 | `src/ploke/frontier.rs:5-38`; `src/common/probability.rs:3-4` | partial/implemented helper | source `faithful-formalization`; crate scope-limited | `frontier_weights` implements the top-m midpoint sigmoid and inverse child-count factor once the caller supplies the active quality coordinate. Missing pieces: `alpha_P`/`alpha_Omega` normalization, quality-coordinate selection, deterministic replayable weighted sampling, and nonpositive/nonfinite replay fallback. |
| Archive parent selection (DGM-H style) | archive page lines 40-96; mechanism-ledger line 62 | `src/papers/hyperagents.rs:20-131`; `src/ploke/frontier.rs:12-38`; registry lines 46-56 | implemented helper | source/crate `Faithful` for DGM-H; `frontier` remains scope-limited Ploke helper | `hyperagents::parent_weights` and `parent_probabilities` implement DGM-H midpoint/sigmoid/child-count weights and probability normalization. They do not own archive state or perform random sampling. |
| Archive admission | archive page lines 98-120; mechanism-ledger lines 63, 68-70, 75 | `src/papers/hyperagents.rs:133-139`; registry lines 57-67 | implemented DGM-H predicate; missing newer archive families | source/crate `Faithful` for DGM-H; medium draft for newer rows | DGM-H valid-child admission is implemented as a hard predicate. SkillDAG/EvoTrainer/DeltaMem/reasoning-primitive admission rows remain missing. Keep admission separate from parent selection. |
| Staged evaluation / multi-domain parent score | archive page lines 121-147; mechanism-ledger lines 64-65 | `src/papers/hyperagents.rs:141-202`; registry lines 68-78 | implemented helper | source/crate `Faithful` | Implements failed-gate zeroing, cross-domain average, and anchored domain gates for coding, at-least-one-success domains, robotics nonzero reward, and preliminary success rate. |
| Routing authority | formal note lines 39-40, 45-47, 55-56, 61-72; mechanism-ledger lines 44, 54, 60 | `src/papers/csa.rs:3-123`; `src/papers/axiom.rs:3-20`; `src/papers/raser.rs:3-173`; protocols modules | implemented helpers, no live authority | mixed `Faithful`/`Interpretive` | CSA labels/rewards, AXIOM abstention routing, and RASER cost-aware route selection are helper-level. They do not route live Ploke work unless an outer caller makes them authoritative. |
| Observability/reporting | index lines 79-85; formal note lines 18-35, 59-72 | catalog + `src/common/rates.rs:3-18`; protocol pass-rate helpers | partial | mixed | The crate has catalog metadata and metric helpers, but does not persist replay material, candidate-set evidence, benchmark tables, or Ploke run records. |

## 3. Implemented coverage rows

### 3.1 Current Ploke selector helpers

| Source mechanism | Source anchor | Crate module | Status | Exactness | Coverage note |
| --- | --- | --- | --- | --- | --- |
| Evidence lanes / hard gates | current-mechanism lines 23-24 | `src/ploke/evidence.rs:5-60` | partial | scope-limited | Generic lane readiness: Operational, Protocol, Oracle, Imp, Cost, Handoff, Trace, History. Does not encode full candidate-membership semantics. |
| Evaluator calibration / reliability | formal note lines 18-21, 33-35 | `src/ploke/evidence.rs:44-76` | implemented helper | scope-limited | Beta-style reliability plus versioned/replayable/calibrated gate. Keep under evaluator authority, not child-selection authority. |
| Relative oracle score | current-mechanism lines 31-32 | `src/ploke/oracle.rs:3-10` | implemented helper | scope-limited faithful sub-mechanism | Implements `resolved/configured` with absent zero denominator. |
| IMP@K score contribution toggle | current-mechanism lines 29-30; index lines 79-85 | `src/ploke/imp.rs:3-12` | partial | scope-limited | Applies contribution only when score-bearing; preserves default zero contribution when disabled. No IMP@K computation. |
| Frontier/child-count sampling weights | current-mechanism lines 35-36; archive page lines 40-96 | `src/ploke/frontier.rs:5-38` | partial/implemented helper | scope-limited faithful sub-mechanism | Top-m midpoint, sigmoid, and inverse child-count factor are implemented. Active-quality normalization and replayable sampling are not. |

### 3.2 Formal scoring mechanism bank mapped to paper/protocol modules

| Wiki row(s) | Source anchor | Crate module | Status | Exactness label | Notes |
| --- | --- | --- | --- | --- | --- |
| dgm-h-parent-selection, dgm-h-archive-admission, dgm-h-staged-evaluation, dgm-h-cross-domain-average, dgm-h-modifiable-parent-selection | mechanism-ledger lines 62-66; archive page lines 40-162 | `src/papers/hyperagents.rs:20-307`; registry lines 46-89 | implemented helpers; modifiable selector is interpretive only | DGM-H faithful for Appendix A.2/admission/staged gates; interpretive for Appendix E.5 sketches | Parent weights/probabilities, hard valid-child admission, staged gates, cross-domain average, softmax/UCB/adaptive/component-score sketches. No archive ownership or random draw. |
| M2-0001..M2-0007 Deliberative Curation reputation, trust propagation, voting weight, fast-track admission, simulation metrics, sanction FPR | mechanism-ledger lines 37-43; formal note lines 37-38 | `src/papers/deliberative.rs:3-353`; registry lines 90-96 | implemented | source mostly `faithful-formalization`; crate `Exactness::ScopeLimited` | Helpers cover Beta reputation, decay, voting weight/caps/tier predicates, EigenTrust row normalization/iteration/convergence, active precision/recall, sanction FPR, and fast-track predicate. Registry marks scope-limited because the whole protocol is not one objective. |
| M2-0008..M2-0012 Capability Self-Assessment labels, SFT, reward, diversity filter, GRPO | mechanism-ledger lines 44-48; formal note lines 39-40 | `src/papers/csa.rs:3-123`; registry lines 97-103 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Implements label construction, binary reward, diversity, SFT loss, policy ratio, standardized advantages, clipped surrogate, and group GRPO loss. |
| M2-0013..M2-0014 Weak Critics / O-PCD filters and KL loss | mechanism-ledger lines 49-50; formal note lines 41-42 | `src/papers/weak_critics.rs:3-41`; registry lines 104-110 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Product filter and retained-set KL loss. |
| M2-0015..M2-0016 TRACE latent evidence / unsafe classifier | mechanism-ledger lines 51-52; formal note lines 43-44 | `src/papers/trace.rs:5-15`; registry lines 111-117 | partial/implemented scalar helpers | `faithful-formalization`; crate `Exactness::Faithful` | Implements unsafe probability and BCE loss, but not compressor/readout tensor plumbing. |
| M2-0017..M2-0018 AXIOM trust metric / abstention gate | mechanism-ledger lines 53-54; formal note lines 45-46 | `src/papers/axiom.rs:3-20`; registry lines 118-124 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Trust score and abstain-first routing predicate. |
| M2-0019..M2-0020 EVA anchor-token score / EVA loss | mechanism-ledger lines 55-56; formal note lines 47-48 | `src/papers/eva.rs:7-85`; registry lines 125-131 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Anchor softmax, expected score, EVA MSE, and total loss. |
| M2-0021 Entropy dynamics orchestration diagnostics | mechanism-ledger line 57; formal note lines 49-50 | `src/papers/entropy.rs:3-43`; registry lines 132-138 | implemented | source `interpretive-compression`; crate `Exactness::Interpretive` | Entropy, entropy rate, drift, and closed-form path helper. Observability/diagnostic, not selector authority. |
| M2-0022 Planning cost / saturated cost partitioning | mechanism-ledger line 58; formal note lines 51-52 | `src/papers/planning.rs:3-75`; registry lines 139-145 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Plan cost, partition validity, heuristic sum, maximum single-change, saturation updates. |
| M2-0023 AGENTCL plasticity/stability/generalization gains | mechanism-ledger line 59; formal note lines 53-54 | `src/papers/agentcl.rs:3-14`; registry lines 146-152 | implemented | source `interpretive-compression`; crate `Exactness::Interpretive` | Gain metrics only; no benchmark protocol runner. |
| M2-0024 RASER route selection | mechanism-ledger line 60; formal note lines 55-56 | `src/papers/raser.rs:3-173`; registry lines 153-159 | implemented | source `interpretive-compression`; crate `Exactness::Interpretive` | Cost-aware route score/argmax, bridgeability label, RASER-2 threshold, RASER-3 score/select. Routing helper only. |
| M2-0025 Behavioral trait-vector diff score | mechanism-ledger line 61; formal note lines 57-58 | `src/papers/traits.rs:5-60`; registry lines 160-166 | implemented | `faithful-formalization`; crate `Exactness::Faithful` | Normalized diff, linear score, sign accuracy, Spearman rho. |
| 2510.23535 SADN sequential advantage / greedy IGM | index lines 56-57; registry lines 167-177 | `src/papers/sadn.rs:8-116` | implemented | crate `Exactness::ScopeLimited` | Source authority is the SADN equation note, not the formal-scoring input file. Included here because the index names SADN as part of the selection-scoring packet and the crate registry includes it. |
| 2606.00103 executable-game benchmark | formal note lines 61-62; registry lines 178-184 | `src/protocols/game_bench.rs:3-67` | implemented | source `interpretive-compression`; crate `Exactness::Interpretive` | Episode status, success rate, average successful turns, efficiency. Benchmark protocol, not child selector. |
| 2606.00384 VESTA metric-directed model selection | formal note lines 63-64; registry lines 185-191 | `src/protocols/vesta.rs:5-14` | implemented | source/crate `Faithful` | Selects argmin for AIC/JSD or argmax for ELPD-LOO over supplied metric scores. |
| 2606.02373 Harness-1 reward components / state predicates | formal note lines 65-66; registry lines 192-198 | `src/protocols/harness1.rs:3-21` | implemented component helpers | source/crate `Interpretive` | Reward-component enum, keep-doc predicate, stop predicate. No scalar RL objective. |
| 2606.02449 HLL human-verification pass rate | formal note lines 67-68; registry lines 199-205 | `src/protocols/hll.rs:3-9` | implemented | source/crate `Interpretive` | Predicate + pass rate. Authority-boundary benchmark, not solver. |
| 2606.02470 MCP-Persona task success | formal note lines 69-70; registry lines 206-212 | `src/protocols/mcp_persona.rs:3-9` | implemented | source/crate `Interpretive` | Predicate + success rate over personalized tool tasks. |
| 2606.02484 Iteris accepted-output predicate | formal note lines 71-72; registry lines 213-219 | `src/protocols/iteris.rs:3-12` | implemented | source/crate `Interpretive` | Output-kind enum + human-verified acceptance predicate. |

## 4. Missing coverage rows

These have source-grounded mechanisms in the wiki packet but no faithful executable implementation in `crates/ploke-selection-score/src`.

| Source mechanism | Source anchor | Missing crate surface | Exactness/source status | Why missing matters |
| --- | --- | --- | --- | --- |
| Full current selector execution `Exec(e_sel, x_sel, s_history)=s_decision` | current-mechanism lines 21-22 | no orchestration module | descriptive current mechanism | The crate exposes helpers only; it cannot replay a full Prototype 1 selection decision end-to-end. |
| Selectable candidate predicate with input, decision, performance, and oracle admissibility | current-mechanism lines 23-24 | no full candidate model | source faithful | `evidence_gate` can require ready lanes, but lacks typed selection input/traversal/performance/oracle mismatch semantics. |
| Operational score `B_op` and `Q_op` | current-mechanism lines 25-26 | `src/ploke/operational.rs` only has `SPEC_ID` | source faithful | Core run-quality scoring is absent. This is the largest gap for current Ploke parity. |
| Protocol score `Q_pr`, gain/repair deltas | current-mechanism lines 27-28 | `src/ploke/protocol.rs` only has `SPEC_ID` | source faithful | Protocol evidence cannot be scored or compared here yet. |
| Candidate performance `O(c)+sum Q_op+sum Q_pr+delta_imp` | current-mechanism lines 29-30 | no candidate-performance composition | source faithful | No formula combines child evidence into `P(c)`. |
| Relative oracle ordering and admissibility errors | current-mechanism lines 31-32 | only `oracle_rate` | source faithful | Missing cross-multiplication ordering, resolved-count tie-break, fewer-configured tie-break, and hard evidence-error rules. |
| `HistoryFrontierMax` lexicographic tuple | current-mechanism lines 33-34 | no tuple selector | source faithful | No `T(c)=(Omega,P,F,H,G)` implementation or deterministic tie key. |
| `HistoryScoreChildProp` active-quality normalization and replay sampling | current-mechanism lines 35-36 | `frontier_weights` only | source faithful | Missing `alpha_P`, `alpha_Omega`, active-coordinate selection, deterministic weighted draw, and fallback replay material. |
| DGM-H archive ownership / random parent draw | archive page lines 68-79, 98-120; mechanism-ledger lines 62-63 | helpers exist, but no stateful archive or RNG/replay sampler | source faithful | `hyperagents` implements probabilities and admission predicates; it does not own `A_t`, mutate an archive, or sample parents. |
| DGM-H modifiable parent selection as production authority | archive page lines 148-162; mechanism-ledger line 66 | interpretive sketch helpers only | source `interpretive-compression`, medium | `softmax`, `ucb`, adaptive weight, and component-score helpers exist; they are replay sketches, not a promoted Ploke/default selector. |
| `imp-at-k` reporting metric | mechanism-ledger line 67; index lines 79-85 | only `imp_delta` toggle | source report-only | No computation of maximum improvement over k variants; do not treat `imp_delta` as full IMP@K. |
| SkillDAG typed skill graph admission | mechanism-ledger line 68 | no graph/admission helper | medium draft | Admission/rollback logic is not implemented. |
| EvoTrainer diagnostic branch admission | mechanism-ledger line 69 | no diagnostic/backtest admission helper | interpretive draft | Missing policy/harness variant keep/edit/merge/prune logic. |
| DeltaMem residual-tree write rule | mechanism-ledger line 70 | no residual-tree or memory-write helper | faithful draft | Missing retrieve-chain/fail-to-cover/write-delta/consolidation rule. |
| Overlaying governance delegation overlay | mechanism-ledger line 71 | no authorization overlay helper | interpretive draft | This belongs to prompt/tool policy authority, not scoring math yet. |
| StepFinder step attribution | mechanism-ledger line 72 | no step-level attribution helper | interpretive draft | Evaluation-only trace localization absent. |
| Handoff-debt rediscovery cost | mechanism-ledger line 73 | no handoff-cost metric helper | interpretive draft | Handoff observability lane exists (`Lane::Handoff`) but no metric. |
| SAGE socialized evolution | mechanism-ledger line 74 | no socialized-evolution metric/helper | interpretive draft | Evaluation-only; not child-selection authority. |
| Reasoning primitive induction | mechanism-ledger line 75 | no primitive clustering/admission helper | faithful draft | Prompt/tool policy or archive admission, not current score formula. |

## 5. Unresolved / candidate-only rows kept out of formulas

These rows should stay unresolved or catalog-only until repaired by deeper source reads. This section is intentionally separate to avoid promoting candidate-only mechanisms into formulas.

| Source row | Source anchor | Crate status | Exactness | Coverage note |
| --- | --- | --- | --- | --- |
| 2606.01066 verifier metrics lead | formal note lines 97-99; registry lines 220-226 | registry-only unresolved entry | source candidate-only; crate `Exactness::NotFormalizable` | The crate correctly tracks this as `MechanismKind::Unresolved`; no formulas are exposed. |
| Prose-only/not-safely-formalizable papers 2606.00005, 2606.00376, 2606.00476, 2606.00618, 2606.00642, 2606.00708, 2606.00756, 2606.00765, 2606.00914, 2606.01120, 2606.01139, 2606.01185, 2606.01199, 2606.01230, 2606.01279, 2606.01365 | formal note lines 75-93 | no registry entries except where separately implemented elsewhere | source prose-only / not-safely-formalizable | Correctly absent from executable formulas. |
| Candidate-only mechanisms 2606.00718, 2606.00726, 2606.01314, 2606.01912, 2606.01991, 2606.02054, 2606.02060, 2606.02109, 2606.02132, 2606.02282, 2606.02355, 2606.02359, 2606.02372 | formal note lines 94-110 | no formula helpers | candidate-only / first-pass pending | Correctly absent. Add only after source-visible equations/predicates are repaired. |
| Negative/excluded paper set | formal note lines 111-112 | no formula helpers | excluded | Correctly absent. |

## 6. Coverage conclusions

Implemented now:
- Source catalog and exactness metadata for 23 registry entries (`src/catalog/registry.rs:44-227`): 22 helper-backed/tracked mechanisms plus one unresolved verifier-metrics lead.
- Most formal note exact/scoped equation helpers from 2606.00007, 2606.00251, 2606.00424, 2606.00611, 2606.00671, 2606.01160, 2606.01351, 2606.02438, 2606.02461, 2606.02488, and 2606.02536.
- DGM-H/HyperAgents parent selection, valid-child admission predicate, staged/cross-domain scoring helpers, and Appendix E.5 interpretive selector sketches.
- Formal decision/protocol helper modules for game benchmark, VESTA, Harness-1, HLL, MCP-Persona, and Iteris.
- Ploke-specific narrow helpers for evidence gates, evaluator gates, oracle rate, IMP@K contribution toggle, and frontier/child-count weighting.

Highest-priority missing current-Ploke parity:
1. Operational run score `B_op`/`Q_op`.
2. Protocol evidence score and delta score.
3. Candidate performance composition `P(c)`.
4. Full selectable-candidate admissibility model.
5. `HistoryFrontierMax` lexicographic selector.
6. `HistoryScoreChildProp` active-quality normalization plus deterministic replayable sampling.

Authority boundaries to preserve:
- Archive admission has a DGM-H hard-predicate helper, but stateful archive ownership and newer admission families remain separate from parent/child selection.
- Parent selection / child selection currently has helper-level weights/probabilities, not live selector authority or random/replay sampling.
- Routing helpers (CSA, AXIOM, RASER) are not live Ploke routing authority.
- Observability helpers/catalog entries are not persisted run evidence.
- Evaluator gates are calibration/authority predicates, not scoring formulas by themselves.
- Candidate-only/unresolved mechanisms remain unresolved and should not be promoted into formulas without new source anchors.
