# Agent 08: Protocol Evaluation Data Census

Date: 2026-05-06

Scope:
- Surveyed: `crates/ploke-protocol/**` and `crates/ploke-eval/src/protocol/**`.
- Not surveyed: `crates/ploke-eval/src/protocol_aggregate.rs`, even though `crates/ploke-eval/src/protocol/mod.rs` re-exports it through `#[path = "../protocol_aggregate.rs"]`.
- Source code was not modified.

## Executive Finding

The scoped implementation already has a useful typed algebra for protocol evaluation data: typed steps, serializable artifacts, explicit evidence policies, executor provenance, LLM adjudication provenance, procedure composition artifacts, procedure timing/debug events, tool-call sequence packets, intent segmentation, and local tool-call/segment review assessments.

Within this scope, however, there is no concrete artifact path, storage backend, report writer, archive join, or child/successor selection procedure. The crate-level docs say protocol artifacts are persisted, but the scoped code only defines serializable in-memory artifacts and procedure return values. Any durable storage path or CLI report projection must live outside the surveyed scope.

Selection implication: the best existing successor-selection inputs are `SegmentedToolCallSequence` and `LocalAnalysisAssessment`, wrapped in `StepArtifact`/`ProcedureArtifact` with `JsonLlmProvenance` or `MechanizedProvenance`. They can support child comparison if an external runner stores them per child and joins them to child identity, objective outcome, generation, and artifact/root path.

## Live Data Algebra

### Core protocol carriers

Path: `crates/ploke-protocol/src/core.rs`

- `Measurement` (`core.rs:5`): names a metric with `Subject` and `Value` associated types. It is a type-level metric label, not a numeric scoring implementation.
- `ProcedureState` (`core.rs:13`): marker bound for serializable, cloneable procedure states.
- `ExecutorKind` (`core.rs:19`): `Mechanized`, `LlmAdjudicator`, `HumanReviewer`.
- `Confidence` (`core.rs:27`): `Low`, `Medium`, `High`.
- `EvidencePolicy` (`core.rs:35`): `allowed`, `forbidden`, `hindsight_allowed`, `external_context_allowed`.
- `StateDisposition` (`core.rs:49`): `RecordOnly`, `ForwardOnly`, `RecordAndForward`, `Ephemeral`; has `should_record` and `should_forward`.
- `StateEnvelope<State>` (`core.rs:67`): state plus disposition.
- `StepArtifact<InputState, OutputState, Provenance>` (`core.rs:73`): carries `step_id`, `step_name`, `executor_kind`, `executor_label`, `evidence_policy`, input/output states, input/output dispositions, and provenance.
- `SequenceArtifact`, `FanOutArtifact`, `MergeArtifact`, `ProcedureArtifact`, `ProcedureRun` (`core.rs:87`, `core.rs:100`, `core.rs:109`, `core.rs:115`, `core.rs:121`): structural artifacts for procedure composition and returned runs.

Selection contribution:
- `EvidencePolicy` can tell a selector whether a judgment was supposed to avoid hindsight or external context.
- `StepArtifact` is the basic replay/audit unit for comparing children, because it joins input, output, executor identity, evidence policy, and provenance.
- `ProcedureArtifact` and composition artifacts preserve causal position. This matters for successor selection because a final assessment without the branch artifacts would lose which LLM branch supplied which evidence.

Limitation:
- `EvidencePolicy` is recorded and used in prompts, but no scoped enforcement layer verifies that an LLM or caller actually complied.

### Step execution and provenance

Path: `crates/ploke-protocol/src/step.rs`

- `StepSpec` (`step.rs:6`): typed input/output states, `step_id`, `step_name`, `evidence_policy`, `disposition`.
- `StepExecution<OutputState, Provenance>` (`step.rs:23`): transient executor result with state, provenance, disposition.
- `StepExecutor<Spec>` (`step.rs:30`): executor interface with associated `Provenance` and `Error`; exposes `kind`, `label`, and `execute`.
- `MechanizedSpec` (`step.rs:48`): native Rust step interface.
- `MechanizedProvenance` (`step.rs:56`): currently only `strategy: String`.
- `MechanizedExecutor` (`step.rs:61`): records `ExecutorKind::Mechanized`, label `native_rust`, and `MechanizedProvenance { strategy: "native_rust" }`.
- `Step<Spec, Exec>::run` (`step.rs:115`): clones input, executes, and returns `StepArtifact`.

