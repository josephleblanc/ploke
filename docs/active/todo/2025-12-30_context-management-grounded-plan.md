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

- CM-01 ContextPlan snapshot artifact + tracing + deterministic test (done)
- CM-02 Retention classes (Sticky/Leased) + decrement TTL only for included items (done)
- CM-03 Tool episode atomicity (group tool call + result)
- CM-04 Light pack default for RAG context + config knob
- CM-04.5 Branch identity + per-branch turn counters
- CM-05 Leased cap + activation ordering
- CM-06 Context Mode (Off/Light/Heavy) + budget meter line
- CM-07 Golden determinism test pinned to ContextPlan fixture

## Progress

- CM-01 done: ContextPlan structs wired into PromptConstructed, plan built in RAG + fallback paths,
  debug-traced, deterministic unit test added in `crates/ploke-tui/src/rag/context.rs`.
- Test note: `cargo test -p ploke-tui` currently fails in preexisting `app::tests::file_completion_*`
  (missing Tokio runtime); CM-01 test passes.
- CM-02 done: added `RetentionClass` (Sticky/Leased), set base prompt to Sticky, and threaded
  included message ids from `ContextPlan` into `DecrementChatTtl` so only included leased items
  decrement.
- CM-03 notes: tool grouping likely belongs in `ChatHistory::current_path_as_llm_request_messages_with_plan`
  (`crates/ploke-tui/src/chat_history.rs`) since `tool_call_id` is already tracked on messages.
  Tool calls are generated in `crates/ploke-tui/src/llm/manager/session.rs` via tool events and
  recorded through `StateCommand::AddMessageTool` (`crates/ploke-tui/src/app_state/commands.rs`).
  The current prompt assembly includes tool messages individually; grouping should ensure assistant
  tool-call + tool result (and optional assistant preamble) are included/excluded together,
  probably by buffering items keyed by `tool_call_id` before finalizing the prompt list.
- CM-03 WIP: `ChatHistory::current_path_as_llm_request_messages_with_plan` now synthesizes assistant
  tool-call messages (from `tool_payload`) and groups them directly before tool results to preserve
  tool call/response atomicity in the prompt.
- CM-03 test: `tool_episode_groups_call_and_result` in `crates/ploke-tui/src/chat_history.rs`.
- CM-04 done: default RAG snippets are truncated to a small line window with kind/score headers,
  `rag.per_part_max_tokens` config drives `TokenBudget` for base RAG and `request_code_context`.
- CM-04 test: `reformat_context_to_system_truncates_and_includes_meta` in
  `crates/ploke-tui/src/rag/context.rs`.
- CM-04 impl notes: truncation uses `DEFAULT_CONTEXT_PART_MAX_LINES` (16) and appends
  `... [truncated]`; token estimates for `ContextPlan` use the truncated text. RAG budget is
  derived in `rag_budget_from_config` and applied in `crates/ploke-tui/src/lib.rs` during
  `AppState` creation. `request_code_context` now respects `rag.per_part_max_tokens`.
- CM-04 notes: light-pack truncation should live in `crates/ploke-tui/src/rag/context.rs`
  (`reformat_context_to_system`), which currently renders full snippets as system messages. Add a
  per-part token/line cap (likely via `TokenBudget`) and include score + kind in the header.
  Plumb config from `crates/ploke-tui/src/user_config.rs` into the RAG call path
  (`crates/ploke-tui/src/app_state/core.rs`) so the UI/default mode can control the per-part limit.
- CM-04 additional notes: `ContextPart` rendering is visible in the context browser UI
  (`crates/ploke-tui/src/app/view/components/context_browser.rs`), so header changes (score/kind) will
  be immediately visible without UI work; ensure truncation preserves header + a consistent excerpt
  window and avoids cutting mid-line. Consider a small default line cap (12–20) and apply it before
  formatting so token estimates remain aligned with what is sent.
- CM-04.5 done: added per-branch activation counters with a stable `branch_id` on `Message` so
  leased activation ordering is deterministic across concurrent branches.
- CM-05 done: leased cap ordering uses `last_included_turn` + `include_count`, with path-recency
  fallback for never-included items; excluded leased items are recorded in ContextPlan with
  Budget/TtlExpired reasons. Config lives in `context_management.max_leased_tokens` and selection is
  applied in `ChatHistory::current_path_as_llm_request_messages_with_plan`.
- CM-05 tests: `leased_cap_respects_activation_ordering` and
  `leased_cap_prefers_newest_when_never_included` in `crates/ploke-tui/src/chat_history.rs`.
