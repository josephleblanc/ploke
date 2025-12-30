# 2025-12-30 Context Management TODO

Goal: turn `docs/active/plans/context-management/immediate-steps.md` into concrete, code-grounded work that
moves the current implementation toward `docs/active/plans/context-management/context-management.md`.

Baseline (current code):
- Prompt assembly: `crates/ploke-tui/src/rag/context.rs` + `crates/ploke-tui/src/llm/manager/mod.rs`.
- Chat retention: `ContextStatus` + `TurnsToLive` in `crates/ploke-tui/src/chat_history.rs`; decremented
  globally via `ChatHistory::decrement_ttl` in `crates/ploke-tui/src/llm/manager/session.rs`.
- RAG context: full snippets in `ploke-rag` (`crates/ploke-rag/src/context/mod.rs`) injected as system
  messages via `reformat_context_to_system`.

## Tickets

- CM-01 ContextPlan snapshot artifact + tracing + deterministic test
- CM-02 Retention classes (Sticky/Leased) + decrement TTL only for included items
- CM-03 Tool episode atomicity (group tool call + result)
- CM-04 Light pack default for RAG context + config knob
- CM-05 Leased cap + activation ordering
- CM-06 Context Mode (Off/Light/Heavy) + budget meter line
- CM-07 Golden determinism test pinned to ContextPlan fixture

## Work items (ordered, concrete)

1) ContextPlan snapshot (describe first, then decide)
- Add a `ContextPlan` struct (new module, or `crates/ploke-tui/src/llm/manager/events.rs`) with:
  - `plan_id`, `parent_id`, `token_budget_estimate`
  - `included_messages`: message ids + kind + rough token estimate
  - `included_rag_parts`: part id + file_path + token estimate + score
  - `stats`: copy `AssembledContext.stats`
- In `crates/ploke-tui/src/rag/context.rs`, build `ContextPlan` right before `ChatEvt::PromptConstructed`.
- Emit the plan through tracing (or `observability.rs`) so it can be correlated to the run id.
- Acceptance: a test that verifies deterministic `ContextPlan` output given fixed messages + rag context.

2) Retention classes (Sticky vs Leased) and TTL only for included items
- Extend `ContextStatus::Pinned` in `crates/ploke-tui/src/chat_history.rs` with `retention: RetentionClass`.
- Default retention = `Leased`; set `BASE_SYSTEM_PROMPT` to `Sticky`.
- Change `ChatHistory::decrement_ttl` to accept a list of message ids (or a `ContextPlan`) and only
  decrement for included `Leased` items.
- Thread the included ids from `ContextPlan` through `StateCommand::DecrementChatTtl`.
- Acceptance: TTL changes only for included items; sticky items never decrement.

3) Tool episode atomicity
- Ensure assistant messages that trigger tools retain the `tool_call_id` (if missing).
- Update `ChatHistory::current_path_as_llm_request_messages` to group tool call + tool result
  (and optionally the assistant preamble) by `tool_call_id`, so they are included/excluded together.
- Add a unit test for grouping behavior in `crates/ploke-tui/src/chat_history.rs`.

4) Light pack default for RAG context
- In `crates/ploke-tui/src/rag/context.rs::reformat_context_to_system`, truncate snippets to a small,
  consistent window (e.g., 12–20 lines) and include `score` + `kind` in the header.
- Add a config knob (e.g., `rag.per_part_max_tokens`) and plumb it into `TokenBudget` in
  `crates/ploke-tui/src/app_state/core.rs` when calling `rag.get_context`.
- Update system prompt/tooling guidance to prefer `request_code_context` for deeper dives.

5) Leased cap + simple activation ordering
- Add `last_included_turn` + `include_count` to message metadata in `crates/ploke-tui/src/chat_history.rs`.
- When assembling prompts, order leased items by `last_included_turn` desc, then `include_count`.
- Apply a `max_leased_tokens` cap using `cap_messages_by_tokens` in
  `crates/ploke-tui/src/llm/manager/mod.rs`.

6) Context Mode (Off/Light/Heavy) + budget meter (UI)
- Add `CtxMode` to `crates/ploke-tui/src/user_config.rs`.
- In `crates/ploke-tui/src/rag/context.rs`, vary `top_k` and per-part budget based on mode.
- Emit a SysInfo line via `add_msg_immediate_sysinfo_unpinned` with mode + retrieved count + est tokens.

7) Golden determinism test
- Add a regression test to pin `ContextPlan` output for a fixed fixture in
  `crates/ploke-tui/src/llm/manager/mod.rs` or a dedicated context-plan test module.

Notes:
- `request_code_context` already supports a `token_budget` argument and is the current “expand” tool.
- ContextBrowser UI already renders `ContextPart` (`crates/ploke-tui/src/app/view/components/context_browser.rs`),
  so a truncated/annotated context part will be visible without new UI work.
