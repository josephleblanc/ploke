# Selection Protocol Evidence

## 1. Target Entity / Visible Claim

The visible claim is: for a selected Artifact that corresponds to a candidate
considered by successor selection, the Inspector can show a `Selection` header
with the selection procedure/policy, traversal policy, selected candidate
coordinate, and protocol aggregate evidence considered during selection.

This is a selection-time evidence claim. It is not Artifact identity, not a
run-record output claim, and not a raw `protocol-artifacts/*.json` drilldown.
It answers: "what did the sealed selection entry say about this candidate and
the protocol metrics available to the selector?"

Status: graph/UI read-model carrier implemented for candidate comparison.
`ploke_tree::Graph` exposes `ChildPlanIndex`, `CandidateNode`,
`SelectionNode`, `MetricSetNode`, and `MetricCandidateNode`, which are enough
for `ploke-egui` to show the parent-owned child universe, selected candidate
marker, selection metric policy, `imp@k` rows, and the sealed compared-run
operational/protocol metric inputs attached to each considered candidate
without reparsing History or child-plan JSON. A future witness can still expose
the full traversal replay and per-candidate score weights; the Inspector must
label missing metric fields as `not_recorded` rather than inventing scores.

## 2. Competing IDs and Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `SelectionDecisionEntryRecord.procedure_or_policy` | `ploke_records::history::SelectionDecisionEntryRecord.procedure_or_policy` | procedure / policy identity | primary procedure label for the selection entry |
| `SelectionDecisionEntryRecord.traversal` | `ploke_records::history::SelectionDecisionEntryRecord.traversal` | selection policy provenance | carries `seed`, `strategy`, `metrics`, `oracle`, and `selected_source` |
| `SelectionDecisionEntryRecord.decision` | `ploke_records::history::SelectionDecisionEntryRecord.decision` | durable decision projection | gives outcome, selected branch, domain findings, and rationale; current examples do not include a protocol domain finding |
| `EvaluationPayloadRecord.sealed_evidence.evaluations[].compared_runs[].{baseline_protocol,treatment_protocol}` | `ploke_records::history::ComparedRunEvidenceRecord` | typed selection evidence | chosen source for protocol aggregate metrics used by traversal scoring |
| `EvaluationPayloadRecord.artifact.resolved.branch.derived_artifact_id` | `ploke_records::history::CandidateArtifactRecord.resolved.branch.derived_artifact_id` | Artifact identity join | ties a considered candidate to the Artifact visible in the inspector |
| `EvaluationPayloadRecord.sealed_evidence.coordinate.branch_id` | `ploke_records::history::CandidateEvidenceRecord.coordinate.branch_id` | branch join key | ties selection evidence to branch-scoped evaluation and run-record refs |
| `ChildPlanRecord.parent_node_id` | `ploke_records::child_plan::ChildPlanRecord.parent_node_id` | parent-owned candidate universe handle | primary starting point for the Candidate Comparison section |
| `ChildPlanChildRecord.node.node_id` | `ploke_records::child_plan::ChildPlanChildRecord.node.node_id` | child candidate handle | joins one planned child to `CandidateNode.node_id` |
| `SelectionNode.{procedure_or_policy,scope,decision_outcome,metric_set_id}` | `ploke_tree::graph::SelectionNode` | graph read-model projection | reduced selection header facts |
| `CandidateNode.{selection_entry_id,payload_index,branch_id,artifact_after}` | `ploke_tree::graph::CandidateNode` | graph read-model join | currently enough to find the candidate, but not its protocol metrics |
| `SelectionMetricWitnessKey` / `SelectionMetricWitnessRef<'_>` | future `ploke_tree::graph` witness | graph read-model witness | not present in current code; needed later for traversal replay and per-candidate score-weight drilldown |
| `MetricSetNode` / `MetricCandidateNode` | `ploke_tree::graph::{MetricSetNode,MetricCandidateNode}` | selection metric projection | preserves metric-set identity, policy, `imp_at_k`, and compared-run metric inputs for candidate comparison |
| `ComparedRunMetricNode` | `ploke_tree::graph::ComparedRunMetricNode` | selection metric input projection | graph-owned projection of `ComparedRunEvidenceRecord` operational/protocol fields used by selection scoring |
| `ProtocolArtifactsEvidence.index` | `ploke_tree::ProtocolArtifactsEvidence.index` | passive protocol payload inventory | full typed per-run procedure artifacts, but not currently branch or selection scoped |
| `EvidenceSubject::ProtocolArtifact` | `ploke_tree::graph::EvidenceSubject::ProtocolArtifact` | reduced locator/provenance | summary and locator evidence only; not enough for a selection header |
| `ploke_records::evaluation::Artifact.compared_instances` | `ploke_records::evaluation::Artifact` | evaluation source handle | owns record paths and operational comparison data; selection-time protocol metrics are sealed elsewhere |
| `RunRecordEvidence.refs_by_branch` | `ploke_tree::RunRecordEvidence.refs_by_branch` | branch run-output join | primary for run records, not for selection protocol metrics |

