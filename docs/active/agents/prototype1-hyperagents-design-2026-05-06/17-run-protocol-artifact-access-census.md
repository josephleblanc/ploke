# Agent 17 Run/Protocol Artifact Access Census

Date: 2026-05-06

Scope: `ploke-eval` run/protocol artifact storage and accessors. Primary files
inspected: `crates/ploke-eval/src/record.rs`, `runner.rs`,
`protocol_artifacts.rs`, `protocol_aggregate.rs`, `protocol_report.rs`,
`protocol_triage_report.rs`, `intervention_issue_aggregate.rs`,
`run_registry.rs`, `layout.rs`, and `inner/registry.rs`. Adjacent CLI builder
code in `crates/ploke-eval/src/cli.rs` was inspected where it produces or
projects these records.

This is a factual census for the HyperAgents design thread. It does not modify
runtime code.

## Concise Finding

Run/protocol evidence already has useful typed locator surfaces, but they are
not one unified evidence object.

- `RunRegistration` plus `RunArtifactRefs` is the strongest current locator for
  run identity, lifecycle, and canonical artifact paths.
- `RunRecord` in `record.json.gz` is the strongest typed source for per-run
  tool-call, LLM, setup, packaging, and replay facts.
- `StoredProtocolArtifact` files are the durable per-procedure protocol evidence
  envelope, with raw typed payloads stored as JSON `input`, `output`, and
  `artifact`.
- `ProtocolAggregate`, `IssueDetectionAggregate`, `ProtocolAggregateReport`,
  and `ProtocolCampaignTriageReport` are projections over those files. They are
  useful for child selection only if the selection record also carries refs back
  to the source artifacts.

The main lossy step is not file storage. It is the projection from persisted
artifacts into small refs, aggregate rows, reports, campaign triage rows, and
human-rendered tables. Those views collapse duplicate attempts, drop model and
schema provenance, summarize rationales, and often retain only counts or labels.

## Storage Roots And Persisted Paths

Default storage root comes from `layout::ploke_eval_home()`, using
`PLOKE_EVAL_HOME` or `~/.ploke-eval`.

Important layout helpers:

| Helper/type | Path convention | Notes |
| --- | --- | --- |
| `layout::registries_dir()` | `~/.ploke-eval/registries` | Root for run registration records. |
| `layout::instances_dir()` | `~/.ploke-eval/instances` | Default instance/campaign artifact area. |
| `layout::protocol_artifacts_dir_for_run(run_dir)` | `<run_dir>/protocol-artifacts` | Protocol artifact directory for one run attempt. |
| `RunStorageRoots::registry_path(run_id)` | `<registries_dir>/runs/<run_id>.json` | Canonical `RunRegistration` path. |
| `RunStorageRoots::run_root(run_id)` | `<runs_dir>/<run_id>` | Canonical per-attempt run directory. |
| `RunStorageRoots::record_path(run_id)` | `<runs_dir>/<run_id>/record.json.gz` | Canonical compressed `RunRecord`. |
| `RunArtifactRefs::from_frozen_spec` | many fixed filenames under `run_root` | Registration-time path census for one run. |

Runner allocation uses `allocate_run_output_dir(instance_dir, run_arm)`, which
creates a run id shaped like:

```text
run-<utc_timestamp_ms>-<run_arm.id>-<uuid8>
```

The run directory contains, depending on control/treatment mode:

```text
execution-log.json
repo-state.json
indexing-status.json
parse-failure.json
snapshot-status.json
indexing-checkpoint.db
indexing-failure.db
record.json.gz
final-snapshot.db
agent-turn-trace.json
agent-turn-summary.json
llm-full-responses.jsonl
multi-swe-bench-submission.jsonl
protocol-artifacts/
```

Provenance note: `RunStorageRoots::final_db_path` says `state-final.db`, while
`RunArtifactRefs::from_frozen_spec` and the runner path use
`final-snapshot.db`. Do not infer final DB path from only one helper without
checking which surface wrote the run.

## Run Locator Surfaces

### `RunRegistration`

Defined in `inner/registry.rs`.

`RunRegistration` is explicitly documented as the canonical authority surface
for run identity, lifecycle state, and artifact refs. It contains:

- `schema_version`
- `run_id`
- `intent: RunIntent`
- `frozen_spec: FrozenRunSpec`
- `spec_fingerprint`
- `lifecycle: RunLifecycle`
- `artifacts: RunArtifactRefs`

Important accessors and writers:

- `RunRegistration::register`
- `RunRegistration::register_with_run_id`
- `RunRegistration::persist`
- `RunRegistration::load`
- `RunRegistration::discover`
- `RunRegistration::run_root`
- `RunRegistration::record_path`
- `RunRegistration::final_db_path`
- `RunRegistration::update_phase`
- `RunRegistration::update_submission_status`
- `RunRegistration::update_protocol_anchor`
- `run_registry::register_live_run`
- `run_registry::persist_registration`
- `run_registry::load_registration_for_run_dir`
- `run_registry::load_registration_for_record_path`
- `run_registry::list_registrations_for_instance`
- `run_registry::preferred_registration_for_instance`
- `run_registry::completed_record_paths_for_instances_root`

Child-selection fit: this is the correct place to start when selecting among
child runs because it preserves role, run id, frozen intent, lifecycle, and all
artifact coordinates. A child selection evidence bundle should cite the
registration path or embed the `run_id` plus enough storage-root identity to
recover `RunRegistration`.

Loss: `RunRegistration` locates evidence but does not itself contain the
evidence payload. `lifecycle.detail` and `protocol_anchor` are status/progress
fields, not complete protocol facts.

### `RunArtifactRefs`

Defined in `inner/registry.rs`, re-exported by `run_registry.rs`.

Fields:

- `run_manifest`
- `run_root`
- `repo_state`
- `execution_log`
- `indexing_status`
- `parse_failure`
- `snapshot_status`
- `indexing_checkpoint_db`
- `indexing_failure_db`
- `record_path`
- `final_snapshot`
- optional `turn_trace`
- optional `turn_summary`
- optional `full_response_trace`
- optional `msb_submission`
- `protocol_artifacts_dir`
- optional `protocol_anchor`

Child-selection fit: `RunArtifactRefs` is the compact path inventory that a
selection record can carry or dereference. It distinguishes the canonical
`record_path` from sidecars and names `protocol_artifacts_dir` directly.

Loss: it is path-only. It has no content digest, schema digest, artifact
creation time per path, or admission authority. `protocol_anchor` is updated to
the latest protocol artifact path, not necessarily the segmentation anchor used
by a later aggregate.

### Runner Return Path Structs

Defined in `runner.rs`.

- `RunArtifactPaths`
- `AgentRunArtifactPaths`
- `BatchRunArtifactPaths`
- `BatchInstanceResult`
- `BatchRunSummary`

`RunArtifactPaths` fields overlap `RunArtifactRefs`, but `record_path` is
`Option<PathBuf>` and the type is a runner return artifact rather than the
canonical registration record. `AgentRunArtifactPaths` adds `turn_trace` and
`turn_summary`. `BatchInstanceResult` carries per-instance run results with
optional `execution_log`, `record_path`, `turn_summary`, and `msb_submission`.

Child-selection fit: these are useful immediately after a runner invocation, but
selection should resolve them into `RunRegistration`/`RunArtifactRefs` before
admission or archive-wide comparison.

Loss: these structs encode what this runner returned, not what is authoritative
or still present after registration sync. Optional paths can be absent on failed
runs.

## Run Record Storage And Accessors

### `RunRecord`

Defined in `record.rs`, written as compressed JSON to `record.json.gz`.

Top-level fields:

- `schema_version`
- `manifest_id`
- `metadata: RunMetadata`
- `phases: RunPhases`
- `db_time_travel_index: Vec<TimeTravelMarker>`
- `conversation: Vec<ConversationMessage>`
- optional `timing: RunTimingSummary`

Important typed accessors:

- `RunRecord::timestamp_for_turn`
- `RunRecord::tool_calls_in_turn`
- `RunRecord::turn_record`
- `RunRecord::llm_response_at_turn`
- `RunRecord::total_token_usage`
- `RunRecord::total_token_cost`
- `RunRecord::turn_count`
- `RunRecord::outcome_summary`
- `RunRecord::conversations`
- `RunRecord::tool_calls`
- `RunRecord::db_snapshots`
- `RunRecord::failures`
- `RunRecord::config`
- `RunRecord::was_tool_used`
- `RunRecord::turns_with_tool`
- `RunRecord::replay_state_at_turn`

Reader/writer functions:

- `write_compressed_record(path, record)`
- `read_compressed_record(path)`

Builder bridge:

- trait `RunRecordBuilder`
- `RunRecordBuilder::add_turn_from_artifact`
- `RunRecordBuilder::finalize`

Runner writes:

- control/setup path writes `execution-log.json`, sidecars, then calls
  `write_compressed_record(&record_path, &run_record)`
- agent path writes `agent-turn-trace.json`, `agent-turn-summary.json`, full
  response slice, packaging information, then writes `record.json.gz`

Child-selection fit: `RunRecord` is the primary typed source for operational
selection evidence: tool-call count and failures, prompt/response capture,
turn-level DB timestamps, provider/model metadata, patch packaging state, and
timing. Current overview code already reads it through `read_compressed_record`
for tool campaign summaries and protocol inputs.

Loss: `RunRecordBuilder::add_turn_from_artifact` converts an
`AgentTurnArtifact` into a `TurnRecord` and stores the full artifact, but some
fields are still compatibility-oriented. Turn `started_at` and `ended_at` are
filled with `chrono::Utc::now()` at builder time. `RunRecord::tool_calls()` may
fallback-extract from `agent_turn_artifact.events`, so consumers should treat
old records as a compatibility source unless the direct `tool_calls` field is
populated. There is no artifact digest tying `record.json.gz` to registration or
History admission.

## Protocol Artifact Storage

### `StoredProtocolArtifact`

Defined in `protocol_artifacts.rs`.

Fields:

- `schema_version`
- `procedure_name`
- `subject_id`
- `run_id`
- `created_at_ms`
- optional `model_id`
- optional `provider_slug`
- `input: serde_json::Value`
- `output: serde_json::Value`
- `artifact: serde_json::Value`

File wrapper:

- `StoredProtocolArtifactFile { path, stored }`

Schema constant:

- `PROTOCOL_ARTIFACT_SCHEMA_VERSION = "protocol-artifact.v1"`

Reader/writer and utility functions:

- `write_protocol_artifact`
- `list_protocol_artifacts`
- `load_protocol_artifact`
- `validate_protocol_artifact_identity`
- `protocol_artifact_summary`
- `protocol_artifact_preview`

Persisted path convention:

```text
<run_dir>/protocol-artifacts/<created_at_ms>_<procedure_name>_<subject_id>.json
```

`procedure_name` and `subject_id` are sanitized by `sanitize_component`.

Identity resolution:

- `write_protocol_artifact` calls `resolve_protocol_run_identity(record_path)`
  and refuses to write if the caller's `subject_id` does not match.
- `list_protocol_artifacts` resolves identity, reads all `.json` files under
  `protocol_artifacts_dir_for_run`, loads each file, validates identity, and
  sorts by descending path.
- `validate_protocol_artifact_identity` checks `stored.run_id`,
  `stored.subject_id`, and for selected protocol outputs checks
  `output.subject_id` through `protocol_output_subject_id`.
- `resolve_protocol_run_identity` prefers `RunRegistration` from the
  `record_path`; if no registration exists, it falls back to
  `read_compressed_record(record_path)` and uses
  `record.metadata.benchmark.instance_id` as `subject_id`.
- `write_protocol_artifact` calls `sync_protocol_registration_status` after
  writing.

Known procedure names observed in the storage/reporting path:

- `tool_call_intent_segmentation`
- `tool_call_review`
- `tool_call_segment_review`
- `intervention_issue_detection`
- `intervention_synthesis`
- `intervention_apply`

Child-selection fit: `StoredProtocolArtifact` is the best durable envelope for
process-quality evidence. It preserves input, output, model/provider, run id,
subject id, procedure, and creation time. A child selection evidence record can
cite exact artifact paths and then load typed outputs through existing aggregate
readers.

