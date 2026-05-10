# Typed Persistence Spine Survey Inventory Handoff

Date: 2026-05-10

## State

The Prototype 1 typed-persistence-spine survey inventory is complete for the first planned pass.

Accepted sources of truth:

- [`inventory.jsonl`](inventory.jsonl)
- [`inventory.md`](inventory.md)
- [`survey-index.md`](survey-index.md)

The hard invariant for follow-up work remains:

> Ploke-owned persisted or transmitted JSON/JSONL must be read through named Rust `Serialize` / `Deserialize` types. Production code must not inspect owned shapes through `serde_json::Value`, anonymous field walking, or ad hoc JSON accessors.

## Accepted Surface Counts

| Family | Accepted surfaces | Non-compliant surfaces | Accepted report |
|---|---:|---:|---|
| `protocol-artifacts` | 5 | 3 | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `tool-calls-results` | 5 | 4 | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `monitor-projections` | 9 | 5 | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `edit-surface-patch-evidence` | 10 | 0 | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `llm-attempts` | 8 | 5 | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `database-context` | 4 | 0 | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) |
| `evaluation-oracle-targets` | 10 | 0 | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |

Total accepted surfaces: 51.

Superseded raw reports retained for traceability:

- `reports/2026-05-10-monitor-projections.survey-c.jsonl`
- `reports/2026-05-10-edit-surface-patch-evidence.survey-d.jsonl`

Use the `-v2` reports for accepted inventory rows.

## Main Findings

Direct typed-persistence violations are concentrated in four areas:

- Protocol artifact decode/storage/aggregate:
  `protocol.artifact.decode`, `protocol.artifact.store`, `protocol.artifact.aggregate.output`.
- Tool-call/result records and projections:
  `tool.call.record.arguments`, `tool.request.arguments.capture`, `tool.response.full_response_trace`, `tool.result.trace.projection`.
- Monitor/projection readers:
  `prototype1.history_preview_document`, `prototype1.history_preview_slice`, `prototype1.agent_turn_trace`, `prototype1.observation_jsonl`, `prototype1.slice_jsonl`.
- LLM/provider/tool bridge surfaces:
  `llm.attempt.provider_error`, `llm.attempt.timeout`, `llm.attempt.timeline`, `llm.dto.openai_response`, `llm.tool_bridge.records`.

The remaining families are mostly typed already, but they still need replay and ownership cleanup:

- `edit-surface-patch-evidence`: decide which proposal and candidate-local records stay in `ploke-eval` / `ploke-tui` and which passive mirrors belong in `ploke-records`.
- `database-context`: unify prompt-attached context and UI/database replay joins.
- `evaluation-oracle-targets`: unify target identity joins and selection/evidence replay joins across record, scheduler, and tree layers.

## Validation Performed

The accepted inventory and reports passed:

```bash
jq -c . docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl >/dev/null
jq -c . docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-protocol-artifacts.survey-a.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-tool-calls-results.survey-b.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-monitor-projections.survey-c-v2.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-llm-attempts.survey-e.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-database-context.survey-f.jsonl docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl >/dev/null
```

Additional checks:

- Accepted surface counts by family match `inventory.md`.
- Duplicate surface ID check returned no rows.
- Required array fields on accepted inventory rows returned no schema errors.
- Spot checks against cited code regions supported the report claims.

No Rust tests were run for this pass; this was a documentation and inventory survey.

## Next Implementation Slices

Start with the highest-signal direct violations:

1. `protocol-artifacts.decode`
   Replace `ploke-records::protocol` `serde_json::Value` staging with typed payload decoding.

2. `protocol-artifacts.storage-aggregate`
   Replace `StoredProtocolArtifact` raw payload fields and aggregate output cloning with typed payload DTOs.

3. `tool.call.arguments`
   Replace `ToolCallRecord.arguments` and `ToolRequestRecord.arguments` with typed argument carriers or typed parse-failure records.

4. `tool.result.trace.projection`
   Replace `cli_facing.rs` tool-result/turn-trace/observation projection field walking with named typed projection records.

5. `llm-attempts.provider-observation-projection`
   Replace provider error, timeout, and attempt timeline JSON projection with typed records.

6. `llm-attempts.dto-tool-bridge`
   Remove `serde_json::Value` from owned `ploke-llm` response metadata/logprobs and tool bridge argument records.

Then address the typed-but-scattered replay surfaces:

7. `edit_surface.source-records`
8. `database-context.prompt-evidence`
9. `evaluation-oracle-targets.identity-joins`
10. `evaluation-oracle-targets.selection-replay`

## Orchestration Notes

The survey workflow worked with bounded JSONL reports and sub-agent correction loops.

Two reports needed correction:

- `monitor-projections.survey-c` combined distinct surfaces; accepted replacement is `survey-c-v2`.
- `edit-surface-patch-evidence.survey-d` used string `ui_drilldown`; accepted replacement is `survey-d-v2`.

Concurrent source-code changes were present in the worktree during this pass and were intentionally ignored. Do not infer that source diffs belong to this inventory commit unless they are under `docs/active/plans/self-improvement-loop/typed-persistence-spine/`.
