# Prototype 1 Scoring Evidence Persistence and CLI Visibility Review

Reviewer: reviewer 2
Date: 2026-05-07
Scope: scoring/selection evidence persistence and operator-facing CLI visibility after Reviewer 1's loop rerun readiness review.

## 1. Short Answer

Enough data is persisted to explain a successful History-backed successor selection after it reaches the handoff sealing path, but the CLI does not yet expose the full explanation.

The sealed `SelectionDecisionEntry` persists the selected candidate, the complete ordered considered payload set, each candidate's `SelectionInput`, sealed candidate evidence, candidate Artifact payload, candidate-set commitment/proofs, projection failures, traversal seed/strategy, and the resulting `SuccessorDecision`. That is sufficient to audit or replay the selection from the History segment.

The current CLI can show that sealed selection rows exist, validate their hashes/bindings, list candidate subjects, and report decision-grade gaps. It cannot print the full sealed decision payload, full `SelectionInput`s, full decision rationale, traversal seed/strategy, per-candidate scoring components/weights, or the exact considered order as active traversal used it without reading the raw History segment.

If the run stops before a selected successor reaches handoff sealing, there may be no sealed selection entry. In that case, only projection surfaces over child evidence, score reports, and transition journal records remain.

## 2. Persisted Data Inventory

- Reviewer 1 context: active selection now defaults to History traversal with current-generation candidates appended, and Reviewer 1 identified handoff cleanliness/tracked-file risk before rerun: `docs/archive/agents/2026-05/2026-05-07-loop-rerun-readiness-review.md:1`.

- Filesystem child evidence remains in normal campaign records, and the read-only store loads it from the transition journal, evaluations, node invocations/results/ready/completion records, node records, runner requests, and runner results: `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:128`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:147`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:197`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:203`.

- The sealed History block store lands under the campaign manifest parent at `prototype1/history`, with sealed blocks in `blocks/segment-000000.jsonl`, indexes under `index`, and heads in `index/heads.json`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:641`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:650`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:654`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:658`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:670`.

- A candidate payload persists the exact selector-facing data: candidate subject, procedure, `selection_input`, `selection_input_hash`, projection failures, source refs/hashes, `sealed_evidence`, and `artifact`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:2707`. The `SelectionInput` contains candidate node/branch/generation, branch disposition, evaluation artifact path, and per-instance parent/child operational metrics: `crates/ploke-eval/src/successor_selection/evidence.rs:17`, `crates/ploke-eval/src/successor_selection/evidence.rs:42`.

- `SealedCandidateEvidence` persists coordinate, lifecycle, evaluation rows, runtime rows, branch rows, citations, and diagnostics: `crates/ploke-eval/src/cli/prototype1_state/history.rs:2649`. `CandidateArtifact` persists the node record and resolved treatment branch needed for handoff hydration: `crates/ploke-eval/src/cli/prototype1_state/history.rs:2674`.

- Candidate payload construction hashes `selection_input`, includes sealed evidence citations in source refs/hashes, and raises schema version when sealed evidence/artifact material is present: `crates/ploke-eval/src/cli/prototype1_state/history.rs:3251`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3276`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3281`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3286`.

- A sealed selection decision persists procedure/policy, scope, selected candidate, ordered `considered` payloads, `considered_order_hash`, candidate-set commitment, projection failures, traversal evidence, and the final successor decision: `crates/ploke-eval/src/cli/prototype1_state/history.rs:3374`. `TraversalEvidence` persists only seed and strategy: `crates/ploke-eval/src/cli/prototype1_state/history.rs:3417`.

- `SelectionDecisionEntry::new_with_traversal` validates selected candidate membership, computes the considered-order hash, and commits the candidate-set root/memberships from the considered payloads: `crates/ploke-eval/src/cli/prototype1_state/history.rs:3444`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3453`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3459`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3463`.

- Candidate-set membership commits a candidate subject and full payload hash under a sparse-Merkle proof; this is persisted in `CandidateSetCommitment`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:3047`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:3055`.

- `History::candidates` reads verified sealed blocks, validates decision observation hash, considered-order hash, candidate-set commitment, and returns each considered payload with block hash/height/lineage/entry provenance, selected-by-this-decision flag, payload hash, candidate-set root, and membership proof: `crates/ploke-eval/src/cli/prototype1_state/history.rs:886`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:891`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:906`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:915`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:924`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:934`.

