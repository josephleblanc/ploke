# 2026-06-22 — Prototype 1 Relational Data Model Draft

Status: review-incorporated draft / pre-implementation design note.

Short description: logical relational shape for Prototype 1 persisted data once ordinary eval evidence, traces, projections, and refs move behind an owner-scoped store. This is not a final Cozo DDL. It is the semantic model needed to query parent/child/successor lifetimes, artifact/runtime operations, selection evidence, and agent traces without collapsing authority domains.

Related files:

- [`storage-plan.md`](storage-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`cozo-feature-map.md`](cozo-feature-map.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md)
- [`schema-review.md`](schema-review.md)
- [`record-usefulness-triage.md`](record-usefulness-triage.md)
- [`open-questions.md`](open-questions.md)

## Design goal

The filesystem layout currently encodes too much protocol state in paths. The database model should make logical entities explicit so we can query:

- a `Parent` epoch across startup, planning, fanout, selection, handoff, and completion;
- a `Child` candidate across plan membership, invocation, channel readiness, evaluation, result, comparison, and cleanup;
- a concrete runtime attempt across invocation, channel, streams, logs, and terminal evidence;
- a `Successor` across sealed History admission, invocation, ready acknowledgement, and next parent startup;
- an artifact/runtime operation: which runtime generated which patch over which artifact surface;
- a selection decision: which candidate set, metric policy, formula, findings, and evidence led to continuation or stop;
- an agent/tool loop timeline across model exchanges, tool calls, edit proposals, validation, costs, and adapter observations;
- policy/edit-surface overlays over code graph nodes without mutating base code graph facts.

The model preserves current authority distinctions:

- `History` / sealed `Block` authority is referenced, not replaced.
- `Channel<R, T: Transport>` authority is represented through message/receipt/import evidence, not generic record rows.
- `MessageBox` authority is represented through box lock/unlock facts and optional payload mirrors.
- Artifact state and checkout/backend mutation are referenced, not reduced to ordinary eval records.
- Child/treatment-local DB facts stay local unless imported under explicit scope.

## Source alignment

This draft follows the source-of-truth order from `crates/ploke-eval/docs/orientation/source-of-truth.md`: current source and `crates/ploke-eval/docs` for current behavior, Evalnomicon for conceptual framing, active docs for rolling design.

Important cross-checks folded into this draft:

- Runtime authority: durable records are projections of allowed transitions, not arbitrary status writes.
- Artifact/runtime model: durable provenance needs artifact graph, runtime derivation graph, and operation graph; the key operation coordinate is `(generator Runtime, target Artifact)`.
- History/Crown: sealed History is lineage authority; side tables and database overlays are projections/evidence.
- Parent/child channel: lifecycle advancement comes from per-runtime channel semantics, not scheduler/result projections.
- Runtime playback: ordering is causal, not timestamp-only; missing joins should surface evidence warnings.
- Agent-turn playback: one turn needs an ordered event timeline, not only response/tool sidecars.

## Relation layers

### 1. Entity relations

Entity relations identify durable objects with lifetimes. They should be small, scoped, and joinable.

Core entity families:

```text
eval_campaign
eval_lineage
eval_runtime
eval_parent_epoch
eval_attempt
eval_child
eval_successor
eval_artifact
eval_invocation
eval_channel
eval_message_box
eval_history_ref
eval_profile_commitment
eval_run
eval_baseline
eval_agent_turn
eval_policy_surface
```

### 2. Fact/event relations

Fact/event relations append observations about entity state over time. They reconstruct trajectories and playback steps.

Core event/fact families for the first storage pass:

```text
eval_transition_event
eval_trace_event
eval_candidate_event
eval_channel_message
eval_channel_receipt
eval_message_box_event
eval_operation
eval_patch
eval_apply_event
eval_build_event
eval_candidate_set
eval_candidate_member
eval_evaluation
eval_selection_decision
eval_import_event
eval_agent_turn_event
eval_evidence_warning
```

Follow-on code-graph-backed event/fact families:

```text
eval_validation_event
eval_validation_cover
eval_retrieval
eval_retrieval_hit
eval_refresh_event
```

### 3. Evidence/ref relations

Evidence/ref relations point at structured records, logs, blobs, DB snapshots, and artifact files. Large or sensitive data can stay outside the DB while remaining hash-addressed and scoped.

Core evidence/ref families for the first storage pass:

```text
eval_record_ref
eval_blob_ref
eval_log_ref
eval_snapshot_ref
eval_artifact_ref
eval_closure_ref
eval_protocol_ref
eval_model_ref
```

Follow-on code graph bridge/ref families:

```text
eval_code_snapshot
eval_code_ref
eval_code_link
```

## Common axes

Most relations should carry a subset of these fields. Use `null` only where genuinely not applicable.

### Identity

