# Tool Packaging Investigation

Date: 2026-04-21

## Scope

I traced the eval path from CLI-visible outputs to run artifacts/logs and then into the code that builds and serializes the LLM request. The key distinction is:

- The live request package sent during current eval runs does include the full current tool set.
- The eval record/CLI artifact path does not preserve that full request package, so CLI inspection can make it look like tools were omitted when they were only omitted from diagnostics.

## CLI-observable behavior

The first CLI-level signal is that `ploke-eval conversations` is not intended to expose packaged tools. Its own help text says it reports turn number, timestamps, tool call count, and outcome, not the request envelope: [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1410).

On an existing eval record, `ploke-eval conversations --format json` shows `llm_request` with only `messages` and `model`. The same limitation is visible in the stored run record at [/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/record.json.gz](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/record.json.gz), where `llm_request` lacks `tools` and `tool_choice`, even though the run clearly executed tool calls.

That CLI symptom is real, but it is a record-capture limitation, not proof that the wire request omitted tools.

## Artifact and log evidence

Recent eval logs contain the raw outbound JSON request. In a 2026-04-21 run, the request log includes a `tools` array with all 10 current tools, including `insert_rust_item`, and `tool_choice: "auto"`: [/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log:142), [/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log:232), [/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log:647).

The same log’s `chat_http_request_start` line reports `tool_count=10`, which matches the 10 names in the serialized payload: [/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260421_062437_2.log:647).

Older logs do show only 9 tools. For example, a 2026-04-17 eval request includes 9 tool names and omits `insert_rust_item`: [/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log:342867), [/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log:343296). That older request also reports `tool_count=9`: [/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260417_094500_831901.log:699440).

That older 9-tool payload is best explained by version drift, not by a runtime serializer dropping one tool. Local git history shows `insert_rust_item` was added on 2026-04-20 in commit `6feee5a3` (`Add insertion tool and stabilize eval baseline`), after the 2026-04-17 log was produced.

Run artifacts also preserve tool-call responses but not request envelopes. The eval run’s response trace shows tool calls returned by the model, for example `read_file`, `list_dir`, `cargo`, and `request_code_context`: [/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/llm-full-responses.jsonl:1). The execution log stores the response-trace path, not a request-trace path: [/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/execution-log.json](/home/brasides/.ploke-eval/runs/tokio-rs__tracing-1015/execution-log.json:1).

## Code path

Tool definitions are built centrally from each tool’s `name`, `description`, and JSON schema through `Tool::tool_def()`: [crates/ploke-tui/src/tools/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs:507).

The current LLM manager assembles a 10-tool vector for eval/chat requests:

- `request_code_context`
- `apply_code_edit`
- `insert_rust_item`
- `create_file`
- `non_semantic_patch`
- `read_file`
- `code_item_lookup`
- `code_item_edges`
- `cargo`
- `list_dir`

Source: [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:507).

The only filter in this stage is all-or-nothing gating on whether a workspace/crate is loaded. If loaded, the full `tool_defs` vector is attached and `tool_choice` is set to `Auto`; otherwise both are `None`: [crates/ploke-tui/src/llm/manager/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/mod.rs:527).

The request object itself has `tools: Option<Vec<ToolDefinition>>` and `tool_choice: Option<ToolChoice>` as first-class serializable fields; there is no per-tool filtering logic in `ChatCompRequest`: [crates/ploke-llm/src/router_only/mod.rs](/home/brasides/code/ploke/crates/ploke-llm/src/router_only/mod.rs:367).

At send time, `ploke-llm` serializes the whole request with `serde_json::to_string_pretty(req)` for logging, computes `tool_count` from `req.tools.len()`, and sends the same request via `reqwest .json(req)`: [crates/ploke-llm/src/manager/session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:105), [crates/ploke-llm/src/manager/session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:127), [crates/ploke-llm/src/manager/session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:320).

## Transformations

I found intentional name transformations at serialization time, but not unexpected filtering:

- `NsPatch` serializes as `non_semantic_patch` and accepts alias `ns_patch`.
- `NsRead` serializes as `read_file` and accepts alias `ns_read`.

Source: [crates/ploke-core/src/tool_types.rs](/home/brasides/code/ploke/crates/ploke-core/src/tool_types.rs:4).

Those are wire-name transforms, not omissions. The recent raw request logs show the canonical wire names exactly as defined there.

## Where tools are actually lost

The eval record path drops tool definitions after the request is sent.

`TurnRecord.llm_request` is typed as `Option<ChatCompReqCore>`, not as the full `ChatCompRequest`, so it cannot store `tools`, `tool_choice`, or router/provider fields: [crates/ploke-eval/src/record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1455).

When a run record is built, the code reconstructs `llm_request` from only the captured prompt messages plus model:

- `ChatCompReqCore::default()`
- `.with_messages(artifact.llm_prompt.clone())`
- `.with_model(artifact.selected_model.clone())`

Source: [crates/ploke-eval/src/record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1486).

Upstream of that, the benchmark-event capture stores `formatted_prompt` from `PromptConstructed` into `artifact.llm_prompt`; that is message content only, not the full request envelope: [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3328).

So the current omission is in eval observability:

- Raw logs can show the actual packaged tools.
- `record.json.gz`, `agent-turn-trace.json`, and `ploke-eval conversations` cannot reliably answer “what tool definitions were sent?” because they only preserve prompt messages and model.

## Conclusion

For current eval runs, I found direct evidence that all 10 current tools are being included in the outbound LLM request package. I did not find evidence of a current per-tool filter or serializer bug in the request pipeline.

I did find two real sources of confusion:

1. Older eval logs from before 2026-04-20 contain only 9 tools because `insert_rust_item` had not been added yet.
2. Current eval records and CLI inspection omit `tools` and `tool_choice` from the captured `llm_request`, so offline inspection can falsely suggest that tools were missing from the original request.

If the question is “are tools missing on the wire during current eval runs?”, the evidence says no.

If the question is “can eval artifacts prove which tools were on the wire?”, the evidence says also no, because the record-building path drops that information after send time.