Selection contribution:
- `MechanizedProvenance` is low-cardinality but useful for confirming deterministic native derivations versus LLM judgments.
- `Step<Spec, Exec>::run` gives each selection datum its producing step and executor, allowing later selectors to compare only like-with-like.

### Procedure composition and debug events

Path: `crates/ploke-protocol/src/procedure.rs`

- `Procedure` (`procedure.rs:201`): typed `Subject`, `Output`, `Artifact`, `Error`, `name`, and async `run`.
- `Sequence`, `FanOut`, `Merge`, `NamedProcedure` (`procedure.rs:283`, `procedure.rs:350`, `procedure.rs:430`, `procedure.rs:496`): composition carriers that preserve intermediate artifacts.
- `ProcedureExt` (`procedure.rs:542`): helper methods `then`, `fan_out`, `fan_out_named`, `merge`, `named`.
- `ObservedSubrequest<Inner>` (`procedure.rs:51`) and `SubrequestDescriptor` (`procedure.rs:44`): wrap subrequests with labels and index/total metadata.
- `ProcedureDebugEventKind` and `ProcedureDebugEvent` (`procedure.rs:16`, `procedure.rs:26`): procedure/subrequest start, finish, failure, elapsed milliseconds, and error detail.
- `set_procedure_debug_sink` (`procedure.rs:65`) and `PLOKE_PROTOCOL_DEBUG` stderr JSON emission (`procedure.rs:70`, `procedure.rs:79`): diagnostic event outputs.

Selection contribution:
- `FanOut` and `Merge` artifacts preserve branch identity, which is essential for interpreting usefulness, redundancy, and recoverability branches separately before computing an aggregate.
- `ObservedSubrequest` makes per-branch LLM timing/failure visible. It can support cost and reliability comparisons if captured durably by an external runner.
- `ProcedureDebugEvent.elapsed_ms` is potentially useful for throughput/cost selection, but in scope it is only emitted to a sink or stderr. It is not part of `ProcedureArtifact`.

Limitation:
- Debug events are diagnostic projections, not durable selection evidence unless an external collector stores them and joins them to the relevant run or child.

### LLM adjudication data

Path: `crates/ploke-protocol/src/llm.rs`

- `JsonChatPrompt` (`llm.rs:19`): system and user text.
- `JsonLlmConfig` (`llm.rs:25`): `model_id`, optional `provider_slug`, `timeout_secs`, `max_attempts`, `max_tokens`; default model is `moonshotai/kimi-k2`.
- `JsonLlmProvenance` (`llm.rs:46`): `model_id`, optional `provider_slug`, `raw_content`, optional `reasoning`, and full `OpenAiResponse`.
- `ProtocolLlmError` (`llm.rs:57`): invalid model, request, unexpected tool calls, missing content, parse JSON.
- `JsonAdjudicationSpec` (`llm.rs:70`): step spec extension that builds a JSON prompt.
- `JsonAdjudicator` (`llm.rs:78`): OpenRouter JSON executor.
- `JsonLlmResult<T>` (`llm.rs:133`): parsed output plus content, reasoning, response.
- `adjudicate_json` (`llm.rs:200`): sends non-streaming JSON chat completion, applies max tokens and attempt timeout, parses JSON, and rejects tool calls.
- `parse_protocol_json_content` and `normalize_protocol_json_aliases` (`llm.rs:142`, `llm.rs:167`): tolerant recovery for capitalized `rationale` and `overall_rationale`.

Selection contribution:
- `JsonLlmProvenance` makes LLM-derived judgments auditable by model/provider/raw response.
- `raw_content` and `reasoning` allow later re-parse, challenge, or disagreement analysis.
- `ProtocolLlmError` can be treated as an evaluator reliability datum when comparing children, but current procedure errors abort rather than produce a partial scored record.

