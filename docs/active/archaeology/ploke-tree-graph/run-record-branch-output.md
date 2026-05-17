# Run Record Branch Output

## 1. Target Entity / Visible Claim

The visible claim is: for a selected run-forest node, or for a selected
Artifact with a unique graph-owned child/patch branch, the Inspector can show
the baseline and treatment `record.json.gz` outputs associated with that
evaluation branch.

This is a branch-scoped run-output claim, not Artifact identity and not History
authority. It answers: "what did the benchmark run record say happened for this
branch arm?"

## 2. Competing IDs and Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `TreeNode.branch_id` | `ploke_tree::TreeNode.branch_id` | join key | selected run-forest node's branch handle; primary UI lookup key for this claim |
| `ParentCreateAttempt::child().resolved.branch.branch_id` | `ploke_tree::graph::ParentCreateAttempt` over `ploke_records::child_plan::ChildPlanChildRecord` | join key | selected Artifact's producing child branch when `Graph::parent_create_for_artifact_key` proves a unique attempt |
| `PatchInspection::branch_id()` | `ploke_records::child_plan::ChildPlanChildRecord.resolved.branch.branch_id` | join key | fallback selected Artifact branch when artifact patch drilldown has exactly one branch |
| `EvaluationArtifact.branch_id` | `ploke_records::evaluation::Artifact.branch_id` | join key / evidence owner | keys evaluation evidence and owns compared baseline/treatment record paths |
| `InstanceComparison.{baseline_record_path,treatment_record_path}` | `ploke_records::evaluation::InstanceComparison` | source handle | concrete paths to the compressed run records for each compared arm |
| `BranchRunRecordRef.{branch_id,instance_id,arm,record_key,record_path}` | `ploke_tree::BranchRunRecordRef` | graph read-model join | branch-scoped ref minted during `FsRunStore` ingestion after the run record parses |
| `RunRecordStats.{turn_count,tool_call_count,failed_tool_call_count}` | `ploke_tree::RunRecordStats` | graph read-model derived facts | per-record stats computed once at ingestion to avoid render-frame scans |
| `RunRecord.manifest_id` | `ploke_records::run_record::RunRecord.manifest_id` | run identity handle | identifies the benchmark task/run record payload, not the branch comparison by itself |
| `RunRecord.metadata.benchmark.instance_id` | `ploke_records::run_record::BenchmarkMetadata.instance_id` | target identity | identifies the benchmark instance inside the run output |
| file path string alone | filesystem path to `record.json.gz` | fallback / source handle | useful locator only; must not be the sole semantic source |

## 3. Minting Sites

- `TreeNode.branch_id` is assembled from scheduler/node records in
  `ploke_tree::TreeNode::from_scheduler_node`.
- `ParentCreateAttempt` is resolved by `Graph::parent_create_for_artifact_key`
  from graph-owned child-plan/parent-create joins; its `child()` record carries
  the treatment branch id.
- `PatchInspection::branch_id()` is a borrowed view over
  `ChildPlanChildRecord.resolved.branch.branch_id`.
- `EvaluationArtifact.branch_id` and compared record paths are persisted by
  Prototype 1 evaluation artifacts under `prototype1/evaluations/*.json`.
- `BranchRunRecordRef` is minted by
  `crates/ploke-tree/src/store/fs.rs` when `FsRunStore` loads an evaluation
  artifact, follows its baseline/treatment record paths, and successfully
  parses `record.json.gz` through `ploke_records::run_record::RunRecord`.
- `RunRecord` is emitted by `ploke-eval` as `record.json.gz`; the passive
  shared schema lives in `ploke_records::run_record`.

## 4. Chosen Primary Carrier

Chosen primary carrier for selected run-forest Inspector lookup:
`TreeNode.branch_id -> RunRecordEvidence.refs_by_branch -> BranchRunRecordRef`.

Chosen primary carrier for selected Artifact Inspector lookup:
`ParentCreateAttempt::child().resolved.branch.branch_id -> RunRecordEvidence.refs_by_branch -> BranchRunRecordRef`.

Fallback for selected Artifact Inspector lookup:
`PatchInspection::branch_id() -> RunRecordEvidence.refs_by_branch -> BranchRunRecordRef`,
only when all patch witnesses for the artifact resolve to the same branch id.

Why:

- the selected run-forest node already carries a branch id
- evaluation artifacts are keyed by branch id and own the baseline/treatment
  record paths
