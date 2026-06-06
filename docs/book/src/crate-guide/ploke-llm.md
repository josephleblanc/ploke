# `ploke-llm`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-llm` contains provider-agnostic LLM request/response, routing, registry, retry, and chat-step logic. It is used by the TUI but keeps HTTP/provider modeling outside the UI crate.

## Responsibilities

- Define chat request messages, roles, responses, tool calls, model IDs, provider keys, and route types.
- Build router-specific chat completion requests for OpenRouter and direct Google routes.
- Execute non-streaming chat HTTP attempts with timeout/retry calibration and provider-attempt timelines.
- Parse responses into `ChatStepOutcome::Content` or `ChatStepOutcome::ToolCalls`.
- Fetch/write/load model and embedding model registries.
- Expose calibration and route abstractions used by the TUI's chat loop.

## Key Files

- [`crates/ploke-llm/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/lib.rs) — public re-exports and crate boundary.
- [`crates/ploke-llm/src/manager/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/manager/mod.rs) — manager exports, `RequestMessage`, `Role`, token counter, endpoint request helper.
- [`crates/ploke-llm/src/manager/session.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/manager/session.rs) — `chat_step`, `chat_step_with_attempts`, `ChatHttpConfig`, `ChatStepOutcome`, response parsing and retry observations.
- [`crates/ploke-llm/src/request/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/request/mod.rs) — common request types such as model pricing and endpoint/model projections.
- [`crates/ploke-llm/src/registry/route.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/registry/route.rs) — `LlmRoute`, `OpenRouterRoute`, `GoogleRoute`, and tool support checks.
- [`crates/ploke-llm/src/router_only/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-llm/src/router_only/mod.rs) — router trait/variant boundary.

## Public API

- `chat_step` and `chat_step_with_attempts` — execute one chat request and return `ChatStepData` / `ChatStepError`.
- `ChatHttpConfig` — per-chat HTTP timeout/retry/backoff policy.
- `ChatStepOutcome::{Content, ToolCalls}` — structured result consumed by the TUI loop.
- `RequestMessage::{new_system, new_user, new_assistant, new_tool, new_assistant_with_tool_calls}` — message constructors with tool-call ID validation.
- `LlmRoute::{openrouter, google, direct_google_model}` — route construction and provider/tool-support projection.
- Registry/calibration exports such as `RouterCalibration`, `RetryTuning`, `AttemptTimeout`, `LlmRoute`, `OpenRouterRoute`, and `GoogleRoute`.

## Internal Structure

The manager/session layer sends router-specific requests through `reqwest`, assigns request IDs, emits optional protocol-debug JSON to stderr, records provider attempt timelines, parses successful bodies, and classifies failures by phase. The route layer separates OpenRouter routes (model, provider, endpoint) from direct Google routes (model plus tool-support flag) while presenting a common `SupportsTools` interface.

## Dependencies

- **Uses:** `reqwest`, `serde`, `serde_json`, `tokio`, `ploke-core`, provider-specific route modules.
- **Used by:** `ploke-tui` chat loop and model/provider commands, `ploke-embed` registry fixtures, protocol/eval tests where LLM adjudication is enabled.

## Notable Patterns / Gotchas

- `RequestMessage::Role::Tool` must carry `tool_call_id`; validation explicitly catches missing IDs.
- Pricing fields parse string or numeric values from provider catalogs; prices are stored per token, not per million tokens.
- Direct Google routes do not carry an OpenRouter provider endpoint; callers must check `is_direct_google`/`provider_key` instead of assuming provider endpoint presence.