```text
campaign_id          -- Prototype 1 campaign namespace
lineage_id           -- History/Crown lineage coordinate
parent_id            -- parent epoch id
runtime_id           -- concrete runtime process/attempt id
attempt_id           -- attempt occurrence id, often equal to runtime id
node_id              -- current Prototype 1 candidate-node join key, not a code graph node id
child_id             -- child candidate id, usually parent/node scoped
artifact_id          -- recoverable artifact id
patch_id             -- generated or composed patch id
operation_id         -- runtime + target operation id
run_id               -- eval/treatment run id
turn_id              -- agent turn id
channel_id           -- logical channel id
box_id               -- logical message box id
record_ref_id        -- typed record/evidence ref id
```

### Scope, visibility, and evidence class

Do not overload one `owner_scope` field with all concepts. Keep ownership, visibility, source, and evidence strength separate.

```text
store_scope          -- parent | child_runtime | successor_runtime | treatment_run | campaign | artifact | external
producer_role        -- parent | child | successor | eval_runner | harness | operator | provider | unknown
visibility_scope     -- local | parent_visible | successor_visible | imported | public_debug
source_class         -- direct_write | channel_payload | channel_ref | message_box_mirror | passive_mirror | log_parse | compatibility_import
evidence_class       -- sealed_history | admitted_channel | typed_transition | passive_record | diagnostic | compatibility | unverified
authority_domain     -- projection | trace | blob_ref | message_box | channel | history_ref | artifact | policy
validation_status    -- unchecked | valid | invalid | rejected | degraded | imported
```

`store_scope` is the ownership boundary. `visibility_scope` says who may rely on or query it. `source_class` says how it entered the store. `evidence_class` says how strong the fact is. A parent-visible imported child summary is not the same as a child-local raw DB fact.

### Ordering

Ordering should be causal where possible, not timestamp-only.

```text
source_stream_id     -- journal/channel/log/turn stream id
source_event_index   -- append index within a structured stream
source_cursor        -- byte offset, transport cursor, or equivalent
source_line          -- source line for JSONL/file imports
runtime_sequence     -- sequence emitted by one runtime when available
parent_sequence      -- sequence within one parent epoch when available
recorded_at          -- producer timestamp
ingested_at          -- DB/import timestamp
```

Examples:

- History: `(lineage_id, block_height, entry_index)`.
- Transition journal: `(campaign_id, source_event_index)`.
- Channel: `(channel_id, direction, source_cursor)` or `message_id`.
- Agent turn: `(turn_id, source_event_index)`.
- Logs: `(log_ref_id, byte_start)`.

### External payload refs

```text
source_ref           -- path, URI, object key, or debug handle
content_sha256       -- hash of external payload bytes
schema_version       -- serialized schema marker
payload_ref          -- ref to another row/blob/object
payload_json         -- optional inline JSON for small records only
sensitivity          -- normal | llm_payload | secret_risk | unknown
```

## Entity sketches

These are logical shapes. Exact Cozo keys can change after first implementation slices.

### `eval_campaign`

Campaign-level identity and storage roots.

```text
eval_campaign {
  campaign_id: String =>
  manifest_ref: String,
  prototype_root: String,
  created_at: String?,
  profile_ref: String?,
  storage_backend: String?
}
```

### `eval_lineage`

Lineage is the History/Crown coordinate. It is not a process id, branch, path, or global singleton.

```text
eval_lineage {
  lineage_id: String =>
  campaign_id: String,
  genesis_parent_id: String?,
  head_ref: String?,             -- projection only, rebuildable from History
  created_at: String?
}
```

### `eval_runtime`

One concrete process/runtime. This is independent of role-specific facts.

```text
eval_runtime {
  runtime_id: String =>
  campaign_id: String,
  role: String,                  -- parent | child | successor | eval_runner | harness
  store_scope: String,
  node_id: String?,
  parent_id: String?,
  artifact_id: String?,
  host_ref: String?,             -- local host, VM, device, container, unknown
  started_at: String?,
  ended_at: String?,
  status: String?
}
```

### `eval_parent_epoch`

A parent generation/epoch. A parent epoch may have one or more runtime attempts under restart/recovery, but only admitted typestate/History paths can make it ruling.

```text
eval_parent_epoch {
  parent_id: String =>
  campaign_id: String,
  lineage_id: String,
  runtime_id: String?,
  node_id: String,
  generation: Int,
  branch_id: String,
  predecessor_id: String?,
  artifact_id: String?,
  tree_hash: String?,
  startup_kind: String,           -- genesis | predecessor
  profile_ref: String?,
  started_at: String?,
  completed_at: String?
}
```

### `eval_attempt`

Attempt-scoped occurrence for child/successor/eval-run execution. This prevents mutable node projections from overwriting attempt history.

```text
eval_attempt {
  attempt_id: String =>
  campaign_id: String,
  runtime_id: String,
  role: String,
  parent_id: String?,
  node_id: String?,
  invocation_id: String?,
  channel_id: String?,
  artifact_id: String?,
  binary_ref: String?,
  started_at: String?,
  status: String?
}
```

### `eval_child`

Child candidate tied to a parent and node.

```text
eval_child {
  child_id: String =>
  campaign_id: String,
  parent_id: String,
  node_id: String,
  generation: Int,
  branch_id: String,
  candidate_id: String,
  member_id: String?,
  planned_index: Int?,
  base_artifact_id: String?,
  derived_artifact_id: String?,
  status: String?
}
```