## 3. Minting Sites

- `SelectionDecisionEntryRecord` is constructed through
  `ploke_eval::cli::prototype1_state::history::SelectionDecisionEntry::new_with_traversal_identity_metrics`.
  That constructor validates the selected candidate, `considered_order_hash`,
  candidate-set root, and metric binding before the History entry is sealed.
- `SelectionSealMaterial` is assembled in
  `ploke_eval::cli::prototype1_state::cli_facing::ParentSelection::select_successor`.
  It carries the selected candidate, considered payloads, traversal evidence,
  and `successor_selection::metrics::Set`.
- `successor_selection::metrics::Set::from_considered` mints the selection
  metric set id, candidate-set root binding, and per-candidate `imp_at_k`
  rows from the exact considered payload order.
- `ComparedRunEvidenceRecord.{baseline_protocol,treatment_protocol}` is sealed
  inside `EvaluationPayloadRecord.sealed_evidence` before the selected payload
  becomes History material. `successor_selection::traversal::performance_score`
  reads those protocol fields when traversal metrics are
  `operational_and_protocol`.
- Per-run protocol artifact JSON files are written by
  `ploke_eval::protocol_artifacts::write_protocol_artifact` under
  `<run>/protocol-artifacts`, and loaded by `ploke_tree::FsRunStore` as
  `ploke_records::protocol::Artifact`.
- `ploke_tree::graph::build::selection::Builder::ingest_selection` mints
  `SelectionNode`, `CandidateNode`, `CandidateBranchNode`, `MetricSetNode`,
  and `MetricCandidateNode` from the sealed selection entry.
- `ploke_tree::graph::types::child_plan::ChildPlanIndex` stores typed
  `ChildPlanRecord` boxes by parent scheduler node id and exposes
  `plan_for_parent_node_id`, `child_for_node_id`, and `child_for_patch_id`.
- `ploke-egui` does not make the successor candidate choice. The current UI
  selection path is:
  `GraphView::show -> egui_graphs::GraphView -> GraphViewCache::selected_payload
  -> GraphSelectionDetail -> SelectionInspector::from_reference`.
  `GraphSelectionRef` currently has only `Artifact { key }` and
  `RunForestNode { key }` variants.
- `default_selections` in `crates/ploke-egui/src/ui/inspector.rs` enumerates
  `graph.artifact_tree().nodes` and creates Artifact selections. The current
  CLI selector for the named Artifact resolves with the normalized graph key
  `git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c`, not with the full
  `artifact:git-commit:...` value.

## 4. Chosen Primary Carrier

Chosen upstream primary carrier:

```text
SelectionDecisionEntryRecord.considered[payload_index]
  -> EvaluationPayloadRecord.sealed_evidence
  -> EvaluationEvidenceRecord.compared_runs[]
  -> ComparedRunEvidenceRecord.{baseline_protocol,treatment_protocol}
```

Chosen Artifact join:

```text
selected Artifact
  -> ParentCreateAttempt::child().resolved.branch.branch_id
     or unique PatchInspection::branch_id()
  -> CandidateNode.{selection_entry_id,payload_index,branch_id,artifact_after}
  -> SelectionNode
```

Chosen Candidate Comparison join:

```text
selected Artifact or run-forest node
  -> parent scheduler node id
  -> ChildPlanIndex.plan_for_parent_node_id(parent_node_id)
  -> ChildPlanChildRecord.node.node_id
  -> CandidateNode.{selection_entry_id,payload_index}
  -> SelectionNode.metric_set_id
  -> MetricSetNode.policy
  -> MetricCandidateNode.{imp_at_k,compared_runs[]}
  -> ComparedRunMetricNode.{baseline_metrics,treatment_metrics,
       baseline_protocol,treatment_protocol}
```

Graph carrier used for rendering:

```text
ploke_records child-plan and selection records
  -> ploke_tree::Graph reduced child/candidate/selection/metric indexes
  -> borrowed ploke-egui Inspector witness
  -> render-only Candidate Comparison section
```

`ploke-egui` must not recover this by reading History JSON or scanning protocol
artifact files from the UI.

Important current UI boundary:

```text
ploke-egui visible selection
  = Artifact / RunForestNode graph node selection
  != successor candidate selection
```

The visible Artifact selection can be joined back to candidate evidence only if
`ploke_tree::Graph` exposes a selection-scoped witness. Without that witness,
the right panel can show Artifact identity, role badges, run records, edge
relations, patch debug, source refs, and Artifact ids for the selected graph
node, but it cannot safely show the sealed selection protocol metrics.

## 5. Rejected Alternatives

### `ProtocolArtifactsEvidence.index` is not the primary selection header source

It owns full typed procedure artifacts and is useful for future drilldown, but
it is a passive inventory keyed by artifact file path. It does not currently
prove which selected candidate, branch, or selection entry used the artifact.

### `EvidenceSubject::ProtocolArtifact` is not enough

The generic evidence index carries reduced summary and locator facts. It should
not be used for `input`, `output`, nested procedure payloads, or selection
scoring facts.

### `selection::Decision.findings` is not enough

The current run's decision findings show operational and oracle findings. The
fact that the traversal used `operational_and_protocol` appears in traversal
policy/rationale, while the protocol aggregate values live in sealed candidate
evidence.

### `MetricSetNode` / `MetricCandidateNode` are reduced evidence

The metric index now binds candidate order, policy, `imp_at_k` rows, and
compared-run operational/protocol metric values. It is enough for the Candidate
Comparison panel to show raw metric inputs used by selection.

It is still not enough for full traversal replay because the final
`score_child_prop` per-candidate weight vector is not a graph carrier. Selected
candidate rationale strings may mention selected components, but the UI should
not parse those strings into sibling score rows.

### `record.json.gz` and `RunRecordEvidence` are not primary

Run records are branch-arm output records. They can show turns and tool steps,
but the selection header should use the sealed selection payload that the
selector considered.

### Raw filesystem paths are not primary

Paths are useful locators and diagnostics. They must not become the UI's source
truth for this claim.

## 6. Upstream Record Types and Fields

- `ploke_records::history::SelectionDecisionEntryRecord`
  - `procedure_or_policy`
  - `scope`
  - `selected_candidate`
  - `selected_occurrence_id`
  - `selected_membership_id`
  - `considered`
  - `considered_sources`
  - `considered_order_hash`
  - `candidate_set`
  - `projection_failures`
  - `traversal`
  - `metrics`
  - `decision`
- `ploke_records::history::EvaluationPayloadRecord`
  - `candidate`
  - `procedure`
  - `selection_input`
  - `source_refs`
  - `sealed_evidence`
  - `artifact`
- `ploke_records::history::CandidateEvidenceRecord`
  - `coordinate`
  - `evaluations`
  - `branches`
  - `child_diagnostics`
- `ploke_records::history::EvaluationEvidenceRecord`
  - `branch_id`
  - `evaluation_procedure_id`
  - `evaluator_identity`
  - `eval_set_identity`
  - `overall_disposition`
  - `compared_runs`
- `ploke_records::history::ComparedRunEvidenceRecord`
  - `instance_id`
  - `status`
  - `baseline_metrics`
  - `treatment_metrics`
  - `oracle_evaluation`
  - `baseline_protocol`
  - `treatment_protocol`
  - `baseline_run`
  - `treatment_run`
- `ploke_records::history::ProtocolMetricsRecord`
  - `scanned_artifact_count`
  - `artifact_counts`
  - `total_calls_in_run`
  - `total_segments_in_anchor`
  - `reviewed_call_count`
  - `reviewed_segment_count`
  - `missing_call_count`
  - `missing_segment_count`
  - `skipped_segment_review_count`
  - `segment_anchor_mismatch_count`
  - `call_review_overall_counts`
  - `segment_review_overall_counts`
  - `call_review_confidence_counts`
  - `segment_review_confidence_counts`
  - `calls_with_segment_crosswalk`
  - `calls_without_segment_crosswalk`
  - `average_calls_per_anchor_segment_x1000`
  - `review_signal_totals`
