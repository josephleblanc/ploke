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

The queue of record is [`implementation-slices.md`](implementation-slices.md).

Use that file, not `.codex/task-stack.jsonl`, to decide what comes next for the typed-persistence-spine lane. The current first slice is `protocol-artifacts.decode`.

## Cold Restart Note: Tool Contracts

During the follow-up discussion, the intended tool-call direction was settled:

- Keep tool execution and runtime state in `ploke-tui`.
- Keep the owned LLM/tool transport DTOs beside the existing tool
  implementations.
- Expose those owned DTOs through a `ploke-tui` `tool_contracts` feature.
- Re-export them from `ploke-records` behind a `tool-contracts` feature as
  `ploke_records::tool_contracts`.
- Use those re-exported DTOs when implementing the later
  `tool.call.arguments` slice.

Current code state at handoff:

- `crates/ploke-tui/Cargo.toml` has a default `tool_contracts` feature.
- Owned tool transport DTOs in `crates/ploke-tui/src/tools/*.rs` derive
  `Deserialize` under `tool_contracts`.
- `crates/ploke-records/Cargo.toml` has an optional direct path dependency on
  `ploke-tui` with `default-features = false` and
  `features = ["tool_contracts"]`.
- `crates/ploke-records/src/tool_contracts.rs` re-exports the transport DTOs
  and explains why this exists.

Do not rehash this as a new architecture question on restart. The purpose is
to avoid mirrored DTOs while letting persisted record readers deserialize tool
arguments through the same Rust transport shapes that the tools use. Do not add
an `app` feature split or gate broad `ploke-tui` runtime modules as part of
this lane unless the user explicitly asks for a crate split.

Verification already run before this handoff:

```bash
cargo check -p ploke-tui 2>&1 | tail -n 80
cargo check -p ploke-records --features tool-contracts 2>&1 | tail -n 80
cargo check -p ploke-records 2>&1 | tail -n 80
```

An extra `ploke-records` unit test for the re-export was briefly added and then
removed because the user asked for simple Cargo/re-export wiring, not a test
slice. Do not restore that test unless the next implementation slice needs it.

## Orchestration Notes

The survey workflow worked with bounded JSONL reports and sub-agent correction loops.

Two reports needed correction:

- `monitor-projections.survey-c` combined distinct surfaces; accepted replacement is `survey-c-v2`.
- `edit-surface-patch-evidence.survey-d` used string `ui_drilldown`; accepted replacement is `survey-d-v2`.

Concurrent source-code changes were present in the worktree during this pass and were intentionally ignored. Do not infer that source diffs belong to this inventory commit unless they are under `docs/active/plans/self-improvement-loop/typed-persistence-spine/`.