### `eval_successor`

Selected continuation attempt.

```text
eval_successor {
  successor_id: String =>
  campaign_id: String,
  predecessor_id: String,
  selected_node_id: String,
  runtime_id: String,
  artifact_id: String,
  block_hash: String?,
  invocation_id: String?,
  ready_at: String?,
  completed_at: String?,
  outcome: String?
}
```

### Legacy scheduler/node projections

Do not add `Prototype1SchedulerState` / `scheduler.json` as a first-class normalized relation in the initial database schema. It is a mutable legacy projection: useful for compatibility, debug context, and old reports, but not the active authority for child planning, selection, continuation, History admission, or successor causality.

The current `node_id` remains an important compatibility join key because child plans, invocations, channels, streams, runner requests/results, and evaluation reports refer to it. Model those semantic facts through `eval_child`, `eval_attempt`, `eval_candidate_member`, `eval_invocation`, `eval_channel_*`, `eval_evaluation`, and `eval_selection_*` relations. If raw `scheduler.json` or `nodes/<node>/node.json` records are ingested, store them as `eval_record_ref` rows with `source_class = compatibility_import` or `passive_mirror`, not as authority-shaped entities.

Status changes derived from typed transitions or admitted channel results belong in `eval_candidate_event`.

### `eval_artifact`

Recoverable artifact identity and lineage. Worktree paths and branch names are handles, not semantic identity.

```text
eval_artifact {
  artifact_id: String =>
  campaign_id: String,
  tree_hash: String?,
  git_branch: String?,
  git_commit: String?,
  source: String,                 -- active_parent | child_worktree | broad_harness | treatment_run | imported
  store_scope: String,
  created_by: String?,            -- runtime_id
  parent_artifact_id: String?
}
```

### `eval_artifact_surface`

Surface commitment/partition associated with an artifact or sealed History ref.

```text
eval_artifact_surface {
  surface_id: String =>
  campaign_id: String,
  artifact_id: String,
  immutable_root: String?,
  mutated_root: String?,
  ambient_root: String?,
  surface_hash: String?,
  source_ref: String?,
  recorded_at: String?
}
```

### `eval_workspace_ref`

Local checkout/worktree handle. It is not artifact identity by itself.

```text
eval_workspace_ref {
  workspace_ref_id: String =>
  campaign_id: String,
  artifact_id: String?,
  runtime_id: String?,
  path: String,
  backend: String,                -- git_worktree | checkout | harness_workspace | remote
  recoverable: Bool,
  recorded_at: String?
}
```

### `eval_binary_ref`

Built binary provenance.

```text
eval_binary_ref {
  binary_ref_id: String =>
  campaign_id: String,
  artifact_id: String?,
  built_by: String?,              -- runtime_id
  source_ref: String,
  content_sha256: String?,
  protocol_digest: String?,
  recorded_at: String?
}
```

### `eval_invocation`

Attempt bootstrap descriptor. Remote execution should deliver the same facts through a bootstrap package or launch transport.

```text
eval_invocation {
  invocation_id: String =>
  campaign_id: String,
  runtime_id: String,
  role: String,                   -- child | successor
  node_id: String,
  parent_id: String?,
  journal_ref: String?,
  channel_id: String?,
  channel_ref: String?,
  node_ref: String?,
  request_ref: String?,
  resolved_ref: String?,
  active_root_ref: String?,
  profile_ref: String?,
  binary_ref: String?,
  argv_ref: String?,
  created_at: String
}
```

### `eval_channel`

Logical channel identity. The transport backend may be file JSONL, socket, queue, object store, etc.

```text
eval_channel {
  channel_id: String =>
  campaign_id: String,
  node_id: String,
  runtime_id: String,
  parent_id: String?,
  transport_kind: String,
  endpoint_ref: String?,
  created_at: String?
}
```

### `eval_message_box`

Logical typed box identity, such as the child-plan box.

```text
eval_message_box {
  box_id: String =>
  campaign_id: String,
  box_kind: String,               -- child_plan
  owner_parent_id: String,
  lock_transition: String,
  unlock_transition: String,
  backing_kind: String,           -- fs_file | db_slot | object_ref
  backing_ref: String?
}
```

The payload mirror belongs in `eval_record_ref` or a box payload ref. The box relation records the authority shape.

### `eval_history_ref`

Reference to sealed History material. This is not the sealed block store itself.

```text
eval_history_ref {
  history_ref_id: String =>
  campaign_id: String,
  lineage_id: String,
  block_hash: String,
  block_height: Int,
  store_ref: String,
  selected_runtime_id: String?,
  selected_artifact_id: String?,
  recorded_at: String?
}
```

### `eval_profile_commitment`

Admitted run profile/config commitment. Selection, fanout, model defaults, protocol settings, and execution stops should join through this relation.

```text
eval_profile_commitment {
  profile_ref_id: String =>
  campaign_id: String,
  schema_version: String,
  profile_name: String?,
  source_ref: String,
  content_sha256: String,
  admitted_at: String?,
  storage_ref: String?
}
```

