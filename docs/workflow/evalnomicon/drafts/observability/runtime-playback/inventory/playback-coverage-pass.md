# Playback Coverage Pass

Status: survey checklist.

This pass is for turning the source map into implementation-sized graph and
playback tasks. It should be run before adding a new CLI column, egui panel, or
model-facing query over Prototype 1 history.

## Per-Item Worksheet

For each emitted record or derived fact, fill this out:

| Field | Question |
| --- | --- |
| Owner type | Which named Rust type owns the persisted or wire shape? |
| Writer | Which crate/module decides to write it? |
| Persisted path | Where does it land under `~/.ploke-eval` or the active run root? |
| Reader | Which typed loader reads it today, if any? |
| Graph status | Core graph node, evidence attachment, loaded side payload, summary only, or missing? |
| Semantic object | History, Artifact, Runtime, Operation, Attempt, Candidate, Selection, Evaluation, AgentTurn, ToolCall, IndexSnapshot, or other? |
| Join key | Which ids bind it to other graph facts? |
| Sequence | Which playback step family owns its order? |
| Granularity | Graph universe, lineage, artifact ancestry, runtime, operation, attempt, agent turn, model exchange, tool call, edit event, or DB snapshot? |
| Evidence class | History authority, transition evidence, passive evidence, diagnostic projection, or render-only? |
| Duplicate pressure | Is there a writer type, passive type, projection type, or partial view carrying the same facts? |
| Gap | What would make the item playable from the shared cursor? |

The latest-run validation worksheet is
[`latest-run-emission-worksheet.md`](latest-run-emission-worksheet.md). Use it
as the current concrete check against `~/.ploke-eval`: it lists the actual file
families observed for the newest run and marks which rows are missing,
summary-only, referenced-only, or outside the run root.

## Graph Coverage Classes

Use these labels while surveying.

- `core`: represented in a typed graph index, such as History, Artifact,
  Runtime, Operation, Candidate, Selection, Metric, or ChildPlan.
- `evidence`: attached to graph objects through `Graph.evidence`.
- `loaded-side-payload`: carried by `RunRecordSet` or `Graph` but not yet
  indexed as steps, such as `AgentTurnRecordSet`.
- `summary-only`: counts are loaded, but individual records are not graph
  facts, such as runtime channel envelopes today.
- `referenced-only`: file paths or ids are stored, but the referenced artifact
  is not loaded into graph memory.
- `missing-loader`: emitted by the loop but not read by `FsRunStore`.
- `outside-run-root`: stored in a workspace, config directory, or DB location
  that is referenced but not part of the run-root load.

## Join Keys

These are the keys that should be preserved before any rendering or
aggregation:

| Key | Joins |
| --- | --- |
| `lineage_id`, `block_id`, `block_hash`, block height | History order, lineage heads, authority epochs |
| `entry_id` | History entry, selection decision, candidate payloads, formula rows |
| `artifact_id`, `tree_key`, `branch_id` | Artifact state, scheduler branch, surface evidence, selected successor |
| `runtime_id` | invocation, channel envelope, transition journal runtime event, agent attempt |
| `node_id` | scheduler node, runner request/result, invocation, channel root, child plan |
| `operation Coordinate` | runtime action over Artifact/target, runner request/invocation operation target |
| `candidate_set_root`, `membership_id`, `occurrence_id` | selection candidate universe and decision-grade membership |
| `manifest_id`, `run_id`, `instance_id`, `record_key` | compressed run record, benchmark run, protocol artifact, evaluation comparison |
| `task_id`, `user_message_id`, `assistant_message_id`, `response_index` | agent-turn record, provider request/response sidecars, model exchange |
| `request_id`, `call_id`, `tool` | tool request/result, repair loops, tool behavior aggregates |
| `db_timestamp_micros`, snapshot path | Cozo time-travel marker, index snapshot, historical DB query |
| channel cursor and `message_id` | ordered channel envelope playback |

## Sequencing Gaps To Close

- Add a per-envelope runtime-channel playback family. Current graph import only
  stores channel counts.
- Add a provider exchange/request snapshot family. Current agent-turn records
  have prompt messages and responses, older run records may carry
  `ChatCompReqCore`, and replay sidecars preserve full provider responses
  separately. The missing piece is a durable full request witness for model,
  tools, tool-choice policy, router-specific fields, and selected endpoint.
- Add an index/reindex playback family for parse, index, DB snapshot, failure,
  and time-travel-marker state.
- Add a protocol procedure playback family that can step through procedure
  artifacts without forcing egui to parse protocol JSON directly. The latest
  checked run wrote separate `tool_call_intent_segmentation`,
  `tool_call_review`, and `tool_call_segment_review` artifacts; keep those
  procedure names visible in the worksheet instead of collapsing them into a
  single protocol bucket.
- Add a run-record turn/tool iterator that chooses an authority source and
  does not double-count the same turn when both `record.json.gz` and
  `agent-turn-*` are available.
- Add explicit graph warnings for emitted files that are loaded as summaries
  only but have ids that should join to core graph objects.
- Add benchmark/export playback rows for `multi-swe-bench-submission.jsonl`
  and `benchmark-patch-projection.json`. These are benchmark-facing evidence
  and patch-projection witnesses, not raw assistant-output authority.
- Add outside-run-root anchor rows for run registration, campaign, closure,
  batch, and latest-run pointer files. Distinguish registry authority from
  operator convenience files such as `last-run.json`.

## First Refactor Targets

1. Teach the graph load path about `llm-full-responses.jsonl` as a typed side
   payload, likely under the agent-turn timeline rather than as a core graph
   node.
2. Decide whether provider-request evidence needs a new canonical
   `ploke-records` owner type or can be derived from an extended
   `AgentTurnArtifactRecord`. Do not add a CLI-only request snapshot type.
3. Promote channel envelopes from summary-only passive evidence to ordered
   runtime/attempt evidence.
4. Define a DB/index snapshot witness that starts with existing run-record
   paths and time-travel markers rather than trying to load Cozo databases into
   the graph.
5. Add borrowed graph accessors for selection metrics and formula rows needed
   by egui before any new renderer-specific table types.
6. Add a synthetic `GraphUniverse` fixture that contains one History block,
   one selected Artifact, one child invocation, one channel exchange, one
   evaluation artifact, one run record, one agent turn, one provider response,
   and one DB snapshot reference.
7. Decide whether TUI `proposals.json` should be captured as run evidence for
   eval runs that redirect `PLOKE_PROPOSALS_PATH`; otherwise keep it as a local
   TUI persistence surface outside default playback authority.

## Output Of A Finished Pass

A finished pass should leave:

- an updated row in `record-surface-map.md`;
- one proposed graph fact or accessor, if needed;
- one proposed playback step family and granularity;
- one verification target, preferably a synthetic graph test;
- one note on whether any duplicate or partial type should be removed,
  preserved, or turned into an owner type.
