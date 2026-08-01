# Tool-Call Payload Semantics

## Findings

- `ploke-eval inspect tool-calls 0` can legitimately show both `truncated: true` and `lines: full`.
  - In `NsRead::execute`, `lines` is a UI field derived only from the requested line window: `(None, None) => "full"` means "no explicit `start_line`/`end_line` was requested," not "the entire file was returned" (`crates/ploke-tui/src/tools/ns_read.rs:248-259`).
  - `truncated` is derived from `io_truncated || slice_truncated` (`crates/ploke-tui/src/tools/ns_read.rs:224-250`).
  - `io_truncated` comes from `ploke-io` byte-cap truncation, and `NsRead` applies a default `max_bytes` cap of `32 * 1024` when omitted (`crates/ploke-tui/src/tools/ns_read.rs:186-202`).
  - `ploke-io` currently ignores the requested line range in the actor (`// TODO: implement server-side slicing; let _ = range`) and truncates by bytes in `read_plain_file` (`crates/ploke-io/src/actor.rs:311-319`, `388-425`).
  - So `lines: full` + `truncated: true` means: the caller asked for the whole file, but the returned content was shortened by byte cap and/or post-read slicing semantics.

- The table/detail rendering from `inspect tool-calls` is partly raw data and partly convenience rendering.
  - `InspectToolCallsCommand::run` loads `ToolExecutionRecord` from the eval record and routes table output through `print_tool_call_detail` (`crates/ploke-eval/src/cli.rs:5107-5139`, `8571-8601`).
  - The `Inputs` section is a pretty rendering of the stored raw argument string `call.request.arguments`; `--full` also prints the raw argument JSON verbatim (`crates/ploke-eval/src/cli.rs:8584-8600`).
  - The `Result` section first prints `ui_payload.summary` and `ui_payload.fields`, then separately prints `completed.content` or `failed.error` (`crates/ploke-eval/src/cli.rs:8604-8647`).
  - That means `lines: full` is not from the model-facing tool result payload. It is a human/UI convenience field carried in `ToolUiPayload`.

- The actual model-facing payload on the next turn is the tool `content`, not the `ui_payload`.
  - After tool execution, `run_chat_session` appends only `tool_result.content` into `req.core.messages` via `RequestMessage::new_tool(...)`; `tool_payload` is sent separately to app state for UI rendering (`crates/ploke-tui/src/llm/manager/session.rs:867-907`).
  - `ToolCompletedRecord` persists both `content` and `ui_payload`, so eval inspection can show both (`crates/ploke-eval/src/runner.rs:728-749`).
  - `ToolExecutionRecord` in the eval record therefore mixes model-facing payload (`content`) with UI-only metadata (`ui_payload`) (`crates/ploke-eval/src/record.rs:1233-1255`).

- The eval record does not preserve the full outbound request schema/tool envelope.
  - `RunRecordBuilder::add_turn_from_artifact` reconstructs `llm_request` from `artifact.llm_prompt` plus model id only; it does not persist `tools` or `tool_choice` (`crates/ploke-eval/src/record.rs:1485-1503`).
  - `RawFullResponseRecord` preserves full normalized provider responses, but only responses (`crates/ploke-eval/src/record.rs:1222-1230`).
  - So `inspect tool-calls` cannot prove the exact provider-facing tool schema from the run record alone.

- The live `read_file` schema is not inconsistent in current code.
  - `NsRead::schema()` includes both `start_line` and `end_line` (`crates/ploke-tui/src/tools/ns_read.rs:22-33`, `93-95`).
  - `Tool::tool_def()` clones that schema directly into the provider tool definition (`crates/ploke-tui/src/tools/mod.rs:507-513`).
  - The active tool list includes `NsRead::tool_def()` (`crates/ploke-tui/src/llm/manager/mod.rs:507-518`).
  - The stored fixture that captures tool definitions also shows both fields in the `read_file` schema (`crates/ploke-eval/src/tests/fixtures/BurntSushi__ripgrep-2209_code_item_lookup_context.json:222-246`).
  - If the CLI appears to show `start_line` without `end_line`, that is an invocation/rendering artifact: `render_tool_inputs` only prints keys present in the actual call arguments, not absent schema fields (`crates/ploke-eval/src/cli.rs:8650-8665`).

## What To Inspect Next

- CLI rendering path:
  - `crates/ploke-eval/src/cli.rs`
  - `InspectToolCallsCommand::run` (`5107-5139`)
  - `print_tool_call_detail` (`8571-8601`)
  - `print_tool_result_detail` (`8604-8647`)
  - `render_tool_inputs` (`8650-8665`)

- `read_file` tool semantics:
  - `crates/ploke-tui/src/tools/ns_read.rs`
  - `NsRead::schema` / `NS_READ_PARAMETERS` (`22-33`, `93-95`)
  - `NsRead::execute` (`128-265`)
  - Focus on the split between serialized `NsReadResult` and `ToolUiPayload`.

- Actual outbound tool schema and request payload:
  - `crates/ploke-tui/src/llm/manager/mod.rs`
  - tool list construction with `NsRead::tool_def()` (`507-518`)
  - `crates/ploke-tui/src/tools/mod.rs`
  - `Tool::tool_def()` (`507-513`)
  - `crates/ploke-llm/src/manager/session.rs`
  - request serialization/logging in `chat_step` (`72-79`)
  - `crates/ploke-tui/src/llm/manager/session.rs`
  - request log path constants and `log_api_request_json` / `write_payload` (`46-48`, `1535-1565`)
  - The concrete artifact to inspect is `logs/openrouter/session/last_request.json`, not `inspect tool-calls`.

- Payload persistence gap in eval:
  - `crates/ploke-eval/src/record.rs`
  - `RunRecordBuilder::add_turn_from_artifact` (`1485-1503`)
  - `RawFullResponseRecord` (`1222-1230`)
  - If we want reproducible provider-facing tool schema inspection from eval artifacts, this is where the schema/tool-choice/request envelope would need to be stored.

- I/O truncation semantics:
  - `crates/ploke-io/src/actor.rs`
  - `IoRequest::ReadFile` handling ignores `range` today (`311-319`)
  - `read_plain_file` truncates only by byte limit (`388-425`)
  - This is the main place where "full" can diverge from "complete content delivered."