- Current-generation candidates are built from typed child outcomes with sealed evidence and Artifact payloads, then appended to History candidates before active traversal selection: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5621`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5630`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5638`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5704`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5705`. The append creates current-generation candidate-set proofs in memory: `crates/ploke-eval/src/successor_selection/traversal.rs:308`.

- The selected material becomes durable only through handoff: the parent converts selection material into a `SelectionDecisionEntry`, passes it to `spawn_and_handoff_prototype1_successor`, admits it as a decision entry, and appends the sealed block: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6355`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6358`, `crates/ploke-eval/src/cli/prototype1_process.rs:1014`, `crates/ploke-eval/src/cli/prototype1_process.rs:1097`, `crates/ploke-eval/src/cli/prototype1_process.rs:1119`, `crates/ploke-eval/src/cli/prototype1_process.rs:1141`.

## 3. Transient Or Non-Persisted Data Inventory

- Active traversal scores are derived in memory from `CandidateCase`, which borrows only `selection_input`, `sealed_evidence`, and `artifact` from each payload: `crates/ploke-eval/src/successor_selection/traversal.rs:480`. This is good for replay, but `CandidateCase` itself is not persisted.

- Decision-grade exclusion is persisted only as projection failures once there is a selected `SelectionDecisionEntry`; the excluded candidate object is not kept as an active scorer item: `crates/ploke-eval/src/successor_selection/traversal.rs:581`, `crates/ploke-eval/src/successor_selection/traversal.rs:646`.

- The active `score_child_prop` per-candidate weight table is not persisted. The selection rationale keeps total weight, sample, selected weight, and selected components, but not every candidate's performance, alpha, exploitation, exploration, or weight: `crates/ploke-eval/src/successor_selection/traversal.rs:842`, `crates/ploke-eval/src/successor_selection/traversal.rs:870`, `crates/ploke-eval/src/successor_selection/traversal.rs:891`.

- The `frontier_max` strategy likewise does not persist a full per-candidate score table; it keeps strategy parameters and optional max normalization in rationale: `crates/ploke-eval/src/successor_selection/traversal.rs:760`, `crates/ploke-eval/src/successor_selection/traversal.rs:807`.

- `selected_from_current_generation` is held in the transient `Selection`/`SelectionSealMaterial` path and drives handoff source selection; it is not a first-class field on `SelectionDecisionEntry`: `crates/ploke-eval/src/successor_selection/traversal.rs:189`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:355`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5833`.

- Current-generation sealed compared-run evidence stores operational metrics but explicitly leaves protocol payloads and rich run snapshots empty: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5567`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5587`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5589`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5592`. If `--successor-selection-metrics operational-and-protocol` is used, current-generation protocol contributions are therefore unavailable unless supplied by historical sealed payloads from another path.

- Score reports are projections, not sealed score records. The score module says it consumes grouped evidence and does not admit anything into History: `crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:1`, `crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:10`.

- The legacy generation-local summary now used by score/report projection only is separate from active selection: `crates/ploke-eval/src/successor_selection/operator_projection.rs:1`, `crates/ploke-eval/src/successor_selection/operator_projection.rs:12`.

## 4. CLI Visibility

- `loop prototype1-monitor history-preview` and `history preview` are the relevant sealed-History surfaces. They build a preview including `sealed_selection_commitments`, scan the sealed block segment, and project decision rows: `crates/ploke-eval/src/cli.rs:517`, `crates/ploke-eval/src/cli.rs:553`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1456`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1488`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:517`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:584`.

- The history preview table prints sealed selection row counts, digest status, segment path, block line/height/hash, procedure, scope, selected candidate, candidate-set root, projection failure notes, candidate subjects, eligibility gaps, selection-input binding status, source ref/hash counts, and sealed evidence row counts: `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1052`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1091`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1110`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1130`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1139`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1158`.

