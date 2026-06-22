# Prototype 1 Relational Database Planning Notes

Status: active planning note / logical-to-physical schema review.

Related files:

- [`relational-data-model.md`](relational-data-model.md)
- [`cozo-feature-map.md`](cozo-feature-map.md)
- [`codegraph-eval-join-model.md`](codegraph-eval-join-model.md)
- [`debugging-and-research-query-workloads.md`](debugging-and-research-query-workloads.md)
- [`record-usefulness-triage.md`](record-usefulness-triage.md)
- [`storage-plan.md`](storage-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)

## Purpose

This note reviews the relational model from a data-structuring and database-planning perspective: relation boundaries, keys, normalization, append semantics, Cozo implementation shape, and first implementation slices.

The goal is not to finalize DDL yet. The goal is to keep the logical model implementable without turning legacy files into schema, over-normalizing too early, or weakening authority semantics.

## Working assumptions

Scope clarification: this note is about the **database schema used by `EvalStore`/eval-query surfaces**, not the full set of Prototype 1 persistence ports. It may include refs or mirror facts about History, channels, message boxes, artifacts, logs, snapshots, and bootstrap files, but those rows do not implement those domains' authority. The broader port map is tracked in [`persistence-port-map.md`](persistence-port-map.md).

- The first database backend targets ordinary eval evidence/projections, not sealed History, live Channel transport, MessageBox authority, or artifact checkout mutation.
- Cozo relations are best used as typed facts plus derived query views. They do not enforce foreign keys for us; writer/import validation must enforce referential integrity.
- The DB should preserve causal order explicitly. Timestamps are audit/profiling facts, not the primary ordering spine.
- Every normalized row should still be traceable back to source evidence: a typed transition, channel receipt, record ref, log ref, sealed History ref, or explicit import.
- Legacy records can be retained as `eval_record_ref` rows without becoming the semantic schema.

## Recommended relation tiers

### Tier 0 — raw/ref evidence lane

These are the safest first rows because they preserve current evidence without claiming new authority.

```text
eval_record_ref
eval_log_ref
eval_blob_ref
eval_snapshot_ref
eval_artifact_ref
```

Use these for raw JSON/TOML/JSONL/log/blob references, hashes, schemas, source path/URI, and sensitivity labels.

Do not overload these rows with domain semantics. A `record_ref` can say “there is a `RunnerResult` JSON at this hash,” but selection queries should eventually join to typed attempt/evaluation/selection rows.

### Tier 1 — runtime/evidence spine

These are stable join anchors used by many query families.

```text
eval_campaign
eval_lineage
eval_parent_epoch
eval_runtime
eval_attempt
eval_profile_commitment
```

This tier should be small and mostly identity/lifetime oriented. It is where owner scope, producer role, and runtime identity become queryable without inspecting paths.

### Tier 2 — append/event lane

Append facts reconstruct trajectories.

```text
eval_transition_event
eval_trace_event
eval_candidate_event
eval_channel_message
eval_channel_receipt
eval_message_box_event
eval_import_event
eval_evidence_warning
```

Prefer append rows over mutable “current status” tables. If a current view is useful, derive it from events or materialize it separately as a projection.

### Tier 3 — domain facts

These are normalized enough to answer actual loop questions.

```text
eval_child
eval_candidate_set
eval_candidate_member
eval_evaluation
eval_evaluation_instance
eval_selection_decision
eval_selection_candidate
eval_selection_finding
eval_selection_score
eval_operation
eval_patch
eval_apply_event
eval_build_event
```

This tier should not be added wholesale. Add narrow subsets only when a real query or writer needs them.

### Tier 4 — agent-turn drilldown

Agent-turn facts are important, but they are a deeper nested timeline. They should follow once the runtime/evidence spine can identify `run_id`, `turn_id`, and `runtime_id` reliably.

```text
eval_agent_turn
eval_agent_turn_event
eval_model_exchange
eval_tool_event
eval_edit_event
eval_adapter_event
eval_message_event
eval_cost_event
```

## Key strategy

### Prefer semantic ids over path ids

Paths are source refs, not identity. Relation keys should be semantic when available:

- `campaign_id`
- `lineage_id`
- `parent_id`
- `runtime_id`
- `attempt_id`
- `channel_id`
- `message_id`
- `turn_id`
- `decision_id`
- `evaluation_id`
- `artifact_id`
- `patch_id`

A source path should usually be a value column or a linked `*_ref`, not the primary identity.

### Keep `attempt_id` distinct from `runtime_id`

For the first implementation, `attempt_id` may equal `runtime_id`. Conceptually, keep both names:

- `runtime_id`: concrete process identity;
- `attempt_id`: role-scoped execution attempt over a node/artifact/invocation.

This gives us room for restarts, retries, replay imports, and degraded provenance without rewriting the schema.

### Treat current `node_id` as compatibility join key

The current Prototype 1 `node_id` remains useful because existing files, channels, invocations, and reports refer to it. But do not model `scheduler.json` as the semantic owner of that id.

Use `node_id` as a join column on semantic relations such as `eval_child`, `eval_attempt`, `eval_candidate_member`, `eval_invocation`, and `eval_channel_*`.

### Deterministic event ids for imports

For imported file/JSONL evidence, prefer deterministic ids where possible:

```text
sha256(source_stream_id || source_event_index || content_sha256)
```

This supports idempotent backfill and dual-write comparison. For live writes, generated UUIDs are fine if the writer also records source stream/cursor/hash.

## Cozo-specific planning notes

### Relation keys should match uniqueness, not every query

Cozo relations have explicit key fields. Choose keys that make a row unique and stable. Do not try to encode every query path into one primary relation.

Examples:

```text
eval_channel_message { message_id => ... }
eval_trace_event { trace_event_id => ... }
eval_candidate_member { member_id => ... }
eval_selection_candidate { decision_id, member_id => ... }
```

For frequent alternate lookups, add small edge/index relations rather than distorting the main relation key.

Possible lookup relations:

```text
eval_parent_runtime { parent_id, runtime_id => role }
eval_node_attempt { node_id, attempt_id => runtime_id, role }
eval_runtime_attempt { runtime_id, attempt_id => node_id?, parent_id? }
eval_turn_run { run_id, turn_id => runtime_id?, node_id? }
eval_subject_ref { subject_kind, subject_id, ref_id => ref_kind }
```

Follow-on code graph overlays may add:

```text
eval_code_link { subject_kind, subject_id, code_ref_id => ref_role }
```

For those later code graph joins, prefer a generic subject-to-code bridge (`eval_code_link`) plus a scoped `eval_code_ref` over adding nullable `code_node_id` columns to every eval relation.

### Do not use Cozo time-travel as the first event model

Existing `ploke-db` observability code uses `Validity` for mutable UI-ish state. Prototype 1 evidence should initially use explicit event order fields instead:

```text
source_stream_id
source_event_index
source_cursor
source_line
recorded_at
ingested_at
```

Cozo validity can be useful later for materialized current projections, but it should not replace causal ordering in the evidence schema.

### Use `Json` sparingly

`Json` payload columns are useful for:

- small compatibility payloads;
- details not yet worth normalizing;
- lossless migration during dual-write.

But primary query dimensions should be typed columns. If every important fact is inside `payload_json`, Cozo cannot give us the query clarity we want.

## Normalization guidance

### Normalize when the field is a join key or policy/audit dimension

Good normalization candidates:

- runtime, attempt, parent, candidate, decision ids;
- channel message id/body hash/validation status;
- evaluation procedure/evaluator/eval-set/policy;
- selection outcome, candidate membership, selected flag;
- artifact/patch ids;
- turn/exchange/tool/proposal ids;
- evidence class/source class/visibility.

### Keep as refs when the payload is large, sensitive, or rarely queried

Good ref-only candidates:

- full LLM responses;
- stdout/stderr streams;
- compressed run records;
- DB snapshots;
- full protocol artifacts;
- large runner logs;
- raw branch registry snapshots.

### Avoid one universal event table

A universal `eval_event` table would be tempting but would become stringly and nullable. Prefer a small shared envelope shape in Rust plus typed relations per event family.

A common envelope can exist in code/docs:

```text
campaign_id
store_scope
producer_role
source_class
evidence_class
validation_status
source_stream_id
source_event_index
recorded_at
ingested_at
```

But each relation should still expose its own queryable columns.

## Suggested physical schema slices

The first slice should prove DB writes, idempotent insertion, source refs, and useful queries without changing authority.

### Slice 0 — fixed first production writer: parent-start/resource evidence

```text
eval_transition_event     -- semantic `R4c -> R5` parent-start/resource fact
eval_runtime              -- only if the writer can name a runtime cheaply
eval_record_ref           -- optional source/ref/hash backpointer to existing filesystem evidence
eval_trace_event          -- optional only if structured observe fields are captured in the same slice
```

Why:

- parent-owned and non-authority;
- no provider/API cost;
- small blast radius;
- exercises config, receipts, deterministic ids/hashes, DB insert/query, and dual-strict comparison.

Queries enabled:

- “did this parent start under the admitted campaign/profile?”
- “which source evidence supports parent-start/resource observation?”
- “can filesystem and DB forms be compared deterministically?”

