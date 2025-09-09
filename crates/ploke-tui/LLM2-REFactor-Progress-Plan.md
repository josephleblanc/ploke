# LLM2 Refactor: Chat History → RequestMessage Mapping

Owner: AI coding agent (Codex CLI)
Date: 2025-09-09
Scope: Implement a clean API on `ChatHistory` to retrieve the selected conversation slice and map it to `llm2::manager::RequestMessage`, preparing for use in `llm2::manager::prepare_and_run_llm_call`.

## Context & Constraints
- Avoid using `llm` and `user_config` types in new functionality; prefer `llm2` equivalents.
- Project currently does not compile; keep changes surgical and isolated.
- `chat_history::MessageKind` includes `SysInfo` and `Tool` kinds which do not directly map cleanly to OpenAI-style `RequestMessage` (tool messages require a `tool_call_id`).
- `llm2::manager::RequestMessage` is the target type per current refactor direction.

## Plan
1. Add `ChatHistory::current_path_as_llm2_request_messages()` to return `Vec<llm2::manager::RequestMessage>`.
2. Use current selection path (root → current) to build request messages in correct order.
3. Map `User`/`Assistant`/`System` kinds; skip `SysInfo`. For `System`, drop empty content (root sentinel).
4. Defer `Tool` kind mapping (requires `tool_call_id` we don’t have in `Message`); skip for now and document.
5. Add a focused unit test covering mapping order, filtering, and content.
6. Wire into `llm2::manager::prepare_and_run_llm_call` to build `messages` via the new API; then cap history and prepare tools.
7. Introduce `llm2`-native `RequestSession<R>` that posts a generic `ChatCompRequest<R>` to the router’s completion URL; await tool calls and iterate.

## Reasoning & Decisions
- Order: Chat APIs expect oldest→newest. Using `current_path_ids()` ensures we only include context up to the currently selected parent message (branching point).
- Filtering: `SysInfo` is UI/diagnostic; not part of the conversational context. `Tool` requires a `tool_call_id`, which `Message` lacks; including a tool message without an ID would violate the OpenRouter contract. We skip for now; tool results are added via dedicated tool flow in the request session pipeline.
- System message handling: A root-empty system message is a structural sentinel; omitting avoids sending empty content.

## Changes Log
- 2025-09-09 1/5: Added plan.
- 2025-09-09 2/5: Implemented `ChatHistory::current_path_as_llm2_request_messages()` and unit test `current_path_as_llm2_request_messages_maps_and_filters`.
- 2025-09-09 3/5: Updated `prepare_and_run_llm_call` to:
  - Build `messages` via new ChatHistory API
  - Append `PromptConstructed` context pairs (skipping `SysInfo`/`Tool`)
  - Apply simple char-budget capping
  - Keep tools fixed for now; defer registry integration
  - Temporarily disable provider-bound diagnostics
- 2025-09-09 4/5: Added tests for `cap_messages_by_chars`/`cap_messages_by_tokens` in `llm2::manager` tests.
- 2025-09-09 5/5: Implemented generic `RequestSession<R>`:
  - Accepts `ChatCompRequest<R>` where `R: ApiRoute` and `R::Parent: Router`
  - Uses router constants (`COMPLETION_URL`) and resolves API key generically
  - Serializes `ChatCompRequest` directly to JSON and posts
  - Handles tool call cycle via event bus; appends tool results as `RequestMessage::new_tool`
  - Adds 404 tools fallback (disables tools and retries once if configured)
  - Updated manager to build a real `ChatCompRequest<openrouter::ChatCompFields>` and pass it to the new session

## To-Do / Reminders
- When registry (`llm2::registry`) is ready, plumb model preferences into `LLMParameters` and re-enable diagnostics with provider metadata.
- Consolidate `RequestMessage`/`Role` to a single module now that `llm2/chat_msg.rs` is removed.
- Add unit tests for `RequestSession` behavior (mock HTTP or surface parsing with fixture JSON).
- Wire `ChatCompRequest` transport for other routers by providing `ApiRoute` impls.

(Please keep updating this document as you proceed.)