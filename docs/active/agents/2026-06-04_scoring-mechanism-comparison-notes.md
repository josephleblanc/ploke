# 2026-06-04 Scoring Mechanism Comparison Notes

Short description: live working notes for comparing Ploke Prototype 1 scoring/selection with formal scoring mechanisms from the Obsidian LLM wiki campaign. Update incrementally as evidence is found; do not hold the synthesis only in chat context.

Related planning/source files:
- Wiki campaign: `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/`
- Core synthesis: `/home/brasides/wiki/queries/research/ploke-arxiv-cs-ai-2026-06-02/synthesis/formal-scoring-mechanisms.md`
- Related synthesis docs: `scoring-mechanisms.md`, `symbolic-theses.md`, `formal-scoring-equation-audit.md`, `category-map.md`
- Ploke code: `/home/brasides/code/ploke/crates/ploke-eval/src/`
- Prototype 1 wiki docs: `/home/brasides/wiki/concepts/prototype1-loop.md`, `/home/brasides/wiki/entities/prototype1-state-command.md`, `/home/brasides/wiki/concepts/prototype1-state-config-planes.md`

Current repo checkpoint:
- Date checked: 2026-06-04
- Branch: `feature/ploke-loop`
- HEAD: `fba84475`
- Worktree is dirty: `AGENTS.md`, `CLAUDE.md`, `crates/ploke-eval/docs/prototype1-run-profile.md`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, `profile.rs`, `run/core.rs`, `tests/cli_tests.rs`, and `crates/ploke-records/src/run_profile.rs` are modified.

## Current Ploke scoring model, evidence first

1. Selection defaults
- `Selection::default()` sets `strategy = HistoryScoreChildProp` and `evidence = Operational` in `crates/ploke-eval/src/cli/prototype1_state/profile.rs:467-472`.
- Available strategies are `GenerationLocal`, `HistoryFrontierMax`, and `HistoryScoreChildProp` in `profile.rs:482-484`.
- Evidence modes are `Operational` and `OperationalAndProtocol` in `profile.rs:489-492` and map to traversal metric inputs in `profile.rs:408-413`.
- Oracle defaults to `RecordOnly` with `require_evidence` configured through the profile in `profile.rs:628-637`; relative-score evidence requires MBE plus target instances in `profile.rs:451-460`.

2. Operational score
- Operational metric fields include failed tool calls, patch attempted, patch state, submission artifact state, projection check state, partial patch failures, aborts, convergence, and oracle eligibility in `operational_metrics.rs:39-71`.
- `oracle_eligible` currently requires convergence, nonempty submission artifact, and passed patch-projection check in `operational_metrics.rs:116-120`.
- `selection_quality_points()` is a tiered point score in `operational_metrics.rs:172-188`:
  - `oracle_eligible`: +900
  - else `convergence`: +500
  - else `nonempty_valid_patch`: +200
  - `patch_attempted`: +50
  - each failed tool call: -25
  - each partial patch failure: -10
  - aborted: -500
  - repair-loop abort: -250

3. Protocol score
- Protocol metric input is opt-in: `metric::Inputs` defaults to `Operational`, with `OperationalAndProtocol` as the other mode in `metric.rs:10-25`.
- Protocol selection points reward reviewed calls/segments, crosswalks, and focused progress, and penalize missing reviews, skipped segment review, anchor mismatches, failed/uncovered/ambiguous source calls, and candidate concerns in `metric.rs:109-127`.
- `selection_delta_points()` adds positive deltas for review coverage/crosswalk/focused progress and reductions in missing/skipped/mismatch counts in `metric.rs:129-159`.

4. History traversal score
- `TraversalScore` contains `oracle`, `performance`, `frontier_delta`, `exploration_pressure`, and `generation` in `successor_selection/traversal.rs:1542-1549`.
- Child counts are derived from prior successful traversal decisions in `traversal.rs:1527-1540`; more successful children lower exploration pressure through `usize::MAX - child_count` in `traversal.rs:1572-1586`.
- `HistoryFrontierMax` compares scored payloads using traversal score plus deterministic tie-break rationale in `traversal.rs:1602-1668`.
- `HistoryScoreChildProp` has defaults `top_m = 3`, `lambda_millis = 10_000`, `metrics = Operational`, and `oracle = RecordOnly` in `traversal.rs:185-199`.
- Score-child-prop replay records seed, top_m, lambda, metric inputs, oracle mode, total weight, sample, selected index/candidate, alpha_mid, rows, alpha/exploitation/exploration/weight fields in `traversal.rs:388-457`.
- Selection weights are computed from items, child counts, metrics, oracle mode, and require-evidence; sampling uses a deterministic seed-derived sample and `sample_weighted_index` in `traversal.rs:539-556`.

