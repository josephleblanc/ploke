# Record Persistence Checklist

Status: draft checklist derived from `record-surface-map.md`.

Use this checklist during Prototype 1 run review, doctor follow-up, or playback
coverage work. It is not a claim that every surface must exist for every phase.
It is a way to mark the expected persistence surfaces precisely enough that
missing evidence does not get confused with playback gaps or operator
convenience files.

## Status Labels

Use one of these labels for each checked item:

- `present`: the expected file or typed record exists for this run/phase.
- `record absent`: the expected file or typed record was not written.
- `record present, playback gap`: the record exists, but graph/playback does
  not expose it as an ordered typed step or drilldown family yet.
- `record present, manual join needed`: the record exists, but review had to
  join it manually by path, id, timestamp, git state, or sidecar reference.
- `operator/convenience record`: useful for discovery or reporting, but not an
  authority source.
- `not applicable`: the phase or feature was not expected for this run.

## Authority And Ordering Spine

- [ ] Sealed History blocks:
  `history/blocks/segment-*.jsonl`.
  Expected owner: `SealedBlockRecord`.
  Primary question: is there sealed lineage authority for the claimed
  transition, selection, successor handoff, or imported evidence?
- [ ] Transition journal:
  `prototype1/transition-journal.jsonl` or campaign-level equivalent.
  Expected owner: `TransitionJournal`.
  Primary question: do typed transition events prove the phase boundary and
  node/runtime/branch coordinates?
- [ ] Parent identity:
  `.ploke/prototype1/parent_identity.json` in the active parent/successor
  checkout.
  Expected owner: `ParentIdentityRecord`.
  Primary question: which Crown holder is allowed to mutate the active lineage?
- [ ] Run profile and commitment:
  `prototype1/run-profile.toml` and `run-profile.commitment.json` when present.
  Expected owner: `RunProfileRecord` and `RunProfileCommitmentRecord`.
  Primary question: what policy, model/provider route, fanout, stop mode, MBE,
  and protocol settings were admitted?
- [ ] Scheduler projection:
  `scheduler.json`.
  Expected owner: `SchedulerStateRecord`.
  Primary question: what does the compatibility projection say, and does it
  disagree with fresher History, journal, node, or process evidence?

## Parent Planning And Child Admission

- [ ] Branch registry:
  `branches.json`.
  Join keys: `branch_id`, `node_id`, `campaign_id`.
- [ ] Child plan:
  `messages/child-plan/*.json`.
  Join keys: `parent_node_id`, child `node_id`, `branch_id`.
- [ ] Node record:
  `nodes/<node-id>/node.json`.
  Join keys: `node_id`, `generation`, `branch_id`, `artifact_id`.
- [ ] Runner request:
  `nodes/<node-id>/runner-request.json`.
  Join keys: `runtime_id`, `node_id`, `branch_id`, operation target.
- [ ] Invocation records:
  `nodes/<node-id>/invocations/*.json`.
  Join keys: `runtime_id`, `node_id`, process argv, operation target.
- [ ] Runtime result records:
  `nodes/<node-id>/runner-result.json` and
  `nodes/<node-id>/results/<runtime-id>.json`.
  Join keys: `runtime_id`, `node_id`, `branch_id`.
- [ ] Successor readiness/completion:
  `nodes/<node-id>/successor-ready/*.json` and
  `nodes/<node-id>/successor-completion/*.json`.
  Join keys: `node_id`, `runtime_id`, successor identity.
- [ ] Runtime channels:
  `nodes/<node-id>/channels/<runtime-id>/parent-to-child.jsonl` and
  `nodes/<node-id>/channels/<runtime-id>/child-to-parent.jsonl`.
  Join keys: `runtime_id`, `node_id`, `message_id`, direction, cursor offset.
  Current expectation: often `record present, playback gap` because envelopes
  are counted or summarized before they become ordered playback steps.

## Broad Harness And Edit Attempts

- [ ] Published edit request:
  `messages/edit-harness-request/<attempt>.json` and optional `.md`.
  Primary question: what prompt, target artifact, policy receipt, and validation
  contract was issued to the child model?
- [ ] Submitted edit result:
  `messages/edit-harness-result/<attempt>.json`.
  Primary question: did the harness submit an admissible candidate artifact?