- The history preview JSON has more structured projection rows, but the row schema is still a summary. It includes hashes, selected candidate, candidate subjects, binding checks, source counts, sealed row counts, and identity gaps; it does not include full `SelectionDecisionEntry`, full `SelectionInput`, traversal seed/strategy, decision rationale, candidate-set proof bytes, or Artifact payload: `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:520`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:535`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:561`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:713`.

- `history child-evidence` / `loop prototype1-monitor child-evidence` prints grouped child evidence from current campaign records. JSON is the richer option; table output is intentionally summarized: `crates/ploke-eval/src/cli.rs:509`, `crates/ploke-eval/src/cli.rs:544`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:492`, `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:818`.

- `history scores` / `loop prototype1-monitor history-scores` prints score projections over child evidence. Table output shows node/branch/state/score/evaluation counts/comparable runs/missing metrics/diagnostics; JSON carries the `ScoreSet`: `crates/ploke-eval/src/cli.rs:513`, `crates/ploke-eval/src/cli.rs:548`, `crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:66`, `crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:200`, `crates/ploke-eval/src/cli/prototype1_state/score/mod.rs:398`.

- `history score-selection-review` / monitor `score-selection-review` is useful but projection-heavy. It compares score rows with `SelectionInput` decisions and the legacy generation summary; it explicitly does not perform archive traversal or History admission: `crates/ploke-eval/src/cli.rs:515`, `crates/ploke-eval/src/cli.rs:551`, `crates/ploke-eval/src/cli/prototype1_state/score/select.rs:55`, `crates/ploke-eval/src/cli/prototype1_state/score/select.rs:164`.

- `loop prototype1-monitor report` is a provisional aggregate over branch registry, transition journal, and evaluation reports. Its own module docs say it is not sealed History: `crates/ploke-eval/src/cli/prototype1_state/report.rs:1`.

## 5. Gaps

- No CLI command prints the full sealed `SelectionDecisionEntry` from History. The raw data is in the segment, but `history preview` projects a summary row.

- No CLI command prints full candidate `SelectionInput`s from the sealed considered set, including every compared instance's parent/child metrics, for a selected History decision.

- No CLI command prints traversal seed/strategy or full `SuccessorDecision.rationale` from the sealed decision, even though those are persisted.

- No CLI command replays active `HistoryScoreChildProp` over the sealed considered set and prints all per-candidate weights. The existing score-selection review is a generation-local operator projection, not the active cross-History traversal.

- No CLI command shows whether the selected candidate came from current-generation append vs previously sealed History as an explicit field.

- No CLI command prints candidate-set membership proof details; it only reports candidate-set commitment/binding status.

- Current-generation protocol scoring evidence is not sealed by the current append path, so protocol-based selection debugging after a run will be incomplete if protocol metrics are enabled.

## 6. Run-Readiness Implication

A short rerun should produce enough persisted evidence to debug the next successful successor selection at the structural level: candidate universe, selected candidate, considered payload hashes, selection-input bindings, decision-grade gaps, selected decision, traversal seed/strategy, and selected rationale are persisted in the sealed History block.

It will not produce a fully CLI-visible scoring audit. From the CLI alone, operators can verify that sealed selection data exists and inspect summarized candidate eligibility, but they cannot print the full scorer inputs or all active traversal weights without adding a new command or reading raw History JSON.

For a short rerun, this is acceptable if the goal is to test whether selection and handoff now cross the History boundary. For a long run, the current CLI gap will slow diagnosis of a surprising selection because the decisive data is persisted but not directly inspectable through bounded commands.

## 7. Recommended Minimal Follow-Ups

1. Add a bounded `history selection-show` or `history preview --selection <row>` view that prints one sealed `SelectionDecisionEntry`: traversal seed/strategy, decision outcome/rationale, considered order, selected candidate, and compact per-candidate `SelectionInput`/sealed-evidence/artifact presence.

2. Add an active traversal replay view for `HistoryScoreChildProp` that prints per-candidate performance, child count, alpha, exploitation, exploration, weight, selected sample, and selected index from a sealed decision.

3. If protocol metrics will be used, persist protocol compared-run fields for current-generation payloads instead of sealing `baseline_protocol`, `treatment_protocol`, and run snapshots as `None`.

4. Before an overnight run, verify the first short rerun creates at least one sealed selection decision row with `row_ok=true`, `candidate_set_ok=ok`, and no unexpected decision-grade gaps via `history preview`.