Limitation:
- There is no scoped adjudicator calibration, judge agreement metric, or persistent response cache.

## Tool-Call Evaluation Data

### Trace and sequence inputs

Path: `crates/ploke-protocol/src/tool_calls/trace.rs`

- `ToolKind` (`trace.rs:5`): `Search`, `Read`, `Browse`, `Edit`, `Execute`, `Other`.
- `Trace` and `Call` (`trace.rs:15`, `trace.rs:21`): minimal trace shape with `subject_id`, calls, index, turn, tool name, summary, failure.
- `NeighborhoodRequest` and `NeighborhoodSource` (`trace.rs:30`, `trace.rs:46`): request/source interface for building a bounded `ToolCallNeighborhood`.
- `TurnContext` (`trace.rs:56`): per-turn counts and patch booleans.
- `NeighborhoodCall` (`trace.rs:65`): index, turn, tool name/kind, failure, `latency_ms`, summary, args/result previews, optional `search_term`, optional `path_hint`.
- `ToolCallNeighborhood` (`trace.rs:88`): subject, total counts, turn, calls before/focal/after; `all_calls` iterates the local packet.
- `ToolCallSequence` (`trace.rs:108`): subject, total turns/calls, turn contexts, all `NeighborhoodCall`s.

Selection contribution:
- `ToolCallSequence` is the run-behavior substrate for child comparison: failed calls, tool mix, edit/execute usage, repeated searches, and directory pivots.
- `ToolCallNeighborhood` supports local failure/thrash diagnosis around a focal call.
- `latency_ms` can become a cost signal, but the current segment-level signals do not aggregate latency.

Stale or weak surface:
- `Trace` and `Call` are not re-exported from `crates/ploke-protocol/src/lib.rs` and are not used by the live segmentation/review procedures, which consume `ToolCallSequence`, `ToolCallNeighborhood`, and `NeighborhoodCall`.
- `NeighborhoodRequest` and `NeighborhoodSource` are public through `tool_calls::trace`, but not re-exported at the crate root.

### Intent segmentation metric and procedure

Path: `crates/ploke-protocol/src/tool_calls/segment.rs`

Metric:
- `tool_calls::segment::Metric` (`segment.rs:121`) implements `Measurement<Subject = ToolCallSequence, Value = SegmentedToolCallSequence>` and names itself `tool_call_intent_segmentation`.

Primary data:
- `IntentLabel` (`segment.rs:19`): locate, inspect, refine, validate, edit, recovery, other.
- `SegmentStatus` (`segment.rs:31`): labeled or ambiguous.
- `SequenceSignals` (`segment.rs:37`): total turns/calls, tool-kind counts, failed calls, repeated search runs, directory pivots, search terms seen.
- `SequenceReviewContext` (`segment.rs:53`): sequence plus derived signals.
- `IntentSegmentProposal` and `SegmentationJudgment` (`segment.rs:59`, `segment.rs:70`): LLM-proposed segment ranges, labels, confidence, rationale, overall rationale.
- `IntentSegment` (`segment.rs:76`): normalized segment with index/range/status/label/confidence/rationale/turns/calls.
- `UncoveredCallSpan` (`segment.rs:90`): contiguous uncovered calls and rationale.
- `SegmentationCoverage` (`segment.rs:98`): total/labeled/ambiguous/uncovered call counts.
- `SegmentedToolCallSequence` (`segment.rs:108`): full sequence, signals, normalized segments, coverage, uncovered spans/indices, overall rationale.

Procedure:
- `ContextualizeSequence` (`segment.rs:133`): mechanized step from `ToolCallSequence` to `SequenceReviewContext`.
- `SegmentByIntent` (`segment.rs:179`): LLM JSON step from context to `SegmentationJudgment`.
- `PreserveSequenceContext` (`segment.rs:224`): mechanized context branch keeper.
- `NormalizeSegments` and `NormalizeSegmentsError` (`segment.rs:260`, `segment.rs:263`): mechanized validation/normalization. Rejects empty sequences, invalid ranges, overlap, labeled-without-label, and ambiguous-with-label.
- `ToolCallIntentSegmentation` (`segment.rs:463`): named procedure `tool_call_intent_segmentation`.
- `IntentSegmentationArtifact` (`segment.rs:423`): full nested `ProcedureArtifact` carrying mechanized context, LLM judgment with `JsonLlmProvenance`, and normalized output with `MechanizedProvenance`.

