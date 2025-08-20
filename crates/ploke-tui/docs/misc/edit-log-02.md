2025-08-17T01:15:00Z — Configurable budgets/timeouts, lifetime fix, and deprecation stance

Summary
- Added configurable history_char_budget and tool_timeout_secs to LLMParameters with sensible defaults (history_char_budget=12000 chars, tool_timeout_secs=30s).
- RequestSession now uses:
  - history_char_budget (or falls back to 4x max_tokens or 12k chars) when capping conversation windows via cap_messages_by_chars.
  - tool_timeout_secs when awaiting tool results, replacing the previously hardcoded 30s.
- Fixed E0106 lifetime error by parameterizing cap_messages_by_chars over the RequestMessage lifetime.
- Kept SystemEvent::ToolCallRequested compatible but explicitly deprecated at call sites (warnings already present).

Files changed
- Modified: crates/ploke-tui/src/llm/mod.rs
  - cap_messages_by_chars signature now includes an explicit lifetime parameter.
  - LLMParameters: added history_char_budget: Option<usize> and tool_timeout_secs: Option<u64>, with Default values.
- Modified: crates/ploke-tui/src/llm/session.rs
  - Respect LLMParameters.history_char_budget for prompt window capping.
  - Use LLMParameters.tool_timeout_secs for per-tool call timeouts.
- Added: crates/ploke-tui/docs/edit-log-02.md (this file).

Reasoning
- Aligns with docs/tool-calls.md milestones: introduce configurable char budget; make per-tool timeout configurable; continue parallel tool calls with deterministic ordering.
- Resolves the build error surfaced in tests by annotating lifetimes where RequestMessage<'a> is cloned and returned.

Deprecation note (decision)
- We are deprecating the SystemEvent::ToolCallRequested path in favor of dedicated tool events routed by EventBus. The legacy system-event path remains functional for compatibility and is marked with runtime warnings pending the broader EventBus refactor.

Next steps
- Finish Milestone 2: Move message assembly entirely into RequestSession::new and reduce prepare_and_run_llm_call to a thin wrapper or remove it.
- Milestone 3: Introduce tool classification and route long-running tools over the background channel; RequestSession will subscribe appropriately while keeping subscribe-before-send semantics.
- Add tests: tool timeout and budget behavior; multi-call ordering; error payload shapes; provider response parsing resilience.

Notable potential issues (tracked)
- If a spawned tool task panics, JoinSet yields a join error and the session proceeds without that result; consider surfacing a system-visible error message.
- Aggressive history_char_budget may trim earlier context more than desired; consider heuristics or token-based caps in future.
