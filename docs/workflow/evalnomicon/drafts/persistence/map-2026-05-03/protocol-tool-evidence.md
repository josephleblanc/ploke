# Worker 3: Protocol And Tool Evidence Persistence Map

Scope: `ploke-eval` CLI introspection surfaces for tool-call records/events, protocol artifacts, reviews, issue/synthesis artifacts, aggregate/status projections, and the `inspect`/`protocol` commands that expose or produce them.

## Storage Classes

- Run record: `~/.ploke-eval/instances/<instance>/runs/run-*/record.json.gz`. This is the durable `RunRecord` written by `write_compressed_record` after the runner populates setup, turn, packaging, timing, and embedded turn evidence (`crates/ploke-eval/src/runner.rs:2526`, `crates/ploke-eval/src/record.rs:1579`).
- Run sidecars: same run dir, including `agent-turn-trace.json`, `agent-turn-summary.json`, and `llm-full-responses.jsonl`. These are durable sidecars used by inspectors, but the full-response sidecar is explicitly noted as potentially undercounting usage (`crates/ploke-eval/src/runner.rs:2098`, `crates/ploke-eval/src/runner.rs:2379`, `crates/ploke-eval/src/runner.rs:2386`, `crates/ploke-eval/src/cli.rs:9942`).
- Protocol artifact dir: `~/.ploke-eval/instances/<instance>/runs/run-*/protocol-artifacts/*.json`, from `protocol_artifacts_dir_for_run(run_dir) = run_dir.join("protocol-artifacts")` (`crates/ploke-eval/src/layout.rs:34`). Files are named `<created_at_ms>_<procedure_name>_<subject_id>.json` (`crates/ploke-eval/src/protocol_artifacts.rs:268`).
- Campaign/status projections: `~/.ploke-eval/campaigns/<campaign>/closure-state.json` and run registrations under the registry roots. These preserve reduced status and anchors, not raw review evidence (`crates/ploke-eval/src/closure.rs:52`, `crates/ploke-eval/src/closure.rs:127`, `crates/ploke-eval/src/inner/registry.rs:379`).

## Tool Calls And Turn Evidence

Item: turn-local tool events and reconstructed tool calls.

- Path pattern: `record.json.gz` under the run dir; duplicated as sidecars in `agent-turn-trace.json` and `agent-turn-summary.json`.
- Rust schema: `RunRecord.phases.agent_turns: Vec<TurnRecord>`; `TurnRecord.tool_calls: Vec<ToolExecutionRecord>`; `TurnRecord.agent_turn_artifact: Option<AgentTurnArtifact>` (`crates/ploke-eval/src/record.rs:806`, `crates/ploke-eval/src/record.rs:831`, `crates/ploke-eval/src/record.rs:838`). Event types are `ObservedTurnEvent::{ToolRequested, ToolCompleted, ToolFailed, LlmResponse, TurnFinished, ...}` (`crates/ploke-eval/src/runner.rs:785`). Tool request/result schemas are `ToolRequestRecord`, `ToolCompletedRecord`, `ToolFailedRecord` with `request_id`, `parent_id`, `call_id`, `tool`, arguments/content/error, and `latency_ms` (`crates/ploke-eval/src/runner.rs:730`).
- Writer: `run_benchmark_turn` initializes and continuously rewrites `agent-turn-trace.json` (`crates/ploke-eval/src/runner.rs:3252`, `crates/ploke-eval/src/runner.rs:3278`, `crates/ploke-eval/src/runner.rs:3322`, `crates/ploke-eval/src/runner.rs:3340`); final summary sidecar is written at `crates/ploke-eval/src/runner.rs:2379`. `handle_benchmark_event` records tool request/completion/failure events (`crates/ploke-eval/src/runner.rs:3513`, `crates/ploke-eval/src/runner.rs:3530`, `crates/ploke-eval/src/runner.rs:3564`). `RunRecord::add_turn_from_artifact` embeds the artifact and extracted tool calls (`crates/ploke-eval/src/record.rs:1535`).
- Reader/CLI: `RunRecord::tool_calls()` and `TurnRecord::tool_calls()` prefer explicit `tool_calls`, then reconstruct from embedded events (`crates/ploke-eval/src/record.rs:345`, `crates/ploke-eval/src/record.rs:938`). `inspect tool-calls`, `inspect turn --show tool-calls`, `inspect turn --show loop`, `inspect conversations`, and protocol subject builders read these projections (`crates/ploke-eval/src/cli.rs:6506`, `crates/ploke-eval/src/cli.rs:6931`, `crates/ploke-eval/src/cli.rs:9414`, `crates/ploke-eval/src/cli.rs:9429`).
- Join IDs: `call_id` joins request to completion/failure (`crates/ploke-eval/src/record.rs:517`); list index joins protocol review subjects to run calls (`crates/ploke-eval/src/cli.rs:9414`, `crates/ploke-eval/src/cli.rs:9584`); `turn_number` and `assistant_message_id` join turn rows to full-response sidecar rows (`crates/ploke-eval/src/cli.rs:9861`, `crates/ploke-eval/src/cli.rs:9907`).
- Authority: run record and trace/summary sidecars are evidence captured from the runner event stream. Reconstructed `ToolExecutionRecord` rows are a typed projection over `ToolRequested` plus terminal tool events (`crates/ploke-eval/src/record.rs:509`).
- Safe inspection:

```sh
zcat "$RUN/record.json.gz" | jq '{run:.manifest_id, subject:.metadata.benchmark.instance_id, tool_calls:[.phases.agent_turns[]?.tool_calls[]? | {call_id:.request.call_id, tool:.request.tool, status:.result.status, latency_ms}] | .[:20]}'
jq '{task_id, event_kinds:[.events[] | keys[0]] | group_by(.) | map({kind:.[0], count:length}), terminal_record}' "$RUN/agent-turn-trace.json"
```

## Raw Full Responses Sidecar

Item: normalized provider response envelopes.

- Path pattern: `llm-full-responses.jsonl` under the run dir.
- Rust schema: JSONL of `RawFullResponseRecord { assistant_message_id, response_index, response: OpenAiResponse }` (`crates/ploke-eval/src/record.rs:1222`).
- Writer: runner records the current full-response log offset, then slices the tracing log into the run sidecar (`crates/ploke-eval/src/runner.rs:2102`, `crates/ploke-eval/src/runner.rs:2381`, `crates/ploke-eval/src/runner.rs:3739`, `crates/ploke-eval/src/runner.rs:3799`). The tracing target is configured in `tracing_setup` (`crates/ploke-eval/src/tracing_setup.rs:67`, `crates/ploke-eval/src/tracing_setup.rs:95`).
- Reader/CLI: `inspect turn --show responses`; `inspect conversations` may use sidecar usage totals when run summary usage is zero (`crates/ploke-eval/src/cli.rs:6897`, `crates/ploke-eval/src/cli.rs:9888`, `crates/ploke-eval/src/cli.rs:9960`).
- Join IDs: `assistant_message_id` from `TurnFinishedRecord`; `response_index` orders multiple responses within that assistant chain.
- Authority: authoritative raw provider envelope sidecar for captured responses, but a known incomplete usage source for some stop responses.
- Safe inspection:

```sh
head -n 5 "$RUN/llm-full-responses.jsonl" | jq -c '{assistant_message_id,response_index,response_id:.response.id,finish:(.response.choices[0].finish_reason),usage:.response.usage}'
```

## Shared Protocol Artifact Envelope

Item: persisted protocol procedure evidence.

- Path pattern: `$RUN/protocol-artifacts/<created_at_ms>_<procedure>_<subject_id>.json`.
- Rust schema: `StoredProtocolArtifact { schema_version, procedure_name, subject_id, run_id, created_at_ms, model_id, provider_slug, input, output, artifact }`; schema version is `protocol-artifact.v1` (`crates/ploke-eval/src/protocol_artifacts.rs:14`, `crates/ploke-eval/src/protocol_artifacts.rs:16`).
- Writer: `write_protocol_artifact` resolves run identity, validates subject, writes the envelope, then syncs registration protocol status (`crates/ploke-eval/src/protocol_artifacts.rs:235`, `crates/ploke-eval/src/protocol_artifacts.rs:250`, `crates/ploke-eval/src/protocol_artifacts.rs:291`, `crates/ploke-eval/src/protocol_artifacts.rs:295`).
- Reader/CLI: `list_protocol_artifacts` loads all JSON files and validates run/subject identity (`crates/ploke-eval/src/protocol_artifacts.rs:299`, `crates/ploke-eval/src/protocol_artifacts.rs:323`, `crates/ploke-eval/src/protocol_artifacts.rs:353`). `inspect protocol-artifacts [INDEX] [--full]` lists or prints bounded/full artifact detail (`crates/ploke-eval/src/cli.rs:7245`, `crates/ploke-eval/src/cli.rs:9680`).
- Join IDs: envelope `run_id`, `subject_id`, `procedure_name`, `created_at_ms`; procedure-specific IDs inside `output.packet` or `output.sequence`.
- Authority: authoritative persisted evidence for protocol procedure runs. CLI summaries/previews are projections only.
- Safe inspection:

```sh
rg --files "$RUN/protocol-artifacts" | sort | tail -n 20
jq '{schema_version,procedure_name,subject_id,run_id,created_at_ms,model_id,provider_slug,input_keys:(.input|keys? // []),output_keys:(.output|keys? // []),artifact_keys:(.artifact|keys? // [])}' "$ARTIFACT"
```

## Intent Segmentation Artifact

Item: `tool_call_intent_segmentation`.

- Path pattern: `$RUN/protocol-artifacts/*_tool_call_intent_segmentation_<subject_id>.json`.
- Rust schema: input `ToolCallSequence { subject_id, total_turns, total_calls_in_run, turns, calls }` (`crates/ploke-protocol/src/tool_calls/trace.rs:107`); output `SegmentedToolCallSequence { sequence, signals, segments, coverage, uncovered_spans, uncovered_call_indices, overall_rationale }` (`crates/ploke-protocol/src/tool_calls/segment.rs:107`). Procedure output name is `tool_call_intent_segmentation` (`crates/ploke-protocol/src/tool_calls/segment.rs:127`, `crates/ploke-protocol/src/tool_calls/segment.rs:463`).
- Writer: direct command `protocol tool-call-intent-segments` writes at `crates/ploke-eval/src/cli.rs:6303`; quiet path used by `protocol run`/closure advancement writes at `crates/ploke-eval/src/cli.rs:5653`.
- Reader/CLI: `inspect protocol-artifacts`; `inspect protocol-overview`; `protocol status`; `protocol run`; `load_latest_segmented_sequence` for segment review (`crates/ploke-eval/src/cli.rs:5814`, `crates/ploke-eval/src/cli.rs:6274`, `crates/ploke-eval/src/cli.rs:7425`).
- Join IDs: `sequence.subject_id`; call `index`; segment `segment_index`; contiguous `start_index..end_index`; `turns`.
- Authority: authoritative segmentation anchor. `ProtocolAggregate` selects the latest segmentation artifact as anchor (`crates/ploke-eval/src/protocol_aggregate.rs:270`) and treats segment reviews against older/different anchors as skipped mismatches.
- Safe inspection:

```sh
jq 'select(.procedure_name=="tool_call_intent_segmentation") | {path:input_filename,created_at_ms,subject_id,run_id,coverage:.output.coverage,segments:[.output.segments[] | {segment_index,start_index,end_index,status,label,turns,calls:[.calls[].index]}]}' "$ARTIFACT"
```

## Tool Call Review Artifacts

Item: `tool_call_review`.

- Path pattern: `$RUN/protocol-artifacts/*_tool_call_review_<subject_id>.json`.
- Rust schema: input `ToolCallNeighborhood { subject_id, total_calls_in_run, total_calls_in_turn, turn, before, focal, after }` (`crates/ploke-protocol/src/tool_calls/trace.rs:87`); output `LocalAnalysisAssessment { packet, signals, usefulness, redundancy, recoverability, overall, overall_confidence, synthesis_rationale }` (`crates/ploke-protocol/src/tool_calls/review.rs:146`). Procedure artifact type is `ToolCallReviewArtifact` (`crates/ploke-protocol/src/tool_calls/review.rs:508`).
- Writer: direct command writes at `crates/ploke-eval/src/cli.rs:4327`; quiet and batch/closure paths write through `write_call_review` (`crates/ploke-eval/src/cli.rs:5689`, `crates/ploke-eval/src/cli.rs:5800`, `crates/ploke-eval/src/cli.rs:5307`).
- Reader/CLI: `inspect protocol-artifacts`; `inspect protocol-overview`; `protocol status`/`protocol run`; issue detection reads through `ProtocolAggregate` (`crates/ploke-eval/src/protocol_aggregate.rs:283`, `crates/ploke-eval/src/intervention/issue.rs:90`).
- Join IDs: focal call `index` in input; `output.packet.focal_call_index`; `output.packet.target_id = "call:<index>"`; call indices in `output.packet.calls`. Aggregate keeps only the latest accepted review per focal call index (`crates/ploke-eval/src/protocol_aggregate.rs:621`).
- Authority: authoritative review evidence from bounded local protocol adjudication. Aggregate rows are normalized projections (`ProtocolCallReviewRow`) over accepted artifacts (`crates/ploke-eval/src/protocol_aggregate.rs:125`, `crates/ploke-eval/src/protocol_aggregate.rs:448`).
- Safe inspection:

```sh
jq 'select(.procedure_name=="tool_call_review") | {created_at_ms,subject_id,run_id,focal:.output.packet.focal_call_index,target:.output.packet.target_id,overall:.output.overall,confidence:.output.overall_confidence,calls:[.output.packet.calls[].index],concerns:.output.signals.candidate_concerns}' "$ARTIFACT"
```

## Segment Review Artifacts

Item: `tool_call_segment_review`.

- Path pattern: `$RUN/protocol-artifacts/*_tool_call_segment_review_<subject_id>.json`.
- Rust schema: input `SegmentReviewSubject { subject_id, sequence, segment, coverage }` (`crates/ploke-protocol/src/tool_calls/review.rs:183`); output again `LocalAnalysisAssessment`; artifact type `ToolCallSegmentReviewArtifact` (`crates/ploke-protocol/src/tool_calls/review.rs:621`).
- Writer: direct command writes at `crates/ploke-eval/src/cli.rs:6169`; quiet path writes at `crates/ploke-eval/src/cli.rs:5864`.
- Reader/CLI: `inspect protocol-artifacts`; `inspect protocol-overview`; `ProtocolAggregate` normalizes accepted reviews and records anchor mismatches as skipped reviews (`crates/ploke-eval/src/protocol_aggregate.rs:295`, `crates/ploke-eval/src/protocol_aggregate.rs:492`, `crates/ploke-eval/src/protocol_aggregate.rs:546`).
- Join IDs: `output.packet.segment_index`; `target_id = "segment:<index>"`; `segment_label`, `segment_status`, `turn_span`, and `total_calls_in_scope` must match the current segmentation anchor basis (`crates/ploke-eval/src/protocol_aggregate.rs:512`).
- Authority: authoritative review evidence, but only authoritative relative to its segmentation basis. `ProtocolAggregate` excludes mismatched segment reviews from usable evidence.
- Safe inspection:

```sh
jq 'select(.procedure_name=="tool_call_segment_review") | {created_at_ms,segment:.output.packet.segment_index,target:.output.packet.target_id,label:.output.packet.segment_label,status:.output.packet.segment_status,turn_span:.output.packet.turn_span,overall:.output.overall,confidence:.output.overall_confidence,calls:[.output.packet.calls[].index]}' "$ARTIFACT"
```

## Issue Detection Artifact

Item: `intervention_issue_detection`.

- Path pattern: `$RUN/protocol-artifacts/*_intervention_issue_detection_<subject_id>.json`.
- Rust schema: input persisted as `IssueDetectionArtifactInput { run_id, subject_id, total_calls_in_run, anchor_segment_count, protocol_reviewed_call_count, protocol_reviewed_segment_count, protocol_artifact_count }`; output `IssueDetectionOutput { cases: Vec<IssueCase> }` (`crates/ploke-eval/src/intervention/issue.rs:43`, `crates/ploke-eval/src/intervention/issue.rs:50`, `crates/ploke-eval/src/intervention/issue.rs:68`).
- Writer: `protocol issue-detection` writes at `crates/ploke-eval/src/cli.rs:4574`; helper `persist_issue_detection_for_record` writes at `crates/ploke-eval/src/cli.rs:1347`.
- Reader/CLI: `inspect issue-overview` loads latest valid artifact via `load_issue_detection_aggregate` (`crates/ploke-eval/src/intervention_issue_aggregate.rs:51`, `crates/ploke-eval/src/cli.rs:7361`); protocol artifact listing summarizes case counts (`crates/ploke-eval/src/protocol_artifacts.rs:150`).
- Join IDs: `run_id`, `subject_id`; issue evidence stores `reviewed_call_indices`, `reviewed_segment_indices`, `candidate_concerns`, and tool target (`crates/ploke-eval/src/intervention/issue.rs:24`, `crates/ploke-eval/src/intervention/issue.rs:167`).
- Authority: derived evidence/synthesis over run record plus protocol aggregate, not a new primary observation. It is persisted as a protocol artifact and can be used as an intervention target selection input.
- Safe inspection:

```sh
jq 'select(.procedure_name=="intervention_issue_detection") | {created_at_ms,input:.input,cases:[.output.cases[] | {target_tool,selection_basis,evidence:.evidence}]}' "$ARTIFACT"
```

## Intervention Synthesis Artifact

Item: `intervention_synthesis`.

- Path pattern: `$RUN/protocol-artifacts/*_intervention_synthesis_<subject_id>.json`.
- Rust schema: input `InterventionSynthesisInput { issue, source_state_id, source_content, operation_target }`; output `InterventionSynthesisOutput { candidate_set }` with `InterventionCandidateSet { source_state_id, target_relpath, source_content, candidates, operation_target }` (`crates/ploke-eval/src/intervention/spec.rs:124`, `crates/ploke-eval/src/intervention/spec.rs:143`, `crates/ploke-eval/src/intervention/spec.rs:153`). Procedure artifact is a protocol `ProcedureArtifact` for the synthesis sequence (`crates/ploke-eval/src/intervention/synthesize.rs:272`).
- Writer: helper `persist_intervention_synthesis_for_record` writes at `crates/ploke-eval/src/cli.rs:1403`; the LLM procedure records branch labels and candidate-generating subrequests (`crates/ploke-eval/src/intervention/synthesize.rs:301`).
- Reader/CLI: protocol artifact listing summarizes `candidate_set.candidates` and `target_relpath` (`crates/ploke-eval/src/protocol_artifacts.rs:177`). I did not find a public `protocol` or `inspect` subcommand dedicated to synthesis output beyond `inspect protocol-artifacts`.
- Join IDs: `source_state_id`; issue evidence joins back to reviewed call/segment indices; `candidate_id`, optional `patch_id`, `target_relpath`, optional `operation_target`/artifact id.
- Authority: derived intervention proposal evidence. It is not direct run evidence; it is a persisted candidate set derived from issue detection and source content.
- Safe inspection:

```sh
jq 'select(.procedure_name=="intervention_synthesis") | {created_at_ms,target:.output.candidate_set.target_relpath,source_state_id:.output.candidate_set.source_state_id,candidates:[.output.candidate_set.candidates[] | {candidate_id,branch_label,spec_kind:.spec.kind}]}' "$ARTIFACT"
```

## Aggregate And Status Projections

Item: `ProtocolAggregate` and human-facing reports.

- Path pattern: no aggregate file found. Built on demand from `record.json.gz` plus `$RUN/protocol-artifacts/*.json`.
- Rust schema: `ProtocolAggregate { run, coverage, segmentation, call_reviews, segment_reviews, crosswalk, derived_metrics, skipped_segment_reviews }` (`crates/ploke-eval/src/protocol_aggregate.rs:204`). Report schema is `ProtocolAggregateReport` (`crates/ploke-eval/src/protocol_report.rs:11`).
- Builder/reader: `load_protocol_aggregate` calls `list_protocol_artifacts`, validates identity, selects latest segmentation anchor, normalizes latest call/segment reviews, computes missing indices and derived metrics (`crates/ploke-eval/src/protocol_aggregate.rs:252`, `crates/ploke-eval/src/protocol_aggregate.rs:269`, `crates/ploke-eval/src/protocol_aggregate.rs:336`, `crates/ploke-eval/src/protocol_aggregate.rs:353`). `build_protocol_report` adds issue rows and report provenance (`crates/ploke-eval/src/cli.rs:8610`).
- CLI: `inspect protocol-overview` for one run, all visible runs, or campaign triage (`crates/ploke-eval/src/cli.rs:7385`); `protocol status` and `protocol run` use `ProtocolRunState` (`crates/ploke-eval/src/cli.rs:4435`, `crates/ploke-eval/src/cli.rs:4466`, `crates/ploke-eval/src/cli.rs:5381`).
- Join IDs: anchor segment indices; focal call indices; artifact refs `{path, created_at_ms, procedure_name}`; `run_id`, `subject_id`.
- Authority: projection/cache-like in memory only. It does not introduce source facts; it normalizes persisted artifacts and classifies coverage.
- Safe inspection:

```sh
ploke-eval inspect protocol-overview --record "$RUN/record.json.gz" --format json | jq '{run_id,subject_id,coverage,segments:(.segments|length),call_issues:(.call_issues|length),notes}'
ploke-eval protocol status --record "$RUN/record.json.gz" --format json | jq '{instance_id,tool_calls_total,artifact_count,segmentation_present,aggregate_available,missing_call_indices,missing_segment_indices,next_step,aggregate_error}'
```