Mechanized derivations:
- `derive_sequence_signals` (`segment.rs:512`): computes tool-kind counts, failed calls, repeated identical adjacent search terms, directory pivots, and unique search terms.
- `build_uncovered_spans` (`segment.rs:651`) and `derive_coverage` (`segment.rs:692`): compute uncovered spans and coverage counts.
- `render_sequence_context` (`segment.rs:594`) and `render_sequence_context_for_diagnostics` (`segment.rs:647`): prompt/diagnostic text projection.

Selection contribution:
- `SequenceSignals.failed_calls`, `repeated_search_runs`, and `directory_pivots` are immediate negative/instability signals.
- `SegmentationCoverage.uncovered_calls`, ambiguous segment counts, and low-confidence segment rationales can identify hard-to-evaluate or incoherent children.
- `IntentLabel::EditAttempt`, `ValidateHypothesis`, and `Recovery` segments make high-value behavior visible for selection.
- `IntentSegmentationArtifact` is more selection-grade than `SegmentedToolCallSequence` alone because it preserves LLM provenance and the normalized branch.

Limitation:
- The segmentation metric is categorical and structural, not a scalar score. A selector still needs an aggregation rule.

### Local tool-call and segment review metrics

Path: `crates/ploke-protocol/src/tool_calls/review.rs`

Metrics:
- `tool_calls::review::Metric` (`review.rs:159`) implements `Measurement<Subject = ToolCallNeighborhood, Value = LocalAnalysisAssessment>` and names itself `tool_call_review`.
- `tool_calls::review::SegmentMetric` (`review.rs:171`) implements `Measurement<Subject = SegmentReviewSubject, Value = LocalAnalysisAssessment>` and names itself `tool_call_segment_review`.

Primary data:
- `Concern` (`review.rs:19`): repeated cluster, search thrash, recovery opportunity, file pivot, ambiguous scope, residual coverage gap.
- `LocalAnalysisTargetKind` (`review.rs:30`): focal call or intent segment.
- `LocalAnalysisPacket` (`review.rs:36`): subject, target kind/id, scope summary, run/scope counts, turn span, optional focal or segment identifiers, calls.
- `LocalAnalysisSignals` (`review.rs:56`): scope turn count, repeated tool count, distinct tool count, tool-kind counts, failed calls, similar search neighbors, directory pivots, optional source segmentation counts, candidate concerns.
- `LocalAnalysisContext` (`review.rs:79`): packet plus signals.
- `UsefulnessVerdict`/`UsefulnessAssessment` (`review.rs:86`, `review.rs:95`): key progress through no value or unclear.
- `RedundancyVerdict`/`RedundancyAssessment` (`review.rs:103`, `review.rs:112`): distinct through search thrash or unclear.
- `RecoverabilityVerdict`/`RecoverabilityAssessment` (`review.rs:120`, `review.rs:129`): no recovery needed through no clear recovery or unclear.
- `OverallVerdict` (`review.rs:137`): focused progress, useful exploration, recoverable detour, redundant thrash, mixed, unclear.
- `LocalAnalysisAssessment` (`review.rs:147`): packet, signals, branch assessments, overall verdict, overall confidence, synthesis rationale.
- `SegmentReviewSubject` (`review.rs:183`): subject, sequence, selected segment, segmentation coverage.

Procedures:
- `ContextualizeNeighborhood` (`review.rs:191`): mechanized focal-neighborhood packet builder.
- `ContextualizeSegment` (`review.rs:253`): mechanized segment packet builder.
- `AssessLocalUsefulness` (`review.rs:322`): LLM branch.
- `AssessRedundancy` (`review.rs:354`): LLM branch.
- `AssessRecoverability` (`review.rs:386`): LLM branch.
- `AssembleAssessment` (`review.rs:430`): mechanized merge step that computes `OverallVerdict`, `overall_confidence`, and synthesis rationale.
- `ToolCallReview` (`review.rs:560`): named procedure `tool_call_review`.
- `ToolCallSegmentReview` (`review.rs:673`): named procedure `tool_call_segment_review`.
- `ToolCallReviewArtifact` and `ToolCallSegmentReviewArtifact` (`review.rs:513`, `review.rs:626`): nested artifacts carrying the mechanized context, three LLM branch artifacts, and mechanized assembly.

