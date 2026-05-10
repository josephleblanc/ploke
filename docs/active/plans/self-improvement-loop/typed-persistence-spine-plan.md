# Typed Persistence Spine Plan

Updated: 2026-05-10

## Goal

Build a complete typed persistence spine for Prototype 1 loop execution.

Every JSON/JSONL shape written, transmitted, replayed, monitored, or shown in UI by the loop has a named Rust type. Production code never parses project-owned persisted JSON through `serde_json::Value`, anonymous field walking, or ad hoc JSON accessors.

The result supports forward replay, backward inspection, live monitoring, and a tree UI that can drill from a parent/child/successor node into the typed evidence, protocol outputs, patches, diffs, tool calls, database context, evaluation targets, and outcomes that produced it.

## Naming

Use this task handle:

```text
typed-persistence-spine
```

Use this lane name in handoffs and sub-agent prompts:

```text
Typed Persistence Spine
```

## Invariant

- If Ploke writes a JSON/JSONL shape, Ploke owns a `Serialize`/`Deserialize` Rust type for it.
- If a reader needs only part of a record, define a named typed projection struct or enum.
- Production readers do not use `serde_json::Value` to parse, inspect, transform, slice, summarize, or project owned persisted JSON/JSONL.
- Test fixtures may use JSON literals for construction and comparison, but tests must not establish `serde_json::Value` as a production reader pattern.
- Live typestate transitions and mutation logic stay in `ploke-eval`; passive persisted and transmitted shapes that are shared across tools live in `ploke-records` or in the owning crate when deliberately local.

## Total-View Questions

The typed spine must eventually let a UI or replay tool answer these questions from typed records:

- Which parent produced a patch for which child?
- What evidence selected the target write surface?
- Which protocol outputs were inspected to derive that choice?
- Which files did the generated patch touch?
- What metadata was created with the patch?
- What diff was applied from parent to child?
- Which typed decision/source record explains why the ruling parent allowed the patch through the edit surface?
- After apply, what did the child do during self-evaluation?
- Which target instance or oracle patch target did the child use?
- Which tool calls were made during child self-evaluation?
- What were the typed tool-call arguments and typed returns?
- What additional context was retrieved from the database and added to the run?
- Which typed records explain failure, timeout, rejection, retry, selection, handoff, or successor startup?

## Repeatable Lane

Run this loop for one execution-surface family at a time.

1. Inventory
   - Name the family and all paths/channels/artifacts it writes or reads.
   - Record writer function/type, reader function/type, file pattern, JSON/JSONL shape, and whether the shape is persisted, transmitted, projected, or UI-facing.
   - Include nested payloads and called helper functions that add data to the record.

2. Ownership Decision
   - If multiple crates need the shape, place the passive record type in `ploke-records`.
   - If only one crate owns and reads the shape, keep the type local but named and typed.
   - If the shape mirrors an external crate type, store a typed local record mirror or typed reference, not anonymous JSON.
   - Keep live typestate constructors, mutation, sealing, admission, and process execution in the owning runtime crate.

3. Type Gap Patch
   - Replace production `serde_json::Value` readers with concrete records, typed enums, or named typed projection structs.
   - Replace persisted `serde_json::Value` fields with typed records, typed enums, or typed parse/error records.
   - Move shared passive DTOs into `ploke-records` only when they are used across crate boundaries.

4. Replay Link
   - Add stable IDs, paths, hashes, spans, node ids, run ids, branch ids, artifact refs, or typed citations needed to join this family into parent/child/successor replay.
   - Preserve enough structure for both forward replay and backward diagnosis.

5. Tests
   - Add roundtrip tests for the record shape.
   - Add real-run fixture parse tests when fixtures exist.
   - Add nested-shape rejection tests for fields that previously passed through anonymous JSON.
   - Add projection tests only after the projection reads typed inputs.

6. Coverage Update
   - Update `typed-data-coverage-report.md`.
   - Add or update the survey row for the family.
   - Record any remaining blocker as a typed gap, not as an accepted exception.

## Survey Row Schema