- the graph only creates a `BranchRunRecordRef` after the compressed run record
  was successfully parsed as a typed `RunRecord`
- the arm marker distinguishes baseline from treatment without relying on path
  substrings
- for Artifact selections, a unique parent-create attempt is stronger than
  artifact id text because it ties the displayed Artifact to the child branch
  that evaluation artifacts use
- ambiguous multi-branch artifact patch witnesses are rejected rather than
  merged into a misleading run-record section

## 5. Rejected Alternatives

### `record.json.gz` path alone is not primary

The path is a locator. It does not say whether the record is baseline or
treatment for the selected branch unless joined through
`InstanceComparison`.

### `RunRecord.manifest_id` is not primary

For Multi-SWE-Bench runs it can equal the instance id and can repeat across
baseline/treatment arms. It identifies the run payload, not the comparison arm.

### Artifact ids are not primary

The claim is about branch-scoped benchmark run output. Artifact ids can lead to
a producing child branch through graph-owned parent-create or patch witnesses,
but the artifact id text alone is not enough to select baseline/treatment run
records.

## 6. Upstream Record Types and Fields

- `ploke_records::evaluation::Artifact.branch_id`
- `ploke_records::evaluation::Artifact.compared_instances`
- `ploke_records::evaluation::InstanceComparison.instance_id`
- `ploke_records::evaluation::InstanceComparison.baseline_record_path`
- `ploke_records::evaluation::InstanceComparison.treatment_record_path`
- `ploke_records::run_record::RunRecord`
- `ploke_records::run_record::RunMetadata`
- `ploke_records::run_record::RunPhases`
- `ploke_records::run_record::TurnRecord`
- `ploke_records::run_record::ToolExecutionRecord`
- `ploke_records::run_record::PackagingPhase`
- `ploke_records::run_record::RunTimingSummary`
- `ploke_records::child_plan::ChildPlanChildRecord.resolved.branch.branch_id`

## 7. Graph / Read-Model Carriers

- `ploke_tree::PassiveEvidence.run_records`
- `ploke_tree::RunRecordEvidence.index`
- `ploke_tree::RunRecordEvidence.stats`
- `ploke_tree::RunRecordEvidence.refs_by_branch`
- `ploke_tree::BranchRunRecordRef`
- `ploke_tree::RunRecordStats`
- `ploke_tree::ComparedRunArm`
- `ploke_tree::Graph::run_records`
- `ploke_tree::Graph::run_record_refs_for_branch`
- `ploke_tree::Graph::parent_create_for_artifact_key`
- `ploke_tree::graph::ParentCreateAttempt`

Graph invariant:

```text
TreeNode.branch_id == EvaluationArtifact.branch_id
  and InstanceComparison.<arm>_record_path parses as RunRecord
  therefore BranchRunRecordRef may expose that arm's typed RunRecord output
  for the selected branch.
```

Artifact graph invariant:

```text
Graph::parent_create_for_artifact_key(selected_artifact) == Attempt(child)
  and child.resolved.branch.branch_id == EvaluationArtifact.branch_id
  and InstanceComparison.<arm>_record_path parses as RunRecord
  therefore BranchRunRecordRef may expose that arm's typed RunRecord output
  for the selected Artifact.
```

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/inspector.rs`
  - `RunRecordBranchInspection`
  - `RunRecordInspection`
  - `RunRecordSnapshot`
  - selected run-forest-node snapshot reconstruction
  - selected artifact snapshot reconstruction when a unique branch witness is available
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_run_records_for_inspector`

## 9. Open Gaps and Caveats

- Artifact selections get run-record output only when a graph-owned
  parent-create attempt or unique patch branch proves the branch. Ambiguous
  multi-branch artifact witnesses render as no run records rather than guessing.
- The first UI summary exposes setup/turn/tool/packaging/timing counts and
  locators. Full turn/tool drilldown should be added as typed nested witnesses,
  not as raw JSON rendering.
- The native Inspector path uses `RunRecordBranchInspection` as a borrowed
  branch view over graph evidence, so it does not allocate a run-record witness
  vector each frame. Snapshot/diagnostic export still materializes owned
  `RunRecordSnapshot` rows at the explicit export boundary.
- Run-record counts are computed during `FsRunStore` ingestion as graph
  read-model facts. The visible egui renderer consumes borrowed path strings
  through `Path::to_str`, stack-formatted numeric labels, and static enum
  labels rather than heap-formatting those values while the section is open.
- This claim is evaluation evidence. It does not upgrade run-output facts into
  History authority.