Mechanized derivations:
- `derive_signals` (`review.rs:751`): counts repeated focal tool names, distinct tools, tool-kind counts, failed calls, directory pivots, similar search neighbors, and candidate concerns.
- `derive_overall` (`review.rs:890`): maps branch verdicts into `OverallVerdict`; confidence is the minimum branch confidence.
- `render_review_context`, `render_call_block`, `render_signals` (`review.rs:945`, `review.rs:980`, `review.rs:1012`): prompt text projection.

Selection contribution:
- `LocalAnalysisAssessment.overall` is the closest existing selection datum. `FocusedProgress` and `UsefulExploration` are positive local signals; `RecoverableDetour` can indicate repairable behavior; `RedundantThrash` is a strong negative signal.
- `overall_confidence` lets a selector discount uncertain assessments.
- Branch-level `UsefulnessAssessment`, `RedundancyAssessment`, and `RecoverabilityAssessment` should be kept separate in selection. Collapsing to only `overall` would hide whether a child was low value, repetitive, or recoverable.
- `candidate_concerns` are mechanized triage tags suitable for pre-filtering or selecting focused review targets before spending LLM budget.

Limitation:
- The review procedure evaluates local scopes, not whole-child success. A successor selector needs an aggregation policy over all reviewed scopes and a join to objective outcome.

## Evaluation-Side Protocol Module

Path: `crates/ploke-eval/src/protocol/mod.rs`

- `mod.rs` states that protocol structure now lives in `crates/ploke-protocol` and keeps this module as a thin re-export (`mod.rs:1` to `mod.rs:5`).
- It re-exports `ploke_protocol::*` (`mod.rs:10`).
- It also includes and re-exports `protocol_aggregate` from `../protocol_aggregate.rs` (`mod.rs:7`, `mod.rs:11`). That target is outside this report's scope.

Path: `crates/ploke-eval/src/protocol/components.rs`

- Empty, 0 lines.

Path: `crates/ploke-eval/src/protocol/metrics.rs`

- Empty, 0 lines.

Path: `crates/ploke-eval/src/protocol/procedures.rs`

- Private sketch types: `ProtocolStep`, `Executor`, `RunId`, `PrintedToolCallLine`, `Step1InspectToolCalls`, `SuspiciousToolCallIndex`, `Step2SelectSuspiciousCall`, `LlmJudge`, `ChatGpt54`, `ToolSummary`, `Step3InspectToolCallDetail`.
- Implementations call `todo!()` at `procedures.rs:25`, `procedures.rs:42`, and `procedures.rs:56`.
- Visible CLI comments mention conceptual commands: `ploke-eval inspect tool-calls ...` (`procedures.rs:24`) and `ploke-eval inspect tool-call {index}` (`procedures.rs:55`).

Selection contribution:
- None as live code. The sketch names an intended flow: inspect printed tool-call lines, select suspicious call by LLM, inspect detail. That intent is superseded by the typed `ploke-protocol` procedures and should not be treated as durable selection evidence.

Stale/dead status:
- `components.rs` and `metrics.rs` are empty.
- `procedures.rs` is private to the module, not re-exported, and contains unimplemented `todo!()` bodies. Treat as stale conceptual scaffolding, not a runnable evaluator.

## Artifact Paths and Storage

Definitive scoped result: no concrete artifact path or storage implementation exists in the surveyed files.

Evidence:
- `crates/ploke-protocol/src/lib.rs` says the crate is organized around "persisted artifacts for each execution boundary" (`lib.rs:8`), and `crates/ploke-protocol/README.md` says it supports "persisted protocol artifacts consumed by ploke-eval".
- The actual scoped implementation returns `ProcedureRun<Output, Artifact>` and nested serializable artifacts from `run`; it does not define a filesystem path, database table, archive key, writer, reader, loader, retention policy, or report file format.
- `ProcedureDebugEvent` can be emitted through `set_procedure_debug_sink` or JSON to stderr under `PLOKE_PROTOCOL_DEBUG`, but this is not durable storage by itself.