Related policy/config projections may be normalized later:

```text
eval_policy_config
eval_model_config
eval_protocol_config
```

### `eval_run`

Eval/treatment run identity. Run-local DB snapshots and run records attach here.

```text
eval_run {
  run_id: String =>
  campaign_id: String,
  treatment_id: String?,
  node_id: String?,
  runtime_id: String?,
  store_scope: String,            -- treatment_run | child_runtime | parent
  run_root_ref: String?,
  record_ref: String?,
  status: String?,
  started_at: String?,
  completed_at: String?
}
```

### `eval_baseline`

Parent baseline identity for generation 0 and successor generations.

```text
eval_baseline {
  baseline_id: String =>
  campaign_id: String,
  parent_id: String,
  source_kind: String,            -- generation0_closure | selected_child_eval
  closure_ref: String?,
  evaluation_id: String?,
  record_ref: String?,
  status: String,
  recorded_at: String?
}
```

### `eval_agent_turn`

Turn-level identity. Detailed timeline is in `eval_agent_turn_event` and related model/tool/edit tables.

```text
eval_agent_turn {
  turn_id: String =>
  campaign_id: String,
  run_id: String?,
  runtime_id: String?,
  node_id: String?,
  turn_index: Int,
  model_id: String?,
  provider: String?,
  trace_ref: String?,
  summary_ref: String?,
  started_at: String?,
  completed_at: String?
}
```

## Event/fact sketches

### `eval_transition_event`

Typed transition facts from the typestate loop and transition journal.

```text
eval_transition_event {
  event_id: String =>
  campaign_id: String,
  parent_id: String?,
  runtime_id: String?,
  node_id: String?,
  transition: String,
  phase: String?,
  outcome: String,
  evidence_class: String,
  record_ref: String?,
  source_stream_id: String?,
  source_event_index: Int?,
  recorded_at: String,
  ingested_at: String?
}
```

### `eval_trace_event`

Structured observability events from `observe::Step`, `observe::TransitionBuilder`, command-output helpers, and the optional Prototype 1 observation JSONL sink.

```text
eval_trace_event {
  trace_event_id: String =>
  campaign_id: String?,
  parent_id: String?,
  runtime_id: String?,
  node_id: String?,
  generation: Int?,
  branch_id: String?,
  role: String?,
  pipeline: String?,
  stage: String?,
  authority: String?,
  transition: String?,
  event_name: String?,
  span_name: String?,
  target: String,
  level: String,
  outcome: String?,
  duration_ms: Int?,
  record_access: String?,
  record_kind: String?,
  record_path: String?,
  record_index: Int?,
  record_count: Int?,
  program: String?,
  exit_code: Int?,
  error: String?,
  source_log_ref: String?,
  source_event_index: Int?,
  recorded_at: String?
}
```

This relation is the answer to “None beyond trace logs”: trace logs are persisted evidence and should be queryable.

### `eval_candidate_event`

Append candidate/child status changes derived from typed transitions or admitted channel results instead of relying on mutable `node.json` projections.

```text
eval_candidate_event {
  event_id: String =>
  campaign_id: String,
  node_id: String,
  child_id: String?,
  runtime_id: String?,
  prev_status: String?,
  next_status: String,
  reason: String?,
  evidence_class: String,
  source_ref: String?,
  recorded_at: String
}
```

### `eval_channel_message`

Evidence mirror of a channel envelope. This is not the transport authority itself.

```text
eval_channel_message {
  message_id: String =>
  channel_id: String,
  campaign_id: String,
  node_id: String,
  runtime_id: String,
  schema_version: String,
  direction: String,
  message_kind: String,
  body_sha256: String,
  payload_ref: String?,
  source_cursor: String?,
  source_ref: String?,
  recorded_at: String?
}
```

### `eval_channel_receipt`

Runtime observation of a channel message with validation status.

```text
eval_channel_receipt {
  receipt_id: String =>
  channel_id: String,
  message_id: String,
  campaign_id: String,
  node_id: String,
  runtime_id: String,
  observed_by: String?,           -- runtime_id
  direction: String,
  validation_status: String,
  imported_ref: String?,
  observed_at: String
}
```

### `eval_message_box_event`

Lock/unlock facts for message boxes.

```text
eval_message_box_event {
  event_id: String =>
  box_id: String,
  campaign_id: String,
  parent_id: String,
  action: String,                 -- lock | unlock | replay_read | failed_lock | failed_unlock
  transition: String,
  payload_sha256: String?,
  payload_ref: String?,
  outcome: String,
  recorded_at: String
}
```

### `eval_operation`

Runtime plus target operation coordinate.

```text
eval_operation {
  operation_id: String =>
  campaign_id: String,
  generator_id: String,           -- runtime_id
  target_kind: String,            -- artifact | patch_set | artifact_set
  target_ref: String,
  procedure_id: String?,
  output_artifact_id: String?,
  output_patch_id: String?,
  recorded_at: String?
}
```

### `eval_patch` and `eval_apply_event`

Patch identity and application lifecycle.