Loss: payload typing is erased to `serde_json::Value` at the storage boundary.
The file has no digest, no direct `RunRegistration` ref, and no explicit
procedure schema id beyond `procedure_name` plus the coarse artifact schema
version. `protocol_artifact_summary` and `protocol_artifact_preview` are
display-only lossy summaries.

## Protocol Aggregate Access Pattern

### `ProtocolArtifactRef`

Defined in `protocol_aggregate.rs`.

Fields:

- `path`
- `created_at_ms`
- `procedure_name`

Child-selection fit: sufficient as a small locator to re-open the stored
artifact with `load_protocol_artifact`.

Loss: it drops `run_id`, `subject_id`, `schema_version`, `model_id`,
`provider_slug`, and all digests. It is a locator, not provenance by itself.

### `ProtocolAggregate`

Defined in `protocol_aggregate.rs`.

Top-level fields:

- `run: ProtocolRunIdentity`
- `coverage: ProtocolCoverage`
- `segmentation: ProtocolSegmentationAnchor`
- `call_reviews: Vec<ProtocolCallReviewRow>`
- `segment_reviews: Vec<ProtocolSegmentReviewRow>`
- `crosswalk: Vec<ProtocolCrosswalkRow>`
- `derived_metrics: ProtocolDerivedMetrics`
- `skipped_segment_reviews: Vec<ProtocolSkippedSegmentReview>`

Primary loader:

- `load_protocol_aggregate(record_path)`
- `load_protocol_aggregate_from_artifacts(record_path, artifacts)`

Internal normalization/access functions:

- `normalize_anchor`
- `normalize_call_review`
- `normalize_segment_review`
- `describe_segment_mismatch`
- `artifact_ref`
- `from_artifact_output`
- `latest_artifact`
- `insert_latest_call_review`
- `insert_latest_segment_review`
- `is_newer_artifact`
- `build_call_segment_lookup`
- `count_artifacts`

Aggregate semantics:

- The latest `tool_call_intent_segmentation` artifact is the anchor.
- `tool_call_review` artifacts are normalized into one accepted row per
  `focal_call_index`; newer artifacts replace older ones.
- `tool_call_segment_review` artifacts are normalized into one accepted row per
  `segment_index`; newer artifacts replace older ones.
- Segment reviews whose packet no longer matches the selected anchor basis are
  moved to `skipped_segment_reviews`.
- Malformed review artifacts are skipped for some errors; identity mismatches
  are hard errors.
- `coverage.artifact_counts` preserves procedure counts, allowing duplicate
  detection after rows are collapsed.

Child-selection fit: this is the most useful typed projection for selection
features such as review coverage, missing call reviews, missing segment
reviews, anchor mismatch count, verdict counts, and call-to-segment crosswalk.
It can plug into child selection as a `ProcessQuality` or `ToolUseQuality`
evidence domain, provided the selected child decision cites the aggregate input
artifact refs.

Loss: the aggregate intentionally collapses duplicates to the latest accepted
review per call or segment. It also normalizes typed protocol outputs into
smaller rows and drops full `input`, full `artifact`, model/provider, and many
rationales. `ProtocolCoverage` counts duplicates but does not retain the losing
artifact refs except for skipped segment reviews.

## Issue Aggregate Access Pattern

### `IssueArtifactRef`

Defined in `intervention_issue_aggregate.rs`.

Fields:

- `path`
- `created_at_ms`
- `procedure_name`

Same loss profile as `ProtocolArtifactRef`: path locator only, no subject/run
fields, model/provider, schema, or digest.

### `IssueDetectionAggregate`

Defined in `intervention_issue_aggregate.rs`.

Fields:

- `run: IssueRunIdentity`
- `artifact: IssueArtifactRef`
- `input: IssueDetectionArtifactInput`
- `output: IssueDetectionOutput`
- optional `primary_issue: IssueCase`

Loader:

- `load_issue_detection_aggregate(record_path)`

Semantics:

- Resolves run identity through `resolve_protocol_run_identity`.
- Lists protocol artifacts through `list_protocol_artifacts`.
- Filters to `INTERVENTION_ISSUE_DETECTION_PROCEDURE`, currently
  `intervention_issue_detection`.