First-slice contract to write before coding:

- **Writer:** `R4c -> R5` only.
- **Current filesystem evidence:** `JournalEntry::ParentStarted` plus the parent-start resource sample appended by `append_parent_target_sample`.
- **Authority class:** evidence/projection only; startup readiness remains decided before this writer.
- **Method shape:** one parent-start-specific `EvalStore` method/envelope, not a generic record emitter.
- **DB rows:** start with `eval_transition_event`; add `eval_record_ref`, `eval_runtime`, or `eval_trace_event` only if the envelope can populate their required fields without guessing.
- **Required pre-code decisions:** deterministic id inputs, semantic envelope/hash fields, minimal common-axis enum values, duplicate/idempotency behavior, and dual-strict write-order/failure behavior.

### Slice A — trace/log/reference evidence

```text
eval_log_ref
eval_trace_event
eval_record_ref
```

Why:

- structured observation JSONL and `observe.rs` fields are already telemetry, not authority;
- logs need hash/ref handling before deeper ingestion;
- `eval_record_ref` can supersede passive mirror limitations without normalizing every record.

Queries enabled:

- “what did this runtime emit?”
- “which transitions failed or timed out?”
- “which records/logs support this parent/child step?”

### Slice B — attempt/channel evidence mirrors

```text
eval_attempt
eval_invocation
eval_channel_message
eval_channel_receipt
eval_import_event
```

Why:

- it preserves the channel-as-authority boundary while making parent-visible receipts queryable;
- it gives remote-runtime import/audit queries a spine;
- it separates attempt-scoped evidence from mutable node/latest-result projections.

Queries enabled:

- “which messages were observed for this runtime?”
- “what evidence crossed from child-local to parent-visible?”
- “did parent observe terminal result through channel or only through projections?”

### Slice C — selection/evaluation summaries

```text
eval_candidate_set
eval_candidate_member
eval_child
eval_evaluation
eval_evaluation_instance
eval_selection_decision
eval_selection_candidate
eval_continuation_decision
```

Why:

- this is where parent selection and baseline promotion become understandable;
- it replaces legacy scheduler/branch projection dependence with explicit candidate/decision facts.

Queries enabled:

- “which candidates were considered?”
- “why was this candidate selected or rejected?”
- “which evaluation evidence supported the successor decision?”

### Slice D — operation/artifact provenance

```text
eval_artifact
eval_artifact_surface
eval_operation
eval_patch
eval_apply_event
eval_binary_ref
eval_build_event
```

Why:

- this is needed to answer artifact/runtime provenance questions;
- it preserves the coordinates a later code graph overlay can join against without making that overlay part of the first pass.

## Schema risk notes

### Too many nullable columns

Some relation sketches are intentionally broad. When implementing, prefer splitting relation families if more than half the columns are usually null.

Example: keep `eval_channel_message` and `eval_channel_receipt` separate instead of one row with optional observed/imported fields.

### Duplicate parent/child/successor modeling

`eval_successor` is a derived/convenience projection over:

- selected child/candidate;
- sealed History ref;
- successor attempt/runtime;
- successor ready/completion receipts.

It is fine as a materialized convenience relation later, but do not make it a base source of handoff truth in the first storage pass.

### Legacy projection leakage

Do not promote these directly into first-class schema:

- `scheduler.json`;
- latest `runner-result.json`;
- passive mirror rows;
- monitor pointers;
- old loop-controller traces.

They can be retained as refs or compatibility imports while semantic rows are built from typed transitions, channel receipts, evaluations, and selection decisions.

## Validation requirements

Because Cozo will not enforce all semantic constraints, writer/import code should validate:

- referenced campaign/runtime/attempt exists when expected;
- channel envelope body hash matches payload;
- imported evidence source scope, target scope, visibility, source class, and evidence class are explicit;
- child-local evidence is not marked parent-visible without import/receipt;
- event ordering fields are present for JSONL/channel/journal imports;
- source payload hash matches raw bytes;
- rows from legacy projections are marked compatibility/projection evidence.

Tests should include idempotent re-import of the same JSONL/log/record source.

## Immediate doc follow-ups

- Keep this note as the physical-planning companion; `relational-data-model.md` remains the canonical logical model.
- Use `debugging-and-research-query-workloads.md` to prioritize first queries before adding new normalized relations.
- Keep `codegraph-eval-join-model.md` as follow-on guidance; do not add code-graph overlay columns/relations to the first file-or-db migration pass.
- Decide first event id scheme for observation JSONL import.
- Define minimal first-slice enum values for `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status` before any DB writer lands.