- `ploke_records::selection::MetricSet`
- `ploke_records::selection::MetricPolicy`
- `ploke_records::selection::MetricCandidate`
- `ploke_records::selection::ImpAtK`
- `ploke_records::protocol::Artifact`
- `ploke_records::protocol::ArtifactBody`
- `ploke_records::evaluation::Artifact`
- `ploke_records::evaluation::InstanceComparison`

## 7. Graph / Read-Model Carriers

Current carriers:

- `ploke_tree::graph::SelectionIndex`
- `ploke_tree::graph::SelectionNode`
- `ploke_tree::graph::CandidateIndex`
- `ploke_tree::graph::CandidateNode`
- `ploke_tree::graph::CandidateBranchNode`
- `ploke_tree::graph::MetricIndex`
- `ploke_tree::graph::MetricSetNode`
- `ploke_tree::graph::MetricCandidateNode`
- `ploke_tree::graph::ComparedRunMetricNode`
- future full `SelectionMetricWitnessRef<'_>` for traversal replay and
  score-weight drilldown
- `ploke_tree::ProtocolArtifactsEvidence`
- `ploke_tree::ProtocolArtifactSummary`
- `ploke_tree::Graph::protocol_artifacts`
- `ploke_tree::RunRecordEvidence`
- `ploke_tree::Graph::run_record_refs_for_branch`
- `ploke_tree::graph::ParentCreateAttempt`

Implemented graph carrier for Candidate Comparison:

- `ChildPlanIndex.plan_for_parent_node_id(parent_node_id)` returns the
  committed parent-owned child universe.
- `CandidateNode { selection_entry_id, payload_index, node_id }` joins each
  child-plan child to the sealed selection entry when available.
- `SelectionNode.metric_set_id`, `MetricSetNode.policy`, and
  `MetricCandidateNode.{imp_at_k,compared_runs}` expose the metric policy,
  reduced improvement rows, and raw operational/protocol inputs available in
  the current graph.

Graph invariant needed for the UI:

```text
Artifact selection resolves to branch B
  and CandidateNode.branch_id == B
  and CandidateNode.artifact_after matches the selected Artifact
  and CandidateNode.{selection_entry_id,payload_index} identifies a considered
      payload in SelectionDecisionEntryRecord
  and MetricCandidateNode.compared_runs comes from the same payload's
      ComparedRunEvidenceRecord values
  therefore the Inspector may show selection protocol metric inputs for that
      Artifact.
```

The current graph satisfies this for raw metric inputs. It does not yet preserve
the final `score_child_prop` sibling weight rows.

## 8. Downstream UI Consumers

Current consumers using the typed witness:

- `crates/ploke-egui/src/ui/inspector.rs`
  - `InspectorSections`
  - `CandidateComparisonSlot`
  - `CandidateComparisonCandidate`
  - `SelectionInspector`
  - `RunForestNodeInspection`
  - `ArtifactInspection`
  - `SelectionInspectorSnapshot`
  - `SelectionSectionSnapshot`
  - `artifact_inspection`
  - `snapshot_artifact`
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_right_inspector`
  - `render_candidate_comparison_for_inspector`

Current right-panel sections in `render_right_inspector` include `Candidate
Comparison`, which renders parent plan children, selected marker, `imp@k`, and
compared-run metric deltas from graph-backed witnesses.

The UI witness should be borrowed from `ploke_tree::Graph`; it should not be a
row-shaped cache or direct JSON projection. A future shape should preserve:

- selection entry id
- payload index
- branch id
- candidate subject
- procedure or policy
- traversal strategy
- decision outcome
- compared-run protocol metrics
- compared-run operational metrics

## 9. Current Run Probe

For campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1
```

The named Artifact:

```text
artifact:git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c
```

is the resolved derived Artifact for:

```text
entry_id = 74c962e5-b4a0-4c3d-9eda-d1f885bc2cb7
candidate = candidate:node-8797f12b728b1edd:plan_index=0
node_id = node-8797f12b728b1edd
branch_id = branch-dc634b257dc8a3e7
generation = 1
```

The selection entry records:

```text
procedure_or_policy = successor-selection:history-traversal:v1
scope = history:all_admitted_candidates
strategy = score_child_prop
metrics = operational_and_protocol
oracle = record-only
selected_source = current_generation
decision.outcome = stop
decision.branch_disposition = reject
```

The selected candidate has one compared run:

```text
instance_id = BurntSushi__ripgrep-2209
evaluation_procedure_id = prototype1.branch_evaluation.operational_metrics.v1
overall_disposition = reject
baseline_protocol = present
treatment_protocol = missing
```