Item: closure/campaign protocol status.

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/closure-state.json`.
- Rust schema: `ClosureState.protocol: ProtocolClosureSummary`; each `ClosureInstanceRow` stores `protocol_status`, `protocol_procedures`, optional `protocol_counts`, and artifact refs including `protocol_artifacts_dir` and `protocol_anchor` (`crates/ploke-eval/src/closure.rs:52`, `crates/ploke-eval/src/closure.rs:127`, `crates/ploke-eval/src/closure.rs:155`, `crates/ploke-eval/src/closure.rs:174`, `crates/ploke-eval/src/closure.rs:202`).
- Writer/reader: `closure recompute` writes recomputed state; `closure status` reads it (`crates/ploke-eval/src/cli.rs:4259`, `crates/ploke-eval/src/cli.rs:4280`). `assess_protocol_state` scans protocol artifacts and aggregates where possible (`crates/ploke-eval/src/closure.rs:898`).
- CLI: `closure status`, `closure recompute`, `inspect protocol-overview --campaign`, `closure advance protocol` (`crates/ploke-eval/src/cli.rs:2765`, `crates/ploke-eval/src/cli.rs:7609`, `crates/ploke-eval/src/protocol_triage_report.rs:6`).
- Join IDs: campaign instance id to run registration/record path; protocol anchor path; procedure names in required-procedure spelling (`tool-call-*`) mapped to stored procedure names (`tool_call_*`) in closure assessment.
- Authority: reduced status projection. Useful for selection and progress, but raw review authority remains in run records and protocol artifact files.
- Safe inspection:

```sh
jq '{campaign_id,protocol,instances:[.instances[] | {instance_id,eval_status,protocol_status,protocol_counts,anchor:.artifacts.protocol_anchor}] | .[:20]}' "$PLOKE_EVAL_HOME/campaigns/$CAMPAIGN/closure-state.json"
ploke-eval inspect protocol-overview --campaign "$CAMPAIGN" --format json | jq '{campaign_id,summary,evidence,problem_families:(.problem_families|length),exemplars:(.exemplars|length)}'
```

## Command Surface Summary

- Producers under `protocol`: `protocol tool-call-intent-segments`, `protocol tool-call-review <INDEX>`, `protocol tool-call-segment-review <SEGMENT>`, `protocol issue-detection`, and `protocol run` (`crates/ploke-eval/src/cli.rs:2890`, `crates/ploke-eval/src/cli.rs:4466`).
- Readers under `protocol`: `protocol status` reads run/protocol state and prints next step (`crates/ploke-eval/src/cli.rs:4435`).
- Readers under `inspect`: `inspect tool-calls`, `inspect turn`, `inspect protocol-artifacts`, `inspect protocol-overview`, `inspect issue-overview`, `inspect tool-overview`, and campaign triage via `inspect protocol-overview --campaign` (`crates/ploke-eval/src/cli.rs:3042`, `crates/ploke-eval/src/cli.rs:3298`, `crates/ploke-eval/src/cli.rs:3390`, `crates/ploke-eval/src/cli.rs:3417`).
- Campaign producers/readers: `closure advance protocol` creates missing segmentation/call/segment review artifacts; `closure status` and `closure recompute` maintain reduced campaign status (`crates/ploke-eval/src/cli.rs:5260`, `crates/ploke-eval/src/cli.rs:6047`).

## Gaps And Unknowns

- I did not find a persisted `ProtocolAggregate` file; aggregates and reports appear to be recomputed projections from protocol artifacts and the run record.
- I did not find a dedicated `protocol` or `inspect` command for `intervention_synthesis`; only protocol artifact listing/inspection exposes it generically.
- `persist_intervention_synthesis_for_record`, `persist_intervention_apply_for_record`, and `persist_issue_detection_for_record` exist as helpers, but only `protocol issue-detection` is visibly wired as a public protocol command in this pass.
- Segment review authority depends on the latest segmentation anchor. Older segment reviews can remain persisted but be excluded as mismatched projections.
- Tool-call list indices are derived from current `RunRecord::tool_calls()` ordering. They are stable for a persisted record but are not a provider-native ID; joins to provider/tool events should use `call_id` where available.
- Full-response sidecar usage totals may undercount final stop responses, per code comments in the inspector.