Implication for HyperAgents:
- A child/successor selector must look outside this scope for storage, or introduce a durable join elsewhere: child id, generation, artifact/root path, procedure name, metric name, output, artifact, debug timing, and evaluator provenance.
- Within this scope, the proper stored object would be the full typed `ProcedureRun` or `ProcedureArtifact`, not only rendered prompt/report text.

## CLI and Report Projections Visible in Scope

Visible projections:
- `render_sequence_context` and `render_sequence_context_for_diagnostics` in `segment.rs` project `SequenceReviewContext` into prompt/diagnostic text.
- `render_review_context`, `render_call_block`, and `render_signals` in `review.rs` project `LocalAnalysisContext` into prompt text.
- `ProcedureDebugEvent` can be serialized to JSON stderr when `PLOKE_PROTOCOL_DEBUG` is enabled.
- `crates/ploke-eval/src/protocol/procedures.rs` mentions conceptual `ploke-eval inspect tool-calls` commands, but only inside `todo!()` sketch code.

Absent projections:
- No scoped CLI command implementation.
- No scoped Markdown/JSON report builder.
- No scoped child leaderboard, admission report, archive report, or successor-selection projection.

## Selection-Use Ranking of Existing Data

High-value selection inputs:
- `LocalAnalysisAssessment`: local usefulness/redundancy/recoverability/overall verdict with confidence and rationales.
- `SegmentedToolCallSequence`: global behavioral structure, coverage, ambiguous/uncovered regions, and sequence signals.
- `ToolCallReviewArtifact`, `ToolCallSegmentReviewArtifact`, `IntentSegmentationArtifact`: selection-grade when stored because they include branch artifacts and provenance.

Medium-value selection inputs:
- `SequenceSignals` and `LocalAnalysisSignals`: cheap mechanized triage for failed calls, repeated search, directory pivots, similar search neighbors, and candidate concerns.
- `JsonLlmProvenance`: evaluator audit, judge consistency analysis, and post-hoc reparse/review.
- `ProcedureDebugEvent.elapsed_ms`: evaluator cost and failure timing if captured externally.

Low-value or diagnostic-only inputs:
- Rendered prompt strings from `render_sequence_context` and `render_review_context`: useful for debugging evaluator context, not authoritative selection data.
- `MechanizedProvenance.strategy`: confirms native derivation but currently too coarse to rank children.
- `Trace`/`Call`: minimal legacy trace shape, not used by live procedures.

Not selection evidence:
- Empty `components.rs` and `metrics.rs`.
- `crates/ploke-eval/src/protocol/procedures.rs` private `todo!()` sketch.
- `ProcedureDebugEvent` stderr lines unless externally captured and joined to a child/procedure run.

## Gaps For Child/Successor Selection

- No scoped child id, generation id, parent id, archive id, or runtime/artifact root carrier.
- No scoped storage path or writer for procedure artifacts.
- No scoped aggregation rule from local assessments to child-level score.
- No scoped objective success datum or compile/test outcome datum.
- No scoped admission/selection record that says which child became successor and why.
- No scoped enforcement that LLM adjudicators obey `EvidencePolicy`; the policy is recorded and prompt-mediated.
- No scoped durability for `ProcedureDebugEvent` timing.

## Bottom Line

The scoped protocol code is ready to supply evaluation evidence, not to select successors by itself. The strongest existing path is:

1. Run `ToolCallIntentSegmentation` over each child's `ToolCallSequence`.
2. Run `ToolCallSegmentReview` on important segments and/or `ToolCallReview` on suspicious neighborhoods.
3. Store full `ProcedureRun` artifacts, not only final verdicts.
4. Join those artifacts outside this scope to child identity, generation, objective outcome, and archive path.
5. Aggregate `LocalAnalysisAssessment` and `SegmentedToolCallSequence` into successor-selection criteria.