The available baseline protocol aggregate includes:

```text
scanned_artifact_count = 61
artifact_counts.tool_call_intent_segmentation = 1
artifact_counts.tool_call_review = 46
artifact_counts.tool_call_segment_review = 14
reviewed_call_count = 46
reviewed_segment_count = 14
missing_call_count = 0
missing_segment_count = 0
segment_anchor_mismatch_count = 0
calls_with_segment_crosswalk = 46
calls_without_segment_crosswalk = 0
```

The associated evaluation artifact is:

```text
/home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1/evaluations/branch-dc634b257dc8a3e7.json
```

It records the baseline and treatment `record.json.gz` paths, but the
treatment run directory has no `protocol-artifacts` files for this branch. That
matches the sealed selection evidence: baseline protocol is present and
treatment protocol is absent.

Second-pass `ploke-egui` CLI probe:

```text
cargo run -q -p ploke-egui --features dev -- \
  --run-root /home/brasides/.ploke-eval/campaigns/p1-selection-metrics-3g1x3-20260517-3/prototype1 \
  --inspect-node git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c
```

The CLI inspector resolved the visible selection as `selection: artifact A10`.
It showed Artifact identity, Child role, run records for the branch, graph
edges, patch debug, and source refs. It did not show selection procedure,
traversal policy, selection entry id, payload index, or protocol aggregate
metrics. The same command with the full
`artifact:git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c` selector failed
because the current visible graph key is the normalized
`git-commit:fded9bb00d2aa67ad57311393151fa402e158c3c` form.

Run-wide protocol persistence check:

```text
selection entries checked = 3
compared runs checked = 17
compared runs with both baseline_protocol and treatment_protocol = 0
compared runs with both baseline_run.protocol.artifacts and treatment_run.protocol.artifacts = 0
```

For the first selection entry, all three considered generation-1 candidates have
`baseline_protocol = present` and `treatment_protocol = missing`. For later
entries, some descendant candidates have treatment-only aggregate protocol and
some have no aggregate protocol on either arm, but no compared run in the
sealed History segment has aggregate protocol on both arms.

Filesystem protocol-artifact directories referenced by
`evaluations/*.json` show the same absence of fully persisted protocol data for
both arms:

```text
branch-dc634b257dc8a3e7  baseline files=61  treatment files=0
branch-71981537f386b0e9  baseline files=61  treatment files=0
branch-687ece2b9c9d33ad  baseline files=61  treatment files=0
branch-0e276430a7a7750a  baseline files=0   treatment files=1
branch-26cee57679f95eb4  baseline files=0   treatment files=0
branch-b22a41e76e2cf7d6  baseline files=0   treatment files=0
branch-0f40ff7d83c063ec  baseline files=0   treatment files=1
branch-e29e4e094a911f8c  baseline files=0   treatment files=0
```

The treatment file count of `1` on two later branches is only an intent
segmentation artifact, not the full baseline-like protocol set. There is no
branch in this run with protocol artifacts fully persisted for both baseline
and treatment arms.

## 10. Open Gaps and Caveats

- The graph/UI witness is implemented for selected Artifact drilldown. Rendering
  it from `ploke-egui` by reparsing History JSON would still violate the UI
  projection rule.
- Older runs may have no `SelectionDecisionEntryRecord.metrics`, no traversal
  evidence, no protocol aggregate fields, or no per-run protocol artifacts.
  Missing evidence must render as unavailable/absent, not as zero.
- `ProtocolMetricsRecord` is an aggregate used by selection. Full
  `ploke_records::protocol::ArtifactBody` drilldown is a related but separate
  claim and needs its own branch/selection join before UI exposure.
- Current `ploke-egui` Artifact selection can find run records through
  `ParentCreateAttempt::child().resolved.branch.branch_id` or a unique patch
  branch, but that is the run-output path, not the selection-protocol evidence
  path.
- `decision.rationale` contains useful strategy strings such as
  `metric_inputs=operational_and_protocol`, but those strings should be
  secondary display context. The typed traversal record and protocol metric
  records are the source facts.
- The `9f7c44801024a388959a045d9049c25313e283db6f4a8c993b141173af6fff43`
  value supplied with the Artifact appears as a tree-key/hash-style carrier in
  child-plan material, not as the protocol evidence source. It can help the
  Artifact identity section, but it should not identify selection protocol
  evidence.