- [ ] Headless TUI trace:
  `messages/edit-harness-result/<attempt>.headless-tui.json`.
  Primary question: what tool events, model-visible results, terminal record,
  and proposal lifecycle were written?
- [ ] Candidate workspace:
  `prototype1/workspaces/edit-harness/<attempt>/`.
  Primary question: does git prove the claimed commit, changed files, clean
  status, and parent target commit?
- [ ] Edit proposal lifecycle:
  proposal ids, request ids, call ids, `staged`, `applied`, `failed`,
  `stale`, and `denied` events in the trace or proposal sidecar.
  Primary question: did `ToolCallCompleted` mean staged-only or actually
  applied?
- [ ] Surface/policy receipt:
  surface attempt, request policy, touched files, file hashes, and protected
  surface check.
  Primary question: was the edit admissible under the granted surface, and did
  it avoid protected core?
- [ ] Model-visible validation commands:
  shell/cargo tool events and their model-facing payloads.
  Primary question: did the model run the requested checks, and did it see the
  relevant output?

## Eval Run Root

- [ ] Run registration:
  registry entry under `registries/runs/<run-id>.json` or equivalent campaign
  registration.
  Primary question: where is the run root, protocol anchor, model route, and
  artifact set for the run?
- [ ] Compressed run record:
  `record.json.gz`.
  Expected owner: `RunRecord`.
  Primary question: what turn/tool sequence, metrics, artifact refs, and
  runtime facts were persisted?
- [ ] Agent turn trace:
  `agent-turn-trace.json`.
  Expected owner: `AgentTurnTraceRecord`.
  Primary question: what ordered tool/model events can be replayed?
- [ ] Agent turn summary:
  `agent-turn-summary.json`.
  Expected owner: `AgentTurnSummaryRecord`.
  Primary question: what terminal outcome, final assistant message, usage, and
  compact turn summary were recorded?
- [ ] Full provider responses:
  `llm-full-responses.jsonl`.
  Expected owner: `RawFullResponseRecord`.
  Current expectation: often `record present, manual join needed` until loaded
  through the record store beside agent-turn records.
- [ ] Validation audit:
  `validation-audit.json`.
  Primary question: which tool completions, command outputs, or validation
  events were recorded, and does the table omit model-facing payload details?
- [ ] Benchmark patch projection:
  `benchmark-patch-projection.json`.
  Expected owner: `BenchmarkPatchProjectionRecord`.
  Primary question: did the run produce a benchmark-facing patch projection, and
  was projection validation successful?
- [ ] Multi-SWE-bench submission:
  `multi-swe-bench-submission.jsonl`.
  Expected owner: `MultiSweBenchSubmissionRecord`.
  Primary question: what patch was exported for the benchmark target?
- [ ] Batch manifests and summaries:
  `batch.json`, per-instance `run.json`, `batch-run-summary.json`, and
  batch-level submission files where applicable.
  Primary question: how does this run join to the batch/campaign closure rows?

## Index And Database Witnesses

- [ ] Indexing status:
  `indexing-status.json`.
  Primary question: did indexing complete, fail, or record partial status?
- [ ] Parse failure artifact:
  `parse-failure.json`.
  Primary question: did parsing/indexing fail in a way that affects tool
  reliability?
- [ ] Indexing checkpoint database:
  `indexing-checkpoint.db`.
  Primary question: is there a DB snapshot witness for mid-run indexing state?
- [ ] Indexing failure database:
  `indexing-failure.db`.
  Primary question: is there a DB snapshot witness for indexing failure?
- [ ] Snapshot status:
  `snapshot-status.json`.
  Primary question: where is the final snapshot and was snapshot persistence
  successful?
- [ ] Final snapshot database:
  `final-snapshot.db`.
  Primary question: what DB state can be inspected after the run/turn?
- [ ] Time-travel markers:
  `db_time_travel_index` in the run record.
  Primary question: can turn/tool events be joined to Cozo validity timestamps?
- [ ] Redirected TUI proposal store:
  `config/ploke/proposals.json` or redirected `PLOKE_PROPOSALS_PATH`, only if
  the run captured that path or proposal ids.
  Current expectation: operator/config evidence unless explicitly recorded by
  the run.

## Protocol And Adjudication