Use this table shape for survey reports:

| Field | Meaning |
|---|---|
| Family | Short name: protocol artifacts, child plan, LLM attempts, tool calls, edit-surface patch, database context, evaluation report, etc. |
| Paths/channels | Exact file pattern, channel, artifact path, or stream. |
| Writer | Function/module/type that serializes or emits the data. |
| Current reader | Function/module/type that reads it today. |
| Current type | Rust type used by writer and reader, or `serde_json::Value` if non-compliant. |
| Desired type home | `ploke-records`, `ploke-eval`, `ploke-llm`, `ploke-tui`, or another crate. |
| Nested payloads | Important nested records/enums and where they currently live. |
| Replay joins | IDs/refs needed to connect parent, child, patch, protocol, tool call, database context, evaluation, and successor. |
| UI drilldown | What the frontend can show from this family. |
| Non-compliance | Exact `serde_json::Value`, stringly JSON, or field-walking site to replace. |
| Smallest verification | One bounded test command for this family. |

## Initial Family Order

1. Protocol artifacts
   - Current blocker: `ploke-records::protocol` stages `input`, `output`, and `artifact` through `serde_json::Value`.
   - Desired outcome: typed procedure envelopes/payloads deserialize without anonymous JSON.

2. Tool calls and tool results
   - Current blocker: `ToolCallRecord.arguments: serde_json::Value` and string argument readers in surrounding projection code.
   - Desired outcome: typed tool-call argument/result records plus typed parse/error records where needed.

3. Monitor/projection JSONL
   - Current blocker: `prototype1_observation_*.jsonl`, `agent-turn-trace.json`, `slice.jsonl`, and selected CLI projection helpers parse anonymous JSON.
   - Desired outcome: named projection record structs for every owned monitor/debug/projection JSON shape.

4. Edit-surface patch evidence
   - Survey grant/check/request/response/proposal/apply records and patch metadata.
   - Desired outcome: typed drilldown from parent choice to target surface, generated patch, diff, checks, apply outcome, and child state.

5. LLM request/response/attempt timeline
   - Survey `ploke-llm` request/response/attempt shapes and any persisted sidecars.
   - Desired outcome: typed UI records for prompt context, provider/model, attempts, timeouts, tool calls, and responses without moving live `ploke-llm` internals unnecessarily into `ploke-records`.

6. Database context retrieval
   - Survey typed context added from the underlying database into prompts or evaluation.
   - Desired outcome: typed record refs for retrieved context, query intent, source nodes, and evidence shown in UI.

7. Evaluation and oracle targets
   - Survey branch evaluation, target instance, oracle patch target, metrics, and self-evaluation tool calls.
   - Desired outcome: typed replay from child evaluation setup through result and selection input.

## Sub-Agent Assignment Template

Use this when delegating one family:

```text
Task handle: typed-persistence-spine
Family: <one execution-surface family>

Goal: inventory and patch only this family so owned persisted/transmitted JSON/JSONL is read through named Rust types. Do not introduce production serde_json::Value parsing. Do not move live typestate transition constructors out of ploke-eval.

Read first:
- docs/active/plans/self-improvement-loop/typed-persistence-spine-plan.md
- docs/active/plans/self-improvement-loop/typed-data-coverage-report.md
- AGENTS.md typed persistence invariant

Output:
- survey row using the plan schema;
- JSONL report path allocated by the main thread, with collision status;
- exact production Value/stringly JSON sites found;
- files changed;
- tests added or the smallest missing tests;
- one bounded verification command.

Report safety:
- Write only to the exact report path assigned by the main thread.
- Before writing, check whether the path exists.
- If it exists, do not overwrite it; write the next `-vN` filename and report the collision.
```

## Completion Bar

This lane is not complete until:

- every owned persisted/transmitted JSON/JSONL family has a named type;
- every production reader uses a named type or named typed projection;
- every nested owned payload has typed deserialize coverage;
- every family has a survey row;
- replay and UI can join parent, child, patch, tool, database context, evaluation, and successor records without parsing anonymous JSON.