```text
eval_patch {
  patch_id: String =>
  campaign_id: String,
  base_artifact_id: String?,
  creator_id: String?,            -- runtime_id
  tool_call_id: String?,
  target_relpath: String?,
  patch_ref: String?,
  content_sha256: String?,
  status: String?
}

eval_apply_event {
  apply_id: String =>
  campaign_id: String,
  patch_id: String,
  runtime_id: String?,
  artifact_id: String?,
  outcome: String,
  output_artifact_id: String?,
  recorded_at: String
}
```

### `eval_build_event`

Build/check outcomes and binary promotion evidence.

```text
eval_build_event {
  build_id: String =>
  campaign_id: String,
  node_id: String,
  runtime_id: String?,
  artifact_id: String?,
  phase: String,                  -- check | build | promote
  outcome: String,
  binary_ref: String?,
  log_ref: String?,
  recorded_at: String
}
```

### Validation, retrieval, and refresh relations

These are follow-on code-graph-backed workload relations, not first-pass storage-migration requirements. Full join semantics are in [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md).

```text
eval_validation_event {
  validation_id: String =>
  campaign_id: String,
  turn_id: String?,
  tool_call_id: String?,
  command: String,
  cwd: String?,
  manifest_path: String?,
  package: String?,
  exit_code: Int?,
  semantic_status: String,
  model_visible: Bool,
  recorded_at: String?
}

eval_validation_cover {
  validation_id: String,
  code_ref_id: String =>
  cover_kind: String,
  evidence_ref: String?
}

eval_retrieval {
  retrieval_id: String =>
  campaign_id: String,
  turn_id: String?,
  exchange_id: String?,
  query_text: String?,
  method: String,
  scope_ref: String?,
  recorded_at: String?
}

eval_retrieval_hit {
  retrieval_id: String,
  rank: Int =>
  code_ref_id: String?,
  score: Float?,
  snippet_hash: String?,
  visible_to_model: Bool,
  reason: String?
}

eval_refresh_event {
  refresh_id: String =>
  campaign_id: String,
  artifact_id: String?,
  trigger_id: String?,
  relpath: String?,
  owner_crate: String?,
  refreshed_crate: String?,
  snapshot_id: String?,
  status: String,
  evidence_ref: String?,
  recorded_at: String?
}
```

### Candidate and child-plan relations

Generation-local candidate sets and membership order are required for selection audit.

```text
eval_candidate_set {
  set_id: String =>
  campaign_id: String,
  parent_id: String,
  generation: Int,
  source: String,                 -- child_plan | rejected_only | recovery
  order_hash: String?,
  set_root: String?,
  recorded_at: String
}

eval_candidate_member {
  member_id: String =>
  set_id: String,
  node_id: String,
  branch_id: String,
  candidate_id: String,
  planned_index: Int,
  admissible: Bool,
  reason_ref: String?
}

eval_child_plan_member {
  box_id: String,
  member_id: String =>
  node_id: String,
  request_ref: String?,
  resolved_ref: String?,
  payload_ref: String?
}
```

### Evaluation and metrics relations

Evaluation records must name evaluator, policy, eval set, compared runtime/artifact, and instance-level evidence.

```text
eval_evaluation {
  evaluation_id: String =>
  campaign_id: String,
  parent_id: String?,
  branch_id: String,
  baseline_id: String?,
  treatment_id: String?,
  procedure_id: String?,
  evaluator_id: String?,
  eval_set_id: String?,
  policy_ref: String?,
  disposition: String,
  record_ref: String?,
  recorded_at: String?
}

eval_evaluation_instance {
  evaluation_id: String,
  instance_id: String =>
  baseline_run_id: String?,
  treatment_run_id: String?,
  baseline_ref: String?,
  treatment_ref: String?,
  status: String,
  outcome: String?,
  oracle_ref: String?
}

eval_metric_observation {
  metric_id: String =>
  evaluation_id: String?,
  candidate_id: String?,
  metric: String,
  value_json: String,
  direction: String?,
  evidence_ref: String?
}

eval_oracle_result {
  oracle_result_id: String =>
  evaluation_id: String,
  instance_id: String?,
  oracle_id: String,
  outcome: String,
  evidence_ref: String?,
  recorded_at: String?
}
```

### Selection and continuation relations

`eval_selection_event` from the first draft is split into decision, candidate, finding, score, and continuation rows.

```text
eval_selection_decision {
  decision_id: String =>
  campaign_id: String,
  parent_id: String,
  set_id: String,
  procedure_id: String,
  selected_node_id: String?,
  selected_artifact_id: String?,
  outcome: String,                -- accepted | explore_from | stop
  disposition: String?,
  decision_ref: String?,
  decision_hash: String?,
  recorded_at: String
}

eval_selection_candidate {
  decision_id: String,
  member_id: String =>
  node_id: String,
  branch_id: String,
  selectable: Bool,
  selected: Bool,
  exclusion_ref: String?
}

eval_selection_finding {
  finding_id: String =>
  decision_id: String,
  member_id: String?,
  domain: String,                 -- operational | protocol | patch | oracle | adjudication
  verdict: String,
  confidence: String,
  evidence_ref: String?,
  rationale_ref: String?
}

eval_selection_score {
  decision_id: String,
  member_id: String =>
  formula_id: String,
  score_json: String,
  weight: Float?,
  rank: Int?,
  selected: Bool
}

eval_continuation_decision {
  decision_id: String =>
  campaign_id: String,
  parent_id: String,
  disposition: String,
  selected_branch_id: String?,
  next_generation: Int,
  total_nodes: Int,
  policy_ref: String?,
  recorded_at: String
}
```

