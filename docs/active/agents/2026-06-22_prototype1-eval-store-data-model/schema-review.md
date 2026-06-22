# Prototype 1 Relational Data Model Review

Status: source-checked review / folded into [`relational-data-model.md`](relational-data-model.md).

Reviewed file: [`relational-data-model.md`](relational-data-model.md).

Folded: the recommended relation additions, common-column changes, and query updates from this review were incorporated into `relational-data-model.md` after user approval.

Cross-referenced sources:

- [`storage-plan.md`](storage-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`crates/ploke-eval/docs/orientation/source-of-truth.md`](../../../../crates/ploke-eval/docs/orientation/source-of-truth.md)
- [`crates/ploke-eval/docs/prototype1/typestate.md`](../../../../crates/ploke-eval/docs/prototype1/typestate.md)
- [`crates/ploke-eval/docs/prototype1/typestate-invariants.md`](../../../../crates/ploke-eval/docs/prototype1/typestate-invariants.md)
- [`crates/ploke-eval/docs/prototype1/operator-map.md`](../../../../crates/ploke-eval/docs/prototype1/operator-map.md)
- [`crates/ploke-eval/docs/prototype1/phase-map.md`](../../../../crates/ploke-eval/docs/prototype1/phase-map.md)
- [`crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md`](../../../../crates/ploke-eval/docs/prototype1/persistence-inventory-and-cozo-map.md)
- [`crates/ploke-eval/docs/reference/records.md`](../../../../crates/ploke-eval/docs/reference/records.md)
- [`crates/ploke-eval/docs/reference/schemas.md`](../../../../crates/ploke-eval/docs/reference/schemas.md)
- [`docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`](../../../workflow/evalnomicon/src/prototype1/runtime-loop.md)
- [`docs/workflow/evalnomicon/src/prototype1/artifact-runtime-model.md`](../../../workflow/evalnomicon/src/prototype1/artifact-runtime-model.md)
- [`docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`](../../../workflow/evalnomicon/src/prototype1/runtime-authority.md)
- [`docs/workflow/evalnomicon/src/prototype1/history-crown.md`](../../../workflow/evalnomicon/src/prototype1/history-crown.md)
- [`docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`](../../../workflow/evalnomicon/src/prototype1/invariant-ledger.md)
- [`docs/workflow/evalnomicon/src/prototype1/persistence-and-observability.md`](../../../workflow/evalnomicon/src/prototype1/persistence-and-observability.md)
- [`docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md`](../../../workflow/evalnomicon/src/prototype1/selection-and-evaluation.md)
- [`docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`](../../../workflow/evalnomicon/drafts/runtime/parent-child-channel.md)
- [`docs/workflow/evalnomicon/drafts/observability/runtime-playback/README.md`](../../../workflow/evalnomicon/drafts/observability/runtime-playback/README.md)
- [`docs/workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md`](../../../workflow/evalnomicon/drafts/observability/runtime-playback/agent-turn.md)
- `crates/ploke-eval/src/loop_graph.rs`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- `crates/ploke-eval/src/cli/prototype1_state/observe.rs`
- `crates/ploke-records/src/{scheduler,branch,evaluation,journal,history,invocation,selection}.rs`

## Bottom line

The draft data model has the right high-level split:

1. entity relations for long-lived objects;
2. event/fact relations for append-like trajectory reconstruction;
3. evidence/ref relations for files, blobs, logs, snapshots, and imported payloads.

It also correctly preserves the major authority boundaries: History/Block, Channel/Transport, MessageBox lock/unlock, artifact/backend state, and ordinary eval evidence.

The main issue is that the current schema is still too centered on visible filesystem artifacts and role names. The loop docs and source records need a few more first-class coordinates before the model can support robust relational queries:

- lineage and parent epoch;
- attempt/runtime occurrence;
- operation coordinate: generator runtime plus target artifact/patch set;
- candidate set and selection evidence;
- admitted profile/config/policy commitments;
- channel envelope identity and import/admission state;
- agent-turn event ordering, model-exchange, tool, edit, adapter, and cost facts;
- causal ordering fields distinct from timestamps.

## What is already appropriate

### Authority separation

The draft aligns with Evalnomicon and `ploke-eval` docs by not turning database rows into History, Channel, or MessageBox authority. This matches:

- `history-crown.md`: scheduler, reports, side tables, and database projections are not History authority;
- `runtime-authority.md`: durable records are projections of allowed transitions;
- `parent-child-channel.md`: lifecycle advancement must come from per-runtime Channel semantics, not compatibility projections;
- `persistence-and-observability.md`: typed boxes are lock/unlock/read obligations, not arbitrary persisted JSON files.

Keep this separation.

### Owner-scoped stores

