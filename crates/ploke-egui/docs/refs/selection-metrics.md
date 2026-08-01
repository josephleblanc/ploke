**Short Verdict**
We already evaluate and persist enough selection-time evidence to start showing a useful first metric panel: strategy/config, selected candidate, operational deltas, `imp@k`, and some protocol aggregates. The main gap is propagation/display: `ploke-tree::Graph` preserves only reduced selection headers and `imp@k` rows, while `ploke-egui` currently has no successor-selection inspector section.

**Metrics To Show**
For a convincing HyperAgents/DGM-like story, show these as selection-time facts, not post-hoc UI guesses:

- Task outcome: selected branch, branch disposition, successor outcome, keep/reject/stop/explore.
- Operational quality deltas: failed tool calls, patch attempted/applied, nonempty valid patch, convergence, aborted, retry counts.
- Protocol/process quality: reviewed calls/segments, missing/skipped reviews, crosswalk coverage, focused-progress counts, protocol delta points.
- Cross-generation improvement: `imp@k` baseline score, best descendant score, improvement, descendant count, scored descendant count.
- Selection mechanics: candidate count, selected source, traversal strategy, seed, metric inputs, oracle mode, `score_child_prop` selected components.
- Generalization: train/validation/held-out split or at least eval-set role labels. This is the major missing piece for strong HyperAgents-style claims.

**Current Status**
- Evaluated for selection today:
  - Operational scoring is used in `performance_score` via child metrics [traversal.rs](/home/brasides/code/ploke/crates/ploke-eval/src/successor_selection/traversal.rs:1008).
  - Protocol scoring is used only when `selection.evidence = "operational-and-protocol"` and treatment protocol evidence exists [metric.rs](/home/brasides/code/ploke/crates/ploke-eval/src/metric.rs:109).
  - `imp@k` is computed and persisted, but affects score only when `score_points_per_imp_point > 0`; in the inspected run it is `0` [run-profile.toml](/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/run-profile.toml:35).
- Persisted:
  - `SelectionDecisionEntryRecord` stores traversal, considered payloads, metrics, and decision [payload.rs](/home/brasides/code/ploke/crates/ploke-records/src/history/payload.rs:687).
  - `MetricSet` / `MetricCandidate` / `ImpAtK` are durable record types [selection.rs](/home/brasides/code/ploke/crates/ploke-records/src/selection.rs:130).
  - Protocol aggregates are in `ComparedRunEvidenceRecord.{baseline_protocol,treatment_protocol}` [payload.rs](/home/brasides/code/ploke/crates/ploke-records/src/history/payload.rs:168).
- Propagated through `ploke-tree::Graph`:
  - Selection header, candidate joins, metric set, and `imp@k` rows are ingested [selection.rs](/home/brasides/code/ploke/crates/ploke-tree/src/graph/build/selection.rs:25), [selection.rs](/home/brasides/code/ploke/crates/ploke-tree/src/graph/types/selection.rs:97).
  - Protocol aggregate values are not carried as a selection-scoped Graph witness.
- Displayed in `ploke-egui`:
  - Current inspector displays artifact/run-forest identity, roles, patch generation, run records, graph/artifact edges, patch debug, source refs, artifact ids [shell.rs](/home/brasides/code/ploke/crates/ploke-egui/src/ui/app/shell.rs:370).
  - No UI section displays successor-selection metrics; `GraphSelectionRef` only supports `Artifact` and `RunForestNode` [mod.rs](/home/brasides/code/ploke/crates/ploke-egui/src/ui/view/mod.rs:271).

**Concrete Persisted Examples**
- Config:
  - [run-profile.toml](/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/run-profile.toml:26) sets `history-score-child-prop`, `operational-and-protocol`, `persist = true`, `imp_at_k.enabled = true`, and `score_points_per_imp_point = 0`.
  - Type: `ploke_records::run_profile::Selection` [run_profile.rs](/home/brasides/code/ploke/crates/ploke-records/src/run_profile.rs:124).
- Sealed selection History:
  - [segment-000000.jsonl](/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/history/blocks/segment-000000.jsonl:1) contains selection entry `74c962e5...`, selected `candidate:node-8797...`, traversal `score_child_prop`, metrics id `156d667d...`, and `imp_at_k` rows.
  - Type path: `SealedBlockRecord -> SelectionDecisionEntryRecord -> MetricSet`.
  - Graph: reachable as `SelectionNode`, `CandidateNode`, `MetricSetNode`, `MetricCandidateNode`; not enough for protocol metrics.
- Branch evaluation artifact:
  - [branch-dc634b257dc8a3e7.json](/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/evaluations/branch-dc634b257dc8a3e7.json:49) contains baseline/treatment `RunMetrics`.
  - Type: `ploke_records::evaluation::Artifact`, `InstanceComparison`, `RunMetrics` [evaluation.rs](/home/brasides/code/ploke/crates/ploke-records/src/evaluation.rs:17).
  - Graph: attached as `CandidateEvaluation` evidence by branch, but raw metrics are not promoted into a metric witness.
- Successor journal:
  - [transition-journal.jsonl](/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/transition-journal.jsonl:129) records `selection_decision.findings[].metrics` and `score_child_prop_selected_components`.
  - Type: `JournalEntry::Successor(SuccessorRecord)` with `SuccessorStateRecord::Selected { selection_decision }` [journal.rs](/home/brasides/code/ploke/crates/ploke-records/src/journal.rs:357).
  - Graph: loaded as journal evidence lines, not as the canonical selection metric display source.

**Next Implementation Slice**
Add a `ploke-tree` selection metric witness keyed by `(selection_entry_id, payload_index, branch_id)` carrying traversal summary, operational score inputs, `imp@k`, and protocol aggregate pairs. Then add an egui `Selection` inspector section that borrows this witness. Do not make `ploke-egui` parse History JSON directly.