- Sorts by reverse `created_at_ms`.
- Returns the first latest artifact whose input and output deserialize.
- Computes `primary_issue` through `select_primary_issue`.

Child-selection fit: useful when selecting or explaining children by known
failure family, target tool, or intervention issue. It keeps typed input and
output, unlike `ProtocolAggregateReport`.

Loss: like the protocol aggregate, it collapses attempts to a single latest
deserializable artifact. Earlier issue detection artifacts, model/provider
choice, and schema data are available only by reopening raw
`StoredProtocolArtifact` files.

## Report And Triage Projections

### `ProtocolAggregateReport`

Defined in `protocol_report.rs`; built in `cli.rs` by `build_protocol_report`.

Report fields include:

- `run_id`
- `subject_id`
- optional `title`, `generated_at`, `scope`
- `provenance: Vec<String>`
- `coverage: ProtocolAggregateCoverage`
- `segments: Vec<ProtocolAggregateSegmentRow>`
- `call_issues: Vec<ProtocolAggregateCallIssueRow>`
- `notes`

Renderer:

- `render_protocol_aggregate_report_with_options`

Builder details:

- `build_protocol_report` reads `ProtocolAggregate`.
- It calls `load_anchor_call_details`, which reopens
  `aggregate.segmentation.artifact.path` with `load_protocol_artifact` and
  reads anchor `input.calls` to recover `tool_name` and `summary`.
- It computes `primary_call_issue`, `call_review_severity`, confidence
  fractions, duplicate counts, and segment evidence notes.
- It stores provenance as strings such as the anchor timestamp/path and a
  generic derivation note.

Child-selection fit: good for human-facing explanation and candidate ranking
features that need compact labels. It should not be the source object for
selection because it is intentionally presentation shaped.

Loss: source artifact refs are converted into strings in `provenance`; segment
rows do not retain artifact refs; call issue rows do not retain artifact refs;
tool detail is recovered from anchor input and summarized. Report filters in
`apply_protocol_report_filters` can remove rows from the report entirely.

### `ProtocolCampaignTriageReport`

Defined in `protocol_triage_report.rs`; built in `cli.rs` by
`collect_protocol_campaign_triage_report`.

Important fields:

- `summary: ProtocolCampaignSummary`
- `evidence: ProtocolCampaignEvidence`
- count rows for issue kinds, issue tools, nearby segment labels/statuses
- `problem_families: Vec<ProtocolCampaignFamilyRow>`
- `exemplars: Vec<ProtocolCampaignExemplarRow>`
- `next_steps`

Renderer:

- `render_protocol_campaign_triage_report`

Builder details:

- Reads closure/campaign state.
- For each completed instance, tries `row.artifacts.record_path`.
- If present, builds a per-run `ProtocolAggregateReport`; otherwise falls back
  to closure summary rows.
- Groups issue labels, tools, statuses, and exemplars across runs.

Child-selection fit: useful for campaign-level prioritization and choosing
which issue family to address next. It is not adequate as child-specific
evidence because it groups and truncates by filters, limits, and exemplar
selection.

Loss: aggregates many runs into counts, affected-run sets, family labels, and
exemplar ids. It does not retain the full per-run artifact ref set behind each
count. `next_steps` are rendered command hints, not evidence.

## Existing Typed Readers/Writers Summary

Strongest typed readers/writers:

| Surface | Writer | Reader/accessor |
| --- | --- | --- |
| Registration | `RunRegistration::persist`, `persist_registration` | `RunRegistration::load`, `discover`, `load_registration_for_record_path` |
| Run record | `write_compressed_record` | `read_compressed_record`, `RunRecord` accessors |
| Protocol artifact | `write_protocol_artifact` | `load_protocol_artifact`, `list_protocol_artifacts` |
| Protocol aggregate | derived from artifact list | `load_protocol_aggregate` |
| Issue aggregate | derived from artifact list | `load_issue_detection_aggregate` |
| Protocol report | derived from aggregate | `build_protocol_report`, renderers |
| Campaign triage | derived from closure rows and reports | `collect_protocol_campaign_triage_report`, renderer |