### `eval_import_event`

Explicit cross-runtime movement of child/successor/treatment evidence into a parent-owned store.

```text
eval_import_event {
  import_id: String =>
  campaign_id: String,
  importer_id: String,
  source_runtime_id: String?,
  source_scope: String,
  target_scope: String,
  evidence_ref: String,
  receipt_id: String?,
  validation_status: String,
  imported_at: String
}
```

Parent-side visibility should be based on imports/receipts, not shared paths.

### Agent-turn timeline relations

One agent turn needs an ordered timeline. The turn-level event spine is the local ordering authority inside the turn.

```text
eval_agent_turn_event {
  event_id: String =>
  turn_id: String,
  source_event_index: Int,
  occurred_at: String?,
  trace_scope_id: String?,
  parent_scope_id: String?,
  source_component: String,
  observation_class: String,      -- runtime | passive | adapter | diagnostic | compatibility
  event_kind: String,             -- model | tool | edit | adapter | message | turn | cost
  payload_ref: String?
}

eval_model_exchange {
  exchange_id: String =>
  turn_id: String,
  source_event_index: Int,
  request_ref: String?,
  response_ref: String?,
  assistant_msg_id: String?,
  model_id: String?,
  provider: String?,
  route: String?,
  params_ref: String?,
  tools_ref: String?,
  tool_choice: String?,
  finish_reason: String?,
  latency_ms: Int?,
  cost_ref: String?
}

eval_tool_event {
  tool_event_id: String =>
  turn_id: String,
  tool_call_id: String,
  exchange_id: String?,
  source_event_index: Int,
  tool_name: String,
  status: String,
  args_ref: String?,
  result_ref: String?,
  model_visible: Bool?,
  recorded_at: String?
}

eval_edit_event {
  edit_event_id: String =>
  turn_id: String,
  proposal_id: String,
  tool_call_id: String?,
  patch_id: String?,
  target_ref: String?,
  state: String,                  -- proposed | staged | applied | rejected | denied | failed
  policy_ref: String?,
  diff_ref: String?,
  recorded_at: String?
}

eval_adapter_event {
  adapter_event_id: String =>
  turn_id: String,
  attempt_id: String?,
  observation_class: String,
  summary_ref: String?,
  terminal_status: String?,
  recorded_at: String?
}

eval_message_event {
  message_event_id: String =>
  turn_id: String,
  message_id: String,
  role: String,
  source_event_index: Int,
  content_ref: String?,
  recorded_at: String?
}

eval_cost_event {
  cost_event_id: String =>
  campaign_id: String,
  turn_id: String?,
  exchange_id: String?,
  procedure_id: String?,
  model_id: String?,
  provider: String?,
  input_tokens: Int?,
  output_tokens: Int?,
  cost_units: Float?,
  latency_ms: Int?,
  recorded_at: String?
}
```

The model request ref should preserve `ChatCompRequest`-level facts, including request parameters, tool schemas, tool choice, and messages. A messages-only request tap is insufficient for replay/debug queries.

### `eval_evidence_warning`

Missing/degraded joins should be queryable rather than silently filled from filenames.

```text
eval_evidence_warning {
  warning_id: String =>
  campaign_id: String?,
  subject_ref: String,
  warning_kind: String,
  missing_kind: String?,
  detail: String,
  recorded_at: String?
}
```

## Evidence/ref sketches

### `eval_record_ref`

Small typed records and projections.

```text
eval_record_ref {
  record_ref_id: String =>
  campaign_id: String,
  family: String,
  schema_version: String,
  store_scope: String,
  producer_role: String,
  producer_id: String?,           -- runtime_id when known
  source_class: String,
  evidence_class: String,
  source_ref: String?,
  content_sha256: String,
  payload_json: String?,
  recorded_at: String?,
  ingested_at: String?
}
```

This supersedes the current passive `prototype1_record` mirror by adding ownership, source, and evidence context.

### `eval_log_ref`

Log/blob references for large or sensitive evidence.

```text
eval_log_ref {
  log_ref_id: String =>
  campaign_id: String?,
  runtime_id: String?,
  store_scope: String,
  log_kind: String,               -- main_trace | observation_jsonl | full_response | stdout | stderr | execution_log
  source_ref: String,
  byte_start: Int?,
  byte_len: Int?,
  content_sha256: String?,
  sensitivity: String?,
  recorded_at: String?
}
```

### `eval_snapshot_ref`

Run-local DB snapshots and checkpoint refs.