- [ ] Protocol artifact root:
  run registration `protocol_anchor` or compared run protocol artifact path.
  Primary question: where should protocol evidence live for this run?
- [ ] Tool-call intent segmentation:
  `*_tool_call_intent_segmentation_*.json`.
  Primary question: how were tool calls grouped into candidate reasoning spans?
- [ ] Tool-call review:
  `*_tool_call_review_*.json`.
  Primary question: what local judgments were made about usefulness, recovery,
  redundancy, validation, or failure?
- [ ] Tool-call segment review:
  `*_tool_call_segment_review_*.json`.
  Primary question: what segment-level judgments were made?
- [ ] Protocol procedure provenance:
  procedure name, subject id, run id, path, prompt/model/provider metadata.
  Current expectation: often `record present, playback gap` because nested
  protocol steps are summarized before they become ordered playback frames.

## Evaluation, Selection, And Successor Evidence

- [ ] Evaluation artifacts:
  `evaluations/*.json`.
  Expected owner: `ploke_records::evaluation::Artifact`.
  Primary question: what baseline/treatment comparison or oracle evidence was
  written?
- [ ] Compared run evidence:
  compared baseline/treatment run roots referenced by evaluation artifacts.
  Join keys: `branch_id`, `instance_id`, run root, record key.
- [ ] Oracle or MBE evidence:
  MBE final reports, oracle verdict files, or attached evaluation refs when
  enabled by profile.
  Primary question: was required oracle evidence present and attached?
- [ ] Selection decision History payload:
  `SelectionDecisionEntryRecord` inside sealed History.
  Primary question: what candidates were considered, which one was selected,
  and what metric/formula/proof justified it?
- [ ] Candidate payloads:
  `EvaluationPayloadRecord`, `CandidateEvidenceRecord`,
  `CandidateArtifactRecord`.
  Primary question: what candidate artifact, branch, source hashes, and surface
  attempts were selected or rejected?
- [ ] Metric set and formula:
  `MetricSet`, `MetricCandidate`, `ImpAtK`, `SelectionFormulaRecord`, and
  `ScoreChildPropRecord`.
  Primary question: can selection be replayed deterministically from persisted
  inputs instead of summaries?
- [ ] Projection failures:
  `ProjectionFailureRecord`.
  Primary question: were candidates excluded, downgraded, or made
  non-comparable?

## Model Request And Provider Provenance

- [ ] Prompt/request messages:
  `llm_prompt` inside agent-turn artifacts or request tap records.
  Primary question: what messages did the model receive?
- [ ] Complete router request:
  provider route, endpoint/model choice, tool list, tool-choice policy,
  reasoning/thinking settings, and provider-specific body.
  Current expectation: may be fragmented or absent; do not infer from current
  model cache.
- [ ] Raw provider response:
  `llm-full-responses.jsonl`.
  Primary question: what tool calls, final content, finish reason, usage, and
  provider error shape came back?
- [ ] Model/provider registry cache:
  local `ploke-llm` cache files.
  Current expectation: configuration/cache surface only, not historical run
  authority unless selected model/provider provenance was persisted with the
  run or turn.

## Operator Convenience And Discovery Files

- [ ] Campaign manifest:
  `campaign.json`.
  Treat as scope and discovery evidence, not sealed transition authority.
- [ ] Closure state:
  `closure-state.json`.
  Treat as closure progress/eval row evidence; join back to run roots and
  protocol anchors before claiming benchmark success.
- [ ] Campaign and batch registries:
  campaign files, batch files, and registry projections outside a run root.
  Treat as lifecycle/discovery evidence unless referenced by sealed History or
  run registration.
- [ ] `last-run.json` or similar pointers.
  Treat as operator convenience only.
- [ ] Logs and stderr/stdout files.
  Treat as useful diagnostic evidence, especially for liveness and timing, but
  not as the authority spine when typed records exist.

## Review Output Requirements

When using this checklist in a run review, include:

- the exact campaign, node, runtime id, branch id, and run root being checked;
- the highest authority surface found for the claim under review;
- a short table or bullets separating `record absent`, `playback gap`,
  `manual join needed`, and `operator/convenience record`;
- at least one concrete trace chain from model/tool event to later action or
  failure;
- any action item tied to the specific missing surface, playback gap, or
  authority mismatch.