The storage layer has typed readers/writers for records and protocol envelopes.
The report layer is typed as Rust structs, but those structs are typed
projections rather than source evidence.

## How This Plugs Into Child Selection Evidence

A durable child-selection evidence bundle can be built without inventing new
path conventions:

1. Start from `RunRegistration` for each candidate child run.
2. Carry `run_id`, `frozen_spec.task_id`, role, lifecycle status, and
   `RunArtifactRefs`.
3. Load `RunRecord` from `artifacts.record_path` for operational metrics:
   timing, tool calls, failed calls, LLM/provider metadata, patch packaging, and
   turn-level DB snapshots.
4. Load protocol evidence from `artifacts.protocol_artifacts_dir` using
   `load_protocol_aggregate(record_path)` when available.
5. Load issue evidence with `load_issue_detection_aggregate(record_path)` when
   available.
6. Store selection inputs as structured facts plus source refs:
   `record_path`, protocol artifact refs, issue artifact ref, and registration
   path or run id.

Do not use rendered protocol reports as source facts. Reports can be attached as
operator-facing explanation, but selection should cite:

- `RunRegistration` or `RunArtifactRefs`
- `record.json.gz`
- `StoredProtocolArtifact` paths
- aggregate rows with `ProtocolArtifactRef`/`IssueArtifactRef`

## Lossy Projection Map

| Projection | What it keeps | What it loses |
| --- | --- | --- |
| `RunArtifactPaths` | paths returned by one runner invocation | canonical authority, digests, lifecycle, failed-run absence semantics |
| `RunArtifactRefs` | canonical path inventory | content digests, per-artifact creation/model/schema, History authority |
| `RunRecord::outcome_summary` | compact run stats | turn-level details, source sidecar paths, protocol evidence |
| `RunRecord::tool_calls()` fallback | compatibility access to tool calls | whether data came from direct field or event extraction |
| `ProtocolArtifactRef` | path, created time, procedure | run/subject/schema/model/provider/input/output/artifact/digest |
| `ProtocolAggregate` | normalized coverage, rows, crosswalk, selected latest reviews | duplicate attempt payloads, model/provider, raw input/artifact, many rationales |
| `IssueArtifactRef` | path, created time, procedure | run/subject/schema/model/provider/digest |
| `IssueDetectionAggregate` | latest typed issue input/output and primary issue | earlier issue attempts and full artifact envelope fields unless reloaded |
| `ProtocolAggregateReport` | compact human/report rows and issue labels | artifact refs per row, raw payloads, unfiltered row set after filters |
| `ProtocolCampaignTriageReport` | campaign counts, families, exemplars | per-run source refs behind counts, exact artifact provenance, truncated rows |
| `protocol_artifact_summary` / `preview` | short display strings | nearly all structured payload content |

## Provenance Notes For Future Design

- Treat `record_path` as a locator into a registered run, not as the only run
  identity. Prefer `resolve_protocol_run_identity` or `RunRegistration` when
  crossing from records into protocol artifacts.
- Treat `StoredProtocolArtifact` as the source envelope for protocol procedure
  outputs. `ProtocolAggregate` is a derived view.
- When child selection consumes protocol evidence, include both selected rows
  and the raw refs used to compute them. Otherwise future replay cannot tell
  whether a child was selected from current evidence, stale duplicate reviews,
  or a filtered report.
- The current ref types are intentionally small. They need either digest-backed
  wrappers or an enclosing evidence bundle if they become admission facts.
- Existing registration sync updates `protocol_anchor` to the latest protocol
  artifact by timestamp, while `ProtocolAggregate` uses the latest
  `tool_call_intent_segmentation` as the anchor. Those are not always the same
  object.
- The compatibility fallback in `resolve_protocol_run_identity` is useful for
  old records but should be marked as lower provenance than registration-backed
  identity.

## Bottom Line

The current run/protocol path is good enough to feed child selection as typed
evidence if selection starts from `RunRegistration`, reads `RunRecord`, and
cites raw `StoredProtocolArtifact` paths behind any aggregate/report features.

The unsafe shortcut is selecting from `ProtocolAggregateReport` or campaign
triage rows alone. Those are projections. They explain evidence; they do not
preserve enough provenance to be the evidence.