The draft and storage plan correctly reject one ambient shared database. This matches the artifact/runtime model and the persistence ledger: parent, child runtime, successor runtime, treatment run, passive mirror, and imported evidence have different ownership and visibility.

### Entity/fact/ref layering

The three-layer split is useful. It lets the DB answer lifecycle questions without requiring large payloads inline. It also leaves room for filesystem/object-store/git-backed blobs while still giving Cozo stable join keys.

### Initial query families

The proposed parent lifetime, child lifetime, agent/tool trace, and remote import audit queries are useful and aligned with the runtime playback draft. They are the right read-side targets for CLI, egui, replay, and model-facing introspection.

## High-priority gaps

### 1. Add explicit lineage / parent-epoch identity

`eval_parent` has `lineage_id`, but there is no `eval_lineage` relation or parent-epoch chain. History/Crown docs make lineage the authority coordinate, not a branch/path/process.

Add a small relation such as:

```text
eval_lineage {
  lineage_id: String =>
  campaign_id: String,
  genesis_parent_id: String?,
  current_head_ref: String?,       -- projection only
  created_at: String?
}
```

Also consider naming `eval_parent` as a parent epoch rather than only a runtime row. A parent epoch should join to one or more concrete runtime attempts if restart/recovery is possible.

### 2. Add concrete attempt/runtime occurrence

`eval_runtime` exists, but child and successor queries need a stable attempt entity. Current files distinguish node-level projections from attempt-scoped runtime ids: `results/<runtime>.json`, channel endpoints, invocation, streams, ready/completion records.

Add or clarify:

```text
eval_attempt {
  attempt_id: String =>             -- often runtime_id, but not always semantically identical
  campaign_id: String,
  runtime_id: String,
  role: String,
  node_id: String?,
  parent_id: String?,
  invocation_id: String?,
  channel_id: String?,
  artifact_id: String?,
  started_at: String?,
  terminal_status: String?
}
```

This prevents per-node mutable files like `runner-result.json` from overwriting the history of multiple attempts.

### 3. Add operation / patch / apply relations

`artifact-runtime-model.md` and `loop_graph.rs` identify the important operation coordinate:

```text
OperationCoordinate = (generator Runtime, target Artifact)
```

Current `eval_artifact` and `eval_child` do not fully model patch generation, patch composition, apply attempts, or derived artifact creation. Current record DTOs carry `OperationTarget`, `PatchId`, `base_artifact_id`, `derived_artifact_id`, and generation coordinates.

Add first-class relations such as:

```text
eval_operation {
  operation_id: String =>
  campaign_id: String,
  generator_runtime_id: String,
  target_kind: String,              -- artifact | patch_set | artifact_set
  target_ref: String,
  procedure_id: String?,
  output_artifact_id: String?,
  output_patch_id: String?,
  recorded_at: String?
}

eval_patch {
  patch_id: String =>
  campaign_id: String,
  base_artifact_id: String?,
  creator_runtime_id: String?,
  source_tool_call_id: String?,
  target_relpath: String?,
  patch_ref: String?,
  content_sha256: String?,
  status: String?
}

eval_apply_event {
  apply_id: String =>
  patch_id: String,
  runtime_id: String?,
  artifact_id: String?,
  outcome: String,
  output_artifact_id: String?,
  recorded_at: String
}
```

Without these, the DB can show that a child existed, but not cleanly answer which runtime generated which patch over which artifact surface.

### 4. Normalize candidate sets and child-plan membership

`eval_child` records planned children, but selection/replay needs the generation-local candidate set and membership order. Selection records use `CandidateMembershipId`, `CandidateOccurrenceId`, candidate set roots/order hashes, formula rows, and per-candidate scores.

Add:

```text
eval_candidate_set
eval_candidate_member
eval_child_plan_member
eval_selection_candidate
eval_selection_score
```

These should preserve planned order, admissibility, rejection reasons, selected flag, and links to child node/runtime/artifact/evaluation evidence.

### 5. Expand selection/evaluation beyond `eval_selection_event`

The `selection-and-evaluation.md` page and `ploke-records/src/selection.rs` require evaluator, policy, oracle/eval-set identity, hard gates, candidate findings, formula inputs, and score rows. The current `eval_selection_event` is a useful summary but too thin for audit/replay.

Add relations or structured refs for:

- evaluation procedure identity;
- evaluator identity/version;
- eval set identity and policy;
- instance-level baseline-vs-treatment comparisons;
- oracle evaluation;
- selector formula and metric set;
- per-candidate domain findings;
- continuation disposition, including stopped/explore-from-rejected cases.

Suggested relations:

```text
eval_evaluation
eval_evaluation_instance
eval_metric_observation
eval_oracle_result
eval_selection_decision
eval_selection_finding
eval_continuation_decision
```

