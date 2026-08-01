# Tool Calls / Results Provenance for MBE Instance Patch

## Scope

Discovery-only inventory for the typed-persistence family `tool-calls-results`, focused on `apply_code_edit` during Prototype 1 self-validation and how its tool-call records contribute evidence for the Multi-SWE-bench instance patch.

This report used the accepted inventory and survey rows first, then inspected only bounded source ranges. It did not inspect raw run logs, JSONL observation streams, or generated run artifacts.

## Inventory Rows Used

- `tool.call.record.arguments` from `docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/2026-05-10-tool-calls-results.survey-b.jsonl`
- `tool.request.arguments.capture` from the same report
- `tool.execution.record` from the same report
- `tool.response.full_response_trace` from the same report
- `tool.result.trace.projection` from the same report

Accepted inventory rows are mirrored in `docs/active/plans/self-improvement-loop/typed-persistence-spine/inventory.jsonl` lines 7-11.

## Source Code Ranges Worth Reading

- `crates/ploke-records/src/tool_contracts.rs:35-75`: `ToolArgumentsJson` carrier, including legacy-compatible deserialize through `serde_json::Value` and `decode_for_tool`.
- `crates/ploke-records/src/tool_contracts.rs:115-216`: closed `ToolCallArguments` enum and `apply_code_edit` decode route into `CodeEditParamsOwned`.
- `crates/ploke-tui/src/rag/utils.rs:13-19`: live `ApplyCodeEditRequest`.
- `crates/ploke-tui/src/rag/utils.rs:21-51`: live `Edit` modes accepted by `apply_code_edit`.
- `crates/ploke-tui/src/rag/utils.rs:211-242`: `ToolCallParams` and direct `ToolCallFailed` emission for legacy apply path failures.
- `crates/ploke-tui/src/llm/manager/session.rs:1622-1629`: `ToolCallRequested` events emitted after dispatcher setup.
- `crates/ploke-tui/src/tools/mod.rs:173-210`: preflight validation and typed deserialization for tool args.
- `crates/ploke-tui/src/tools/mod.rs:271-296`: `ApplyCodeEdit` dispatch, typed execute call, and terminal completion emission.
- `crates/ploke-tui/src/tools/code_edit.rs:93-128`: GAT wrapper converts typed params into legacy `ToolCallParams`, calls `apply_code_edit_tool`, then reconstructs typed result from proposal registry.
- `crates/ploke-tui/src/tools/code_edit.rs:131-210`: `ApplyCodeEditResult` content and `ToolUiPayload` fields for staged/pending state.
- `crates/ploke-tui/src/rag/tools.rs:250-323`: legacy staging result emission, including `auto_confirmed` and async approval spawn.
- `crates/ploke-tui/src/rag/tools.rs:622-658`: `apply_code_edit_tool` validation, failure emission, resolution, and staging call.
- `crates/ploke-tui/src/app_state/core.rs:321-374`: proposal status enum and `EditProposal` typed state.
- `crates/ploke-tui/src/rag/editing.rs:380-485`: semantic edit apply path, status transition to `Applied`/`Failed`, and terminal tool event payload.
- `crates/ploke-eval/src/runner.rs:730-764`: persisted `ToolRequestRecord`, `ToolCompletedRecord`, and `ToolFailedRecord`.
- `crates/ploke-eval/src/runner.rs:3516-3592`: self-validation event capture into `ObservedTurnEvent`.
- `crates/ploke-eval/src/record.rs:509-540`: request/result pairing by `call_id`.
- `crates/ploke-eval/src/record.rs:1222-1341`: full response trace type, turn trace projection event, `ToolExecutionRecord`, and `ToolResult`.
- `crates/ploke-eval/src/record.rs:1510-1516`: conversation `ToolCallRecord` arguments carrier.
- `crates/ploke-eval/src/record.rs:1608-1668`: `RunRecordBuilder` copies paired tool calls into `TurnRecord`; compressed record writer.
- `crates/ploke-eval/src/runner.rs:907-995`: patch artifact snapshots proposal statuses and expected file changes.
- `crates/ploke-eval/src/runner.rs:997-1006`: proposal status labels used in patch artifact snapshots.
- `crates/ploke-eval/src/runner.rs:1015-1068`: MBE submission record and `fix_patch` collection from repo diff.
- `crates/ploke-eval/src/runner.rs:2098-2105`: run artifact path names.
- `crates/ploke-eval/src/runner.rs:2408-2415`: turn artifact added to `RunRecord`.
- `crates/ploke-eval/src/runner.rs:2448-2486`: MBE submission artifact write and packaging state.
- `crates/ploke-eval/src/runner.rs:2527-2529`: `record.json.gz` write.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4698-4735`: typed `agent-turn-trace.json` projection counts completed/failed tool events.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4763-4810`: observation JSONL projection entry point; bounded use only.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:4941-4973`: remaining ad hoc field lookup for observation projection fields.

## Persisted Records / Artifact Paths Involved

- `record.json.gz`: compressed `RunRecord`; stores `TurnRecord.tool_calls`, `agent_turn_artifact.events`, patch artifact summaries, packaging metadata, and MBE submission path.
- `agent-turn-trace.json`: turn trace projection source; supports counting `ToolCompleted` and `ToolFailed` without rendered text parsing.
- `llm-full-responses.jsonl`: full provider response trace; contains raw normalized provider response with tool call argument strings.
- `multi-swe-bench-submission.jsonl`: MBE output; `fix_patch` is collected from `git diff --no-ext-diff --binary <base_sha or HEAD> --`, not reconstructed from tool-result text.
- `${config_dir}/ploke/proposals.json` or `PLOKE_PROPOSALS_PATH`: typed `Vec<EditProposal>` best-effort proposal registry persistence.

## Provenance Chain Contribution

1. Provider response yields a tool call with function name `apply_code_edit` and raw argument text.
2. TUI preflight validates and deserializes the argument string through the tool-specific `GatCodeEdit` path before execution.
3. The GAT wrapper converts the typed borrowed params into `ApplyCodeEditRequest`, calls the legacy staging path, and reconstructs `ApplyCodeEditResult` from the proposal registry.
4. The legacy staging path emits a typed-ish `ToolCallCompleted` content JSON and `ToolUiPayload` with `status=pending`, `staged`, `applied=0`, `preview_mode`, and in the legacy path `auto_confirmed`.
5. If `auto_confirm_edits` is enabled, approval is spawned asynchronously; the semantic apply path mutates `EditProposalStatus` to `Applied` or `Failed` and emits a second terminal tool event with `status=applied` or `status=failed`.
6. `ploke-eval` captures `ToolCallRequested`, `ToolCallCompleted`, and `ToolCallFailed` system events into `ObservedTurnEvent`, then pairs request/result by `call_id` into `ToolExecutionRecord`.
7. Patch artifact collection snapshots proposal statuses independently of rendered text.
8. The MBE `fix_patch` is produced from the repository diff at packaging time. Tool-call records explain why and how the edit was staged/applied; they are not the patch bytes themselves.

## Smallest Verification Commands

- `cargo test -p ploke-eval tool_calls_in_turn_returns_correct_calls 2>&1 | tail -n 20`
- `cargo test -p ploke-eval collect_patch_artifact_snapshots_applied_proposals 2>&1 | tail -n 20`
- `cargo test -p ploke-eval write_msb_submission_artifact_writes_treatment_submission_into_run_dir 2>&1 | tail -n 20`
- `cargo test -p ploke-tui apply_code_edit 2>&1 | rg 'auto_confirm|Applied|ToolCallFailed|ToolCallCompleted'`
- Metadata-only artifact check for a run directory: `ls -lh <run>/record.json.gz <run>/agent-turn-trace.json <run>/llm-full-responses.jsonl <run>/multi-swe-bench-submission.jsonl`

## Avoid Reading Wholesale

- Do not read `llm-full-responses.jsonl` wholesale. If needed, inspect at most the matching response line by request/turn id and cap width with `cut -c 1-400`.
- Do not read observation JSONL wholesale. Routine health/provenance checks should use mtimes, sizes, line counts, and exact `rg` patterns capped by `head` and `cut`.
- Do not read `agent-turn-trace.json` wholesale for routine checks. Prefer the typed `AgentTurnTraceProjection` path or bounded `rg -n 'ToolCompleted|ToolFailed|apply_code_edit' <file> | head -n 5 | cut -c 1-400`.
- Do not infer patch content from rendered chat summaries. The authoritative MBE patch bytes are from the packaging repo diff.

## Open Questions

- `ApplyCodeEditResult` is typed at construction, but persisted `ToolCompletedRecord.content` is still a `String`; readers must deserialize content explicitly if they need staged/applied/auto-confirm fields.
- `ToolUiPayload.fields` stores `status`, `staged`, `applied`, and `auto_confirmed` as string key/value pairs, not as a closed typed result enum.
- The first completion for `apply_code_edit` can mean staged/pending, while later completion or failure can mean applied/failed after auto-confirm. Consumers must account for multiple terminal-looking events for the same `call_id`.
- `ToolArgumentsJson` gives a named carrier and decode API, but its deserialize compatibility still uses `serde_json::Value` internally. That is intentionally bounded but is not fully end-to-end typed on the persisted wire.
- The observation projection now has named structs for many fields, but campaign/node/generation extraction still relies on helper field names across top-level/span/spans.