```text
eval_snapshot_ref {
  snapshot_ref_id: String =>
  campaign_id: String,
  run_id: String?,
  runtime_id: String?,
  store_scope: String,
  snapshot_kind: String,          -- indexing_checkpoint | indexing_failure | final_snapshot | starting_cache
  source_ref: String,
  content_sha256: String?,
  db_identity: String?,
  recorded_at: String?
}
```

Snapshots usually remain child/treatment-local. Parent imports summaries or selected refs, not raw code graph facts by default.

### Closure, protocol, artifact, and code refs

```text
eval_closure_ref {
  closure_ref_id: String =>
  campaign_id: String,
  run_id: String?,
  store_scope: String,
  source_ref: String,
  content_sha256: String?,
  summary_json: String?,
  recorded_at: String?
}

eval_protocol_ref {
  protocol_ref_id: String =>
  campaign_id: String,
  run_id: String?,
  procedure_id: String?,
  source_ref: String,
  content_sha256: String?,
  summary_json: String?,
  recorded_at: String?
}

eval_artifact_ref {
  artifact_ref_id: String =>
  campaign_id: String,
  artifact_id: String?,
  kind: String,
  source_ref: String,
  content_sha256: String?,
  recorded_at: String?
}

eval_code_snapshot {
  snapshot_id: String =>
  campaign_id: String?,
  artifact_id: String?,
  store_scope: String,
  namespace: String?,
  crate_id: String?,
  root_path: String?,
  git_commit: String?,
  code_validity: String?,
  schema_version: String?,
  created_at: String?
}

eval_code_ref {
  code_ref_id: String =>
  campaign_id: String,
  artifact_id: String?,
  snapshot_id: String?,
  code_node_id: Uuid?,
  node_kind: String?,
  crate_id: String?,
  namespace: String?,
  canonical_path: [String]?,
  relpath: String?,
  span: [Int; 2]?,
  node_hash: String?,
  file_hash: String?,
  resolution: String,
  source_ref: String?,
  recorded_at: String?
}

eval_code_link {
  subject_kind: String,
  subject_id: String,
  code_ref_id: String =>
  ref_role: String,              -- cited | retrieved | edited | proposed | applied | validated | failed | protected | selected | denied
  event_id: String?,
  confidence: Float?,
  evidence_ref: String?,
  recorded_at: String?
}

eval_code_touch {
  touch_id: String =>
  campaign_id: String,
  patch_id: String?,
  apply_id: String?,
  code_ref_id: String?,
  touch_kind: String,
  relpath: String,
  pre_hash: String?,
  post_hash: String?,
  hunk_ref: String?,
  recorded_at: String?
}
```

See [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md) for the join semantics and snapshot-scope guardrails. `code_node_id` means a parsed code graph UUID; current Prototype 1 `node_id` strings remain candidate/work-item join keys.

## Policy/edit-surface overlay sketches

The edit surface needs to be queryable over code graph nodes without mutating base ingest facts.

```text
eval_policy_surface {
  surface_id: String =>
  campaign_id: String,
  parent_id: String?,
  artifact_id: String?,
  policy_id: String,
  surface_hash: String?,
  store_scope: String,
  created_at: String
}

eval_code_label {
  surface_id: String,
  code_node_id: String =>
  artifact_id: String,
  label: String,                  -- immutable | mutable | protected | allowed_edit
  reason_ref: String?,
  recorded_at: String
}

eval_policy_grant {
  grant_id: String =>
  surface_id: String,
  grantee_role: String,           -- parent | child | successor | harness
  operation: String,
  constraint_ref: String?,
  recorded_at: String
}
```

Successor rule: a selected successor must inherit, recompute, or validate the admitted policy surface under the selected artifact. Do not rely on parent-only in-memory policy state.

## Query shapes

### Parent epoch lifetime

Inputs: `parent_id`.

Join:

```text
eval_parent_epoch
-> eval_lineage
-> eval_profile_commitment
-> eval_transition_event / eval_trace_event where parent_id
-> eval_message_box_event where parent_id
-> eval_candidate_set / eval_candidate_member where parent_id
-> eval_child where parent_id
-> eval_evaluation where parent_id
-> eval_selection_decision where parent_id
-> eval_continuation_decision where parent_id
-> eval_successor where predecessor_id = parent_id
-> eval_history_ref through selected successor / sealed block
```

Result: startup evidence, admitted profile, child plan, fanout, child outcomes, evaluation, selection, handoff, completion, and which evidence was sealed vs merely projected.

### Child candidate lifetime

Inputs: `child_id` or `node_id`.

Join:

```text
eval_child
-> eval_candidate_member
-> eval_attempt where node_id
-> eval_invocation where attempt/runtime
-> eval_channel where attempt/runtime
-> eval_channel_message / eval_channel_receipt where channel_id
-> eval_candidate_event where node_id
-> eval_build_event / eval_binary_ref
-> eval_run / eval_evaluation where node_id/branch_id
-> eval_record_ref / eval_log_ref / eval_snapshot_ref by attempt/run scope
```

Result: planned payload, materialization/build/spawn events, ready/evaluating/result messages, treatment refs, failure cause, and comparison outcome.