### 6. Make admitted configuration/profile commitments first-class

The run profile owns target selection, child fanout, selection policy, execution stops, control mode, model defaults, protocol config, and storage/worktree roots. The draft has `eval_policy_surface`, but that models edit-surface policy more than admitted run configuration.

Add:

```text
eval_profile_commitment
eval_policy_config
eval_model_config
eval_protocol_config
```

At minimum, parent lifetime and selection queries should join the parent epoch to the exact admitted profile commitment and relevant policy snapshot.

### 7. Separate store ownership from visibility/admission class

The draft `owner_scope` enum currently mixes owners (`parent`, `child_runtime`, `treatment_run`) with provenance/admission classes (`imported`, `passive_mirror`, `global_log`). These should be separate axes.

Suggested split:

```text
store_scope       -- parent | child_runtime | successor_runtime | treatment_run | campaign | artifact | external
producer_role     -- parent | child | successor | eval_runner | harness | operator | provider | unknown
visibility_scope  -- local | parent_visible | successor_visible | imported | public_debug
source_class      -- direct_write | channel_payload | channel_ref | message_box_mirror | passive_mirror | log_parse | compatibility_import
evidence_class    -- sealed_history | admitted_channel | typed_transition | passive_record | diagnostic | compatibility | unverified
```

This split will make queries safer than overloading `authority_domain`.

### 8. Add causal ordering fields

Runtime playback explicitly says ordering should be causal, not timestamp/filesystem order. The relational draft has `recorded_at`, but many queries need deterministic order.

Add common optional fields:

```text
source_stream_id
source_event_index
source_line
source_cursor
runtime_sequence
parent_turn_sequence
ingested_at
```

For examples:

- transition journal: `(campaign_id, event_index)`;
- channel: `(channel_id, direction, cursor/message_id)`;
- agent turn: `(turn_id, event_index)`;
- logs: `(log_ref_id, byte_start)`;
- History: `(lineage_id, block_height, entry_index)`.

### 9. Preserve channel envelope identity

`eval_channel_receipt` should include fields from the current `Envelope<M>` shape and file transport cursor:

```text
schema_version
direction
message_id
message_kind
body_hash
recorded_at
cursor / byte offset
endpoint_ref
validation_status
```

The model should distinguish:

- channel message evidence mirror;
- parent receipt/observation;
- import/admission derived from a channel message.

Suggested split:

```text
eval_channel_message      -- evidence mirror of envelope, still not Transport authority
eval_channel_receipt      -- runtime observed message at cursor with validation result
eval_import_event         -- parent imported/admitted payload/ref from that receipt
```

### 10. Expand invocation/bootstrap relation

Current invocation records include `journal_path`, `channel_root`, embedded node/request/resolved payloads, optional `active_parent_root`, and successor `run_profile` commitment. The draft `eval_invocation` has only `bootstrap_ref` and basic ids.

Add refs/columns for:

- invocation role;
- journal ref;
- channel endpoint root/ref;
- node/request/resolved refs or inline hashes;
- active parent root / selected artifact for successors;
- run profile commitment for successors;
- launch argv/binary ref when known.

Remote execution will need the same facts in a bootstrap package even if no shared invocation path exists.

### 11. Add artifact surface and binary/build refs

`eval_artifact` should be accompanied by surface and build-product facts. History seals surface commitments; runtime docs warn that a binary from one checkout should not be treated as authority for another checkout.

Add:

```text
eval_artifact_surface     -- immutable/mutated/ambient roots or refs
eval_workspace_ref        -- local checkout/worktree handle, non-authority by itself
eval_binary_ref           -- built binary path/hash/source artifact/build runtime
eval_build_event          -- cargo check/build outcomes, stderr/stdout refs
```

This supports queries like “which binary was spawned for this runtime and which artifact built it?”

### 12. Add baseline/closure/protocol/run entities

Generation 0 baseline and later selected-child baseline promotion rely on closure state, protocol artifacts, run records, and treatment campaign evidence. `eval_run` is a start, but it does not model the closure/baseline/protocol relations explicitly.

Add or expand:

```text
eval_baseline
eval_closure_state_ref
eval_protocol_artifact_ref
eval_instance_run
eval_run_metric
```

This is important for answering “why was this selected child considered better than the parent baseline?”

### 13. Make agent-turn model event-based, not only response/tool rows

The current `eval_agent_turn`, `eval_llm_response`, and `eval_tool_call` relations are useful, but the runtime-playback agent-turn design asks for one ordered turn timeline with observation class, model exchanges, tool lifecycle, edit lifecycle, adapter observations, message flow, terminal turn facts, and cost events.

Add:

```text
eval_agent_turn_event
eval_model_exchange
eval_model_request_ref
eval_model_response_ref
eval_tool_event
eval_edit_event
eval_adapter_event
eval_message_event
eval_cost_event
```

