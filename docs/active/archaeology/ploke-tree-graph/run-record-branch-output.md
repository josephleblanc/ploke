# Run Record Branch Output

## 1. Target Entity / Visible Claim

The visible claim is: for a selected run-forest node, the Inspector can show the
baseline and treatment `record.json.gz` outputs associated with that node's
evaluation branch.

This is a branch-scoped run-output claim, not Artifact identity and not History
authority. It answers: "what did the benchmark run record say happened for this
branch arm?"

## 2. Competing IDs and Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `TreeNode.branch_id` | `ploke_tree::TreeNode.branch_id` | join key | selected run-forest node's branch handle; primary UI lookup key for this claim |
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

Why:

- the selected run-forest node already carries a branch id
- evaluation artifacts are keyed by branch id and own the baseline/treatment
  record paths
- the graph only creates a `BranchRunRecordRef` after the compressed run record
  was successfully parsed as a typed `RunRecord`
- the arm marker distinguishes baseline from treatment without relying on path
  substrings

## 5. Rejected Alternatives

### `record.json.gz` path alone is not primary

The path is a locator. It does not say whether the record is baseline or
treatment for the selected branch unless joined through
`InstanceComparison`.

### `RunRecord.manifest_id` is not primary

For Multi-SWE-Bench runs it can equal the instance id and can repeat across
baseline/treatment arms. It identifies the run payload, not the comparison arm.

### Artifact ids are not primary

The claim is about branch-scoped benchmark run output. Artifact ids may later
join this data to artifact/runtime lineage, but they are not required to show
the selected branch's baseline/treatment output records.

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

Graph invariant:

```text
TreeNode.branch_id == EvaluationArtifact.branch_id
  and InstanceComparison.<arm>_record_path parses as RunRecord
  therefore BranchRunRecordRef may expose that arm's typed RunRecord output
  for the selected branch.
```

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/inspector.rs`
  - `RunRecordBranchInspection`
  - `RunRecordInspection`
  - `RunRecordSnapshot`
  - selected run-forest-node snapshot reconstruction
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_run_records_for_inspector`

## 9. Open Gaps and Caveats

- Artifact selections do not yet get run-record output unless a future typed
  artifact-to-branch join proves a unique branch. This report only unblocks
  selected run-forest nodes.
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