Added audit question: was failure derived from channel evidence, process exit, timeout, or only missing compatibility projections?

### Runtime attempt playback

Inputs: `attempt_id` or `runtime_id`.

Join:

```text
eval_attempt
-> eval_runtime
-> eval_invocation
-> eval_channel / eval_channel_receipt
-> eval_log_ref for streams and tracing
-> eval_trace_event by runtime_id
-> eval_record_ref by producer_id
```

Result: one concrete process attempt with bootstrap, protocol messages, streams, logs, and terminal evidence.

### Operation / artifact provenance

Inputs: `artifact_id`, `patch_id`, or `operation_id`.

Join:

```text
eval_operation
-> eval_runtime as generator
-> eval_artifact as target/output
-> eval_patch
-> eval_apply_event
-> eval_artifact_surface
-> eval_binary_ref when built/spawned
```

Result: which runtime operated over which artifact/patch set, what patch/artifact it produced, and what binary/artifact hydrated later attempts.

### Selection audit

Inputs: `decision_id` or `parent_id`.

Join:

```text
eval_selection_decision
-> eval_candidate_set / eval_candidate_member
-> eval_selection_candidate
-> eval_selection_finding
-> eval_selection_score
-> eval_evaluation / eval_evaluation_instance
-> eval_oracle_result / eval_metric_observation
-> eval_profile_commitment / eval_policy_config
-> eval_history_ref if sealed
```

Result: candidate set, score/formula evidence, hard gates, oracle/eval evidence, rationale, continuation disposition, and sealed admission status.

### Agent/tool-loop trace

Inputs: `run_id`, `turn_id`, `node_id`, or `tool_call_id`.

Join:

```text
eval_run
-> eval_agent_turn
-> eval_agent_turn_event ordered by source_event_index
-> eval_model_exchange
-> eval_tool_event
-> eval_edit_event
-> eval_adapter_event
-> eval_message_event
-> eval_cost_event
-> eval_record_ref / eval_log_ref for sidecars
-> eval_code_ref / eval_code_label for cited or edited code items
```

Result: prompt/request/response/tool/edit timeline, including model request parameters, tool schemas, tool choice, costs, patch proposals, and code graph touch points.

### Code graph join audit

Inputs: `code_node_id`, `code_ref_id`, `patch_id`, `validation_id`, `selection_id`, or `relpath`.

Join:

```text
code graph relation row at snapshot/as-of
-> eval_code_ref
-> eval_code_link by subject_kind/subject_id/ref_role
-> eval_code_touch for patch/apply effects
-> eval_validation_event / coverage rows
-> eval_retrieval / retrieval hits
-> eval_selection_decision / eval_review_finding when applicable
```

Result: which parsed Ploke code items were retrieved, cited, edited, validated, selected, rejected, protected, or associated with failures, without mutating base code graph facts.

### Remote import audit

Inputs: `campaign_id`, `parent_id`, or `channel_id`.

Join:

```text
eval_channel_message
-> eval_channel_receipt
-> eval_import_event
-> eval_record_ref / eval_log_ref / eval_artifact_ref / eval_snapshot_ref
```

Result: exactly which child-local facts became parent-visible, under what validation, and which child-local facts remained unimported.

## First implementation candidates

First pass goal: configurable file-or-db storage for ordinary eval evidence while preserving current filesystem output and authority boundaries. Do not require code-graph overlay relations in this pass.

1. Define common enums/columns: store scope, visibility, source class, evidence class, validation status, and causal order fields.
2. Ingest `eval_trace_event` from `observe.rs` / observation JSONL while preserving current structured fields.
3. Add `eval_attempt`, `eval_invocation`, `eval_channel_message`, and `eval_channel_receipt` as evidence mirrors only, not transport authority.
4. Add `eval_record_ref` for legacy node/request/result records, split by child-local writes and parent-imported terminal facts.
5. Add `eval_candidate_event` from typed transition/channel outcomes; raw `node.json` status can be a compatibility source, not the authority.
6. Add `eval_evaluation` and `eval_selection_decision` summary rows after parent comparison.
7. Add `eval_log_ref` for child/successor stdout/stderr and runner execution logs.

## Follow-on code graph overlay candidates

These are intentionally separated from the first file-or-db migration pass. See [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md).

1. Add `eval_code_snapshot`, richer `eval_code_ref`, and `eval_code_link` for parent-code-graph joins once core DB writes are stable.
2. Add `eval_code_touch`, `eval_validation_event` / validation coverage, retrieval-hit, and refresh-event rows as code-graph-backed workload slices.
3. Add policy/code overlays after record, trace, and code-ref lanes are stable.

## Non-goals for this model

- Do not replace sealed History block storage.
- Do not make ordinary DB rows the parent/child channel.
- Do not make ordinary DB rows the child-plan MessageBox authority unless a typed box backing is implemented.
- Do not merge child/treatment code graph facts into the parent graph by default.
- Do not store full LLM payloads inline without an explicit sensitivity/redaction policy.
- Do not use timestamps as the primary ordering spine.
- Do not treat worktree paths, branch names, or local DB handles as semantic identity without artifact/runtime refs.
