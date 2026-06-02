# LLM Full Response Trace Stopgap

- date: 2026-04-13
- mode: surgical implementation note
- control_plane: [2026-04-12_eval-infra-sprint-control-plane.md](./2026-04-12_eval-infra-sprint-control-plane.md)
- workstream: `A4` / `A5`
- scope: narrow eval-introspection stopgap for raw provider response capture during agent runs

## Why This Exists

We needed a fast path to make raw provider response envelopes reachable from `ploke-eval` during live eval runs without stopping to redesign turn persistence or the normalized `RunRecord` schema.

The immediate driver was missing or stranded token-usage data when the useful information existed in the provider payload but was not ergonomic to inspect from existing eval artifacts.

## Current Implementation

Production code changes landed in:

- [crates/ploke-tui/src/llm/manager/session.rs](/home/brasides/code/ploke/crates/ploke-tui/src/llm/manager/session.rs)
- [crates/ploke-tui/src/tracing_setup.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tracing_setup.rs)
- [crates/ploke-eval/src/tracing_setup.rs](/home/brasides/code/ploke/crates/ploke-eval/src/tracing_setup.rs)
- [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs)
- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
- [crates/ploke-eval/src/record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs)

What changed:

- Added a dedicated tracing target: `llm-full-response`
- Just before finish-policy handling, `ploke-tui` serializes a wrapper record instead of the bare `OpenAiResponse`
- The wrapper currently contains:
  - `assistant_message_id`
  - `response_index` (`chain_index` within the chat loop)
  - `response: OpenAiResponse`
- `ploke-eval` now installs a dedicated target-filtered file appender for that target
- Each eval process writes a dedicated full-response trace file under `.ploke-eval/logs/`
- Agent eval runs now slice the relevant appended segment into a run-local sidecar artifact:
  - `runs/<instance>/llm-full-responses.jsonl`
- `execution-log.json` and `RunArtifactPaths` now point at that run-local sidecar when present
- `ploke-eval inspect turn <n> --show responses` now loads that sidecar and filters it by the turn's `assistant_message_id`
- `ploke-eval inspect turns` now falls back to sidecar-derived token totals when the normalized `RunRecord` totals are zero
- `inspect turn --show responses` now prints summed prompt/completion/total token totals for the displayed raw responses

## Important Joining Rules

- `assistant_message_id` is the stable assistant-node anchor for the whole chat turn
- `response_index` is required because the same `assistant_message_id` can legitimately receive multiple non-tool-call provider responses in one turn
- tool-call-bearing responses can also be matched through provider `tool_call.id`
- the raw provider response envelope also carries `OpenAiResponse.id`, which may be useful later but is not yet lifted into the wrapper as a first-class join field

## What This Does Not Do Yet

- It does not ingest the raw response stream into `RunRecord`
- It does not provide a run-level `inspect responses` surface yet; the current CLI exposure is turn-scoped via `inspect turn --show responses`
- It does not solve the earlier `chat_step -> Err(LlmError::...)` parse-failure path where no `full_response` exists
- It does not yet capture the final non-tool-call/`stop` response reliably; current sidecar totals can undercount by that missing final response
- It does not use the sidecar to derive displayed token cost; `inspect turns` still shows normalized-path cost only

## Recommended Next Step

Keep the next increment narrow:

1. run bounded evals using the current sidecar-backed token totals
2. fix the missing final `stop` response capture before trusting sidecar-derived cost or exact totals
3. only after that, decide whether the sidecar should stay file-backed only or gain a lightly indexed home in `RunRecord`

## Verification

- `cargo check -p ploke-eval` passed after the tracing-target, sidecar, and CLI fallback changes
- a fresh `run-msb-agent-single --instance BurntSushi__ripgrep-1294` emitted `llm-full-responses.jsonl`
- `inspect turn --show responses` displayed 13 raw response rows with summed totals
- OpenRouter dashboard comparison showed the sidecar is currently missing the final `stop` response:
  - sidecar totals: `prompt:224373 completion:7692 total:232065`
  - dashboard totals: `prompt:291021 completion:8450 total:299471`
  - missing delta: `prompt:66648 completion:758 total:67406`

## Risks

- The wrapper is intentionally minimal and may need one more correlation field if later inspection surfaces demand it
- This stopgap preserves raw data access but does not replace the normalized turn/usage model in `RunRecord`
- Sidecar-derived usage is good enough for readiness and debugging, but not yet exact enough to present as authoritative cost
