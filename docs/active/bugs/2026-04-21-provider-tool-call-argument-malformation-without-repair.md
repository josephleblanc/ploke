# Bug: Provider-emitted tool-call arguments can be malformed or schema-invalid, and `ploke` does not repair/retry them

**Date Discovered:** 2026-04-21  
**Crates Affected:** `ploke-llm`, `ploke-tui`, `ploke-eval`  
**Severity:** High  
**Status:** Open

## Summary

Two different providers produced invalid tool-call arguments during live evals,
and the current runtime accepted those tool calls without a provider-aware
repair/retry path.

Observed failure modes:

- `groq` + `moonshotai/kimi-k2-0905`
  - provider rejected invalid tool args at provider validation time
- `io-net` + `moonshotai/kimi-k2.6`
  - provider returned malformed `tool_calls[].function.arguments` JSON, which
    we accepted and then failed while dispatching the tool locally

This is one bug class with two provider-specific surfaces:

1. invalid tool args produced by the model/provider
2. no structured repair/retry path in `ploke` once that happens

## Concrete Reproductions

### 1. `groq` rejects invalid `apply_code_edit` / `cargo` args

Observed on live eval runs:

- `BurntSushi__ripgrep-2209`
  - provider/model: `groq` / `moonshotai/kimi-k2-0905`
  - final raw response error:
    - `apply_code_edit.edits` expected array, got string
- `tokio-rs__bytes-543`
  - provider/model: `groq` / `moonshotai/kimi-k2-0905`
  - final raw response error:
    - `cargo.command` must be one of `"test"`, `"check"`

Relevant artifacts:

- [ripgrep attempt 4 llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776835790755-structured-current-policy-08408511/llm-full-responses.jsonl:1)
- [bytes attempt 1 llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776836358560-structured-current-policy-e15f7836/llm-full-responses.jsonl:1)

### 2. `io-net` emits malformed `tool_calls[].function.arguments`

Observed on live eval run:

- `tokio-rs__bytes-543`
  - provider/model: `io-net` / `moonshotai/kimi-k2.6`
  - raw provider response contains truncated `read_file.arguments`

Relevant artifacts:

- [bytes attempt 2 llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/llm-full-responses.jsonl:13)
- [bytes attempt 2 agent-turn-summary.json](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/agent-turn-summary.json:1438)

The malformed call recorded on the run is:

```json
{"file": "/home/brasides/.ploke-eval/repos/tokio-rs/bytes/src/bytes.rs", "start_line": 1, "end_line":
```

and the corresponding local tool failure is:

- `read_file: failed to parse tool arguments: EOF while parsing a value`

## Pipeline

### Current behavior

1. Provider returns a chat completion response with `message.tool_calls`
2. `ploke-llm` accepts any non-empty `tool_calls` payload as authoritative
3. `ploke-tui` trims a suffix token if present, then directly
   `serde_json::from_str`s the raw `arguments` string for the selected tool
4. malformed args become tool failures, or provider validation errors terminate
   the turn, with no provider-aware repair/retry loop

### Relevant code

- accept `tool_calls` as the outcome:
  - [crates/ploke-llm/src/manager/session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:727)
- preserve provider-supplied `reasoning` / `content` / `tool_calls`:
  - [crates/ploke-llm/src/manager/session.rs](/home/brasides/code/ploke/crates/ploke-llm/src/manager/session.rs:729)
- tool dispatch uses raw `function.arguments`:
  - [crates/ploke-tui/src/tools/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs:165)
- only current sanitization is suffix trimming:
  - [crates/ploke-tui/src/tools/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs:466)
- tool arg parse boundary:
  - [crates/ploke-tui/src/tools/mod.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/mod.rs:544)
- parse failures become `WrongType` tool errors:
  - [crates/ploke-tui/src/tools/error.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/error.rs:181)

## Why this matters

- A provider can reject the tool call before dispatch (`groq`)
- A provider can emit malformed tool-call JSON that reaches local dispatch
  (`io-net`)
- In both cases the turn is lost because we do not yet:
  - validate tool args before treating the response as a valid tool-call step
  - emit a structured “repair your tool args” continuation
  - retry the step with provider-specific context

This is especially damaging for evals because a run can do substantial useful
work and still end:

- `completed + empty_patch`
- `completed + nonempty_patch + exhausted`
- `aborted`

depending on when the invalid tool call occurred.

## Evidence From This Investigation

### `io-net` malformed tool-call response

From the raw persisted provider response:

- [bytes attempt 2 llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/llm-full-responses.jsonl:13)

The response includes:

- `finish_reason: "tool_calls"`
- `message.tool_calls[0].function.name = "read_file"`
- malformed `message.tool_calls[0].function.arguments`
- nested `message.reasoning`

That means the malformed JSON came from the provider payload itself, not from
our inspector.

### Local dispatch failure

- [bytes attempt 2 tool call 16](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/llm-full-responses.jsonl:13)
- [bytes attempt 2 agent-turn-summary.json](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/agent-turn-summary.json:1356)

The exact tool error:

- `failed to parse tool arguments: EOF while parsing a value`

### Later provider abort

- [bytes attempt 2 agent-turn-summary.json](/home/brasides/.ploke-eval/runs/tokio-rs__bytes-543/runs/run-1776837022039-structured-current-policy-0d263016/agent-turn-summary.json:1454)

The later API error is:

- `HTTP 400`
- provider detail:
  - `{"detail":"Invalid request. Please check your input and try again."}`

## Expected Behavior

When a provider emits malformed or schema-invalid tool args, the runtime should
not treat that as a normal completed tool-call step and then drift into an
opaque failure.

Instead, the runtime should:

1. detect malformed or invalid tool args
2. classify the failure as tool-argument repairable
3. send a compact correction message back to the model
4. retry the step, ideally with provider-specific handling

## Likely Fix Direction

Short term:

- validate tool-call `arguments` before normal dispatch
- distinguish:
  - malformed JSON
  - schema-invalid JSON
  - provider-side tool validation rejection
- add a repair/retry path instead of immediately exhausting/aborting

Longer term:

- provider-aware policy:
  - `groq`: recover from provider validation rejection
  - `io-net`: recover from malformed emitted tool-call JSON
- possibly tighten tool descriptions/examples for providers that are repeatedly
  emitting bad arguments

## Restart Note

This bug report was written before a cold restart because the provider/tool-call
failure pattern is now reproducible and should not be lost in handoff. On
restart, begin from this report and the linked run artifacts instead of
reconstructing the pattern from compact memory.