- Test note: `cargo test -p ploke-tui` still fails in preexisting file completion tests:
  `app::tests::file_completion_resolves_temp_entries` (Tokio runtime missing in bm25_service)
  and `app::tests::file_completion_uses_cwd_for_bare_at` (cwd mismatch vs temp dir).
- CM-06 partial: added `CtxMode` (Off/Light/Heavy) config under `context_management` with per-mode
  `top_k` and `per_part_max_tokens` (Heavy defaults to 3x), RAG retrieval skips entirely on Off,
  and a minimal SysInfo budget meter is emitted when RAG runs (parts + estimated tokens). UI
  supports Ctrl+f cycling (Normal mode only) plus config overlay entries for mode/top_k/per-part.
- CM-06 complete:
  - [x] Add context mode indicator to the footer panel left of `ctx tokens: <amount>`.
  - [x] Make Ctrl+f cycle mode in Insert mode too.
  - [x] Config overlay: support +/- to adjust numeric values by 1 and Shift+/- by 10.
  - [x] Wire config overlay selections to update runtime config (apply changes).
  - Implementation notes (2025-12-30):
    - Footer now renders `ctx mode: <Off|Light|Heavy>` to the left of `ctx tokens`.
    - Ctrl+f cycles mode in Insert/Normal via keymap (Ctrl+f binding added for Insert).
    - Config overlay supports +/- (shift=10) on numeric values and applies changes immediately to
      runtime config (not persisted); help text updated accordingly.
    - Runtime config updates happen on config overlay input; app updates command style/tool verbosity
      fields for immediate UI consistency.

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

4.5) Branch identity + per-branch turn counters
- Add `branch_id` to `Message` and inherit it in `add_child`; create a new branch id in `add_sibling`.
- Track per-branch `last_turn` via a `BTreeMap<BranchId, BranchState>` on `ChatHistory`.
- Use the branch counter to stamp `last_included_turn` when leased items are included.

5) Leased cap + simple activation ordering
- Add `last_included_turn` + `include_count` to message metadata in `crates/ploke-tui/src/chat_history.rs`.
- When assembling prompts, order leased items by `last_included_turn` desc, then `include_count`.
- Apply a `max_leased_tokens` cap in `ChatHistory::current_path_as_llm_request_messages_with_plan`
  using the same ApproxCharTokenizer used for other token estimates.

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

## User experience changes (before/after)

Before this feature set:
- Context was assembled as a long, expanding tail of conversation + full RAG snippets.
- Tool call/results could be split apart, so partial tool traces leaked into the prompt.
- Auto-retrieval could feel heavy, with little user-visible explanation of what was included.
- TTL decreased globally, so early tool spam could linger even when not included.

After this feature set:
- Context is assembled as a explainable plan, with tool episodes kept together.
- RAG defaults to a light pack (short snippets with metadata); deeper context is requested explicitly.
- Leased items decay only when actually included, and a leased token cap limits bloat.
- Context mode (Off/Light/Heavy) is visible and user-controlled, with a small budget meter line after retrieval.

## Manual Verification
- [/] Check the footer: confirm ctx mode: Off|Light|Heavy updates as you cycle
  (Ctrl+f) in both Normal and Insert.
  - [x] verified that the UI updates
  - [ ] verified that the context management config also updates
- [x] Watch the SysInfo line after each user message: verify mode label, parts count,
  and estimated tokens look sane.
  - issue found: A SysInfo line appears just after user input, but is then
  overwritten or otherwise replaced (maybe by a message update) so the user
  never actually sees the context management feedback.
- [ ] Ask a small question first; note that RAG snippets are short “light pack” cards
  (header includes kind + score).
  - question: Where can I see these? Somewhere in the logs? Tell me where to find them.
- [ ] Use request_code_context on a symbol the model mentions; confirm the returned
  context is deeper than the auto pack.
- [ ] Trigger a tool call (e.g., list files) and then inspect if the tool call + tool
  result stay adjacent in the prompt (no orphan tool output).
- [/] Send a few turns and see if older tool outputs/leased items fall out when not
  included (TTL only decrements when included).
  - seems to work, as ctx tokens decreases significantly after the next user
  message, and not in the middle of the llm's tool calling loop between user
  messages.
- [ ] Try switching to CtxMode::Off and ask a question; confirm only conversation
  history is used and no RAG SysInfo line appears.
- [ ] If the context browser UI is visible, verify the truncated snippet and header
  metadata match what the model sees.
