# Protocol And Evaluation Data Locations

This note records where Prototype 1 evaluation, protocol, and per-run record
data lives when `ploke-egui` loads a `ploke_tree::Graph`.

The UI rule is unchanged: `ploke-egui` renders typed facts from
`ploke_tree::Graph`. It should not parse run JSON, compressed records, rendered
CLI output, or copied path strings directly.

## Import Entry Point

`ploke-egui` imports a run root through:

```text
crates/ploke-egui/src/import/mod.rs
  graph_from_run_root(run_root)
    -> ploke_tree::FsRunStore::new(run_root).load_record_set()
    -> ploke_tree::Graph::from_records(&records)
```

For Prototype 1 campaign imports, `run_root` is normally:

```text
~/.ploke-eval/campaigns/<campaign-id>/prototype1
```

For the current example:

```text
/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1
```

## Evaluation Artifacts

Branch evaluation files live under the campaign run root:

```text
<run_root>/evaluations/<branch-id>.json
```

They are loaded by `ploke-tree` as:

```text
ploke_records::evaluation::Artifact
  -> ploke_tree::EvaluationEvidence.index
  -> RunForest.passive_evidence.evaluations
  -> Graph.forest.passive_evidence.evaluations
```

Important fields for UI drilldown:

```text
branch_id
overall_disposition
reasons
compared_instances[].instance_id
compared_instances[].baseline_registration_path
compared_instances[].treatment_registration_path
compared_instances[].baseline_record_path
compared_instances[].treatment_record_path
compared_instances[].baseline_metrics
compared_instances[].treatment_metrics
compared_instances[].evaluation
compared_instances[].status
```

The graph builder currently also creates a reduced evidence attachment for each
evaluation branch:

```text
EvidenceSubject::Branch(branch_id)
EvidenceKind::CandidateEvaluation
EvidenceLocator::EvaluationArtifact { path }
```

That attachment is useful as a locator/provenance breadcrumb. The full typed
evaluation object remains available through `Graph.forest.passive_evidence`.

## Example Evaluation Pair

For branch:

```text
branch-b53a075a894cedf9
```

The evaluation file is:

```text
/home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1/evaluations/branch-b53a075a894cedf9.json
```

It records:

```text
overall_disposition = reject
baseline tool_calls_total = 18
baseline tool_calls_failed = 2
baseline patch_apply_state = applied
baseline submission_artifact_state = nonempty
baseline aborted = false
baseline oracle_eligible = true

treatment tool_calls_total = 39
treatment tool_calls_failed = 8
treatment patch_apply_state = no
treatment submission_artifact_state = empty
treatment aborted = true
treatment oracle_eligible = false
```

The compared run records are:

```text
baseline_record_path =
/home/brasides/.ploke-eval/instances/prototype1/p1-five-gen-1x3-20260516-1/BurntSushi__ripgrep-2209/runs/run-1778950623675-structured-current-policy-da5906b1/record.json.gz

treatment_record_path =
/home/brasides/.ploke-eval/instances/prototype1/p1-five-gen-1x3-20260516-1/treatments/branch-b53a075a894cedf9/instances/BurntSushi__ripgrep-2209/runs/run-1778953269791-structured-current-policy-6d0dc6f7/record.json.gz
```

## Protocol Evidence Records

Protocol files, when present, live beside each benchmark run record:

```text
<record.json.gz parent>/protocol-artifacts/*.json
```

The directory is derived from each evaluation comparison by:

```text
baseline_record_path.parent().join("protocol-artifacts")
treatment_record_path.parent().join("protocol-artifacts")
```

`ploke-tree` also checks `<run_root>/protocol-artifacts` and any explicit
directories passed through `FsRunStore::with_protocol_artifacts_dir(s)`.

The writer-side helper in `ploke-eval` uses the same layout:

```text
crates/ploke-eval/src/layout.rs
  protocol_artifacts_dir_for_run(run_dir) = run_dir.join("protocol-artifacts")
```

Run registrations may also declare the configured protocol directory under the
run's artifact refs. For example:

```text
~/.ploke-eval/registries/runs/<run-id>.json
```

For `p1-five-gen-1x3-20260516-1`, the evaluation records derive candidate
protocol directories, but the directories do not currently exist. That means
`Graph::protocol_artifacts()` is `None` for this run today.

## Protocol Record Shape

Protocol files are loaded by `ploke-tree` as:

```text
ploke_records::protocol::Artifact
```

That type name is historical and overloaded. In UI/model discussion, treat it
as a passive protocol evidence record, not as a Prototype 1 checkout Artifact.

Each protocol evidence record contains:

```text
schema_version
procedure_name
subject_id
run_id
created_at_ms
model_id
provider_slug
input
output
artifact
```

Supported procedure names:

```text
tool_call_intent_segmentation
tool_call_review
tool_call_segment_review
intervention_issue_detection
intervention_synthesis
intervention_apply
```

When directories exist, `ploke-tree` stores the full typed protocol payloads in:

```text
Graph::protocol_artifacts()
  -> ProtocolArtifactsEvidence.index
  -> ploke_records::protocol::Artifact
  -> ArtifactBody::{...}
```

The generic graph evidence index only keeps reduced summary/locator data:

```text
EvidenceSubject::ProtocolArtifactSummary { counts... }
EvidenceSubject::ProtocolArtifact { procedure_name, subject_id, run_id }
EvidenceLocator::ProtocolArtifact { path, procedure_name, subject_id, run_id }
```

Do not use those reduced evidence attachments when the UI needs the full
protocol `input`, `output`, or nested provenance payload. Borrow from
`Graph::protocol_artifacts()` instead.

## Run Records

The `baseline_record_path` and `treatment_record_path` point to compressed
per-run records:

```text
record.json.gz
```

These are currently owned by `ploke-eval` as:

```text
ploke_eval::record::RunRecord
```

The persisted shape includes:

```text
schema_version
manifest_id
metadata
phases.setup
phases.agent_turns
phases.patch
phases.validation
phases.packaging
db_time_travel_index
conversation
timing
```

For UI work, this is the next important boundary:

```text
evaluation artifact already reaches ploke_tree::Graph
record.json.gz does not currently reach ploke_tree::Graph
```

Because `ploke-tree` should not depend on live `ploke-eval` internals, exposing
run-record details in `ploke-egui` needs a passive typed record/projection
boundary, not ad hoc gzip JSON parsing in the UI.

## UI Direction

Good first Inspector targets already available through `Graph`:

```text
branch_id
overall_disposition
evaluation reasons
baseline/treatment record paths
baseline/treatment registration paths
baseline/treatment RunMetrics
comparison status
candidate protocol-artifact dirs
loaded protocol summary, when present
```

Targets that need an additional typed graph ingestion slice:

```text
full RunRecord metadata
setup/indexing details
agent turn count and turn outcomes
tool-call sequence and tool failures from RunRecord
patch packaging paths and patch artifact details
timing summaries from RunRecord
conversation replay from RunRecord
```

Keep the render boundary typed: select borrowed graph/evidence records first,
then convert to `egui` text/widgets inside the Inspector renderer.