## Formal mechanisms from wiki campaign to compare against

High-signal mechanisms already read:
- AGENTCL: plasticity/stability/generalization gains `PG_i = F_i - B_i`, `SG_i = S_i - F_i`, `GG_j = H_j - B_j`; see `formal-scoring-mechanisms.md:532-575`.
- RASER: cost-aware route argmax `argmax_r [ f_hat_r(s) - lambda c_r ]`; see `formal-scoring-mechanisms.md:577-613`.
- Behavioral trait-vector diff scoring: normalized before/after embedding diff, linear trait score, sign accuracy, Spearman validation; see `formal-scoring-mechanisms.md:615-663`.
- Interactive executable-game benchmark: status logic separates format error, success, failure, continue, timeout; efficiency combines success rate and turns; see `formal-scoring-mechanisms.md:669-719`.
- VESTA: metric-directed selection among candidate models by min AIC/JSD or max ELPD-LOO; see `formal-scoring-mechanisms.md:721-758`.
- Harness-1: reward-component set plus authority-bounded state predicates `keep(d)` and `stop(tau)`; see `formal-scoring-mechanisms.md:760-803`.
- MCP-Persona: success predicate combines execution success, persona consistency, and simulation fidelity; see `formal-scoring-mechanisms.md:847-887`.

Do not treat campaign `interest_score` as a paper-internal score. It is a campaign triage score, not a mechanism from the papers.

## Early comparison map

1. Ploke already has Harness-like structure.
- Ploke separates policy-facing candidate selection from recorded evidence, run records, protocol review, oracle evidence, candidate-set membership, replay seed, and deterministic sampling evidence.
- Nearest wiki analog: Harness-1 authority-bounded state evaluation, not a single scalar RL objective.

2. Ploke already has RASER-like routing potential, but cost is not first-class in the default score.
- Current score rewards operational success and penalizes failures/patch churn.
- It does not yet look like `benefit - lambda * cost` over routes/operators. Token/latency/model cost could become a separate route penalty rather than being inferred from failures.

3. Ploke has AGENTCL-style ingredients, but not the same temporal decomposition.
- The history graph can support baseline, first-pass, second-pass, and held-out comparisons.
- Current `selection_quality_points()` is candidate-local/run-local; it does not explicitly split plasticity, stability, and generalization gains.

4. Ploke’s protocol score resembles executable-game trace validation.
- It distinguishes process evidence quality, missing/invalid review evidence, crosswalk coverage, and candidate concerns.
- This is closer to trace-conditioned validation than final-answer-only scoring.

5. Trait-vector diff is a possible add-on, not a replacement.
- Ploke could score before/after code or behavior embeddings, but this should be gated by recorded artifacts and oracle/protocol evidence.
- Risk: embedding diff can overfit style/shape unless validated against actual patch success and replayed outcomes.

6. IMP@K is present but currently non-driving by default.
- Profile defaults keep `score_points_per_imp_point = 0` and `require_for_score = false` for IMP@K (`profile.rs:547-565`).
- This makes IMP@K useful as observability/archive metric before it becomes selection pressure.

## Gaps to inspect next

- Separate update-generation quality from beneficiary success: current operational score can reward a good run artifact, but longitudinal benefit to descendants needs explicit AGENTCL/IMP-style deltas.
- Add first-class cost accounting: token/model/latency/retry budget could support RASER-like `benefit - cost` selection.
- Tighten archive admission/novelty gates: Ploke has evidence/replay machinery, but novelty/freshness gates for memory/archive writes still need explicit predicates.
- Verifier robustness: current oracle/protocol paths rely on recorded evidence, but reward hacking/fuzzing-style verifier failure metrics from the campaign remain candidate-only and need deep reads before adoption.
- Authority handoff: config-plane docs say campaign config is runtime source of truth while profile is operator policy; scoring docs should keep these authority layers separate.

## Note-taking rule for this thread

After each source-read or code-inspection cluster, append a short evidence note here with:
1. source path + line range,
2. exact mechanism/finding,
3. comparison implication,
4. unresolved caveat.