Important fields include:

- `event_index` inside the turn;
- `observation_class`;
- `exchange_id`;
- full `ChatCompRequest` snapshot ref, not only messages;
- model id/provider/route/request params/tool choice/tool definitions;
- token usage, finish reason, latency, and cost;
- `tool_call_id` and decoded/raw arguments;
- `proposal_id`, target paths, stage/apply/reject/deny state, and policy denial facts.

### 14. Add trace fields emitted by current `observe.rs`

`eval_trace_event` should preserve structured fields already emitted by `observe::TransitionBuilder` / `observe::Step`, including:

- role;
- pipeline;
- stage/phase;
- authority;
- transition label;
- campaign_id, parent_id, node_id, generation, branch_id;
- record access/kind/path/index/count;
- outcome;
- duration;
- error;
- program/exit_code/stdout/stderr excerpts for command output events;
- parent span id or trace scope if available.

Without these, first-slice trace ingestion will throw away the exact fields needed to diagnose MessageBox/typestate behavior.

### 15. Add evidence warning / degraded fact representation

Runtime playback says missing joins should surface partial steps with evidence warnings rather than filling gaps from filenames. Add a small relation or common columns for degraded evidence:

```text
eval_evidence_warning {
  warning_id: String =>
  campaign_id: String?,
  subject_ref: String,
  warning_kind: String,
  missing_ref_kind: String?,
  detail: String,
  recorded_at: String?
}
```

This will be useful during migration from legacy filesystem evidence.

## Query assessment

### Parent lifetime query

Current query is useful, but incomplete. Add joins to:

- `eval_lineage` / History refs;
- admitted profile/config commitments;
- candidate set and child-plan membership;
- selection/continuation decision details;
- artifact install/build/surface refs;
- causal order fields.

Important added query: “Which evidence was actually sealed into the successor History block, and which evidence was only a mutable projection?”

### Child lifetime query

Current query is useful, but needs attempts. A child node can have node-level projections and one or more runtime attempts. Join via `eval_attempt`, channel envelope receipts, invocation, binary/build refs, and run/evaluation evidence.

Important added query: “Was this child considered failed because the channel reported failure, the process exited, timeout fired, or only a compatibility projection was missing?”

### Agent/tool-loop query

Current query direction is right, but `eval_llm_response` and `eval_tool_call` are too coarse. Add turn events, model exchanges, request snapshots, tool/edit events, adapter observations, and costs.

Important added query: “Which model request, including tool schemas and tool choice, led to the tool call or edit proposal that produced this patch?”

### Remote import audit query

This is one of the strongest proposed queries. It needs channel message identity, validation status, source/target store scopes, object/log/blob refs, and imported evidence class.

Important added query: “Which child-local facts were not imported, and therefore should not be visible in parent selection?”

## Recommended relation additions before implementation

Minimum additions to the draft before the first DB slice:

```text
eval_lineage
eval_attempt
eval_operation
eval_patch
eval_candidate_set
eval_candidate_member
eval_profile_commitment
eval_channel_message
eval_agent_turn_event
eval_model_exchange
eval_edit_event
eval_evaluation
eval_evaluation_instance
eval_selection_decision
eval_selection_candidate
eval_artifact_surface
eval_binary_ref
eval_evidence_warning
```

Minimum common-column changes:

```text
store_scope
visibility_scope
source_class
evidence_class
validation_status
source_stream_id
source_event_index
source_cursor
source_line
ingested_at
```

## Suggested implementation ordering adjustment

The first slice can still be `eval_trace_event`, but only if it captures the existing structured fields from `observe.rs`. Otherwise, it will prove the write path while losing query value.

This ordering is for the first file-or-db storage pass. Code graph overlay relations from `codegraph-eval-join-model.md` are follow-on work and should not block DB parity for ordinary eval evidence.

Recommended first slices:

1. Define enums/common columns: store scope, visibility/admission, evidence class, source class, validation status, and causal order keys.
2. Ingest `eval_trace_event` from `observe.rs`/observation JSONL with full structured fields.
3. Add `eval_attempt`, `eval_invocation`, `eval_channel_message`, and `eval_channel_receipt` as evidence mirrors only, not transport authority.
4. Add `eval_record_ref` for legacy node/request/result files, split by child-local and parent-imported visibility.
5. Add `eval_evaluation` / `eval_selection_decision` summary rows after parent comparison, before any History-store migration.

## Do not change

- Do not make channel message rows the channel transport.
- Do not make child-plan payload rows the MessageBox authority.
- Do not make History refs replace the sealed BlockStore.
- Do not import child/treatment code graph facts into parent scope by default.
- Do not use timestamps as the primary ordering spine.
