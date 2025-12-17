# 2025-12-17 — Audience-aware tool errors

- Implement ADR 002 (`docs/active/ADRs/002-audience-aware-tool-errors.md`): add Audience-aware `ToolError`/`ToolInvocationError`, trait hook `adapt_error`, shared validators (token/context limits, diff format), and chat-loop wiring.
- Ensure new errors render for User/Llm/System audiences and embed structured payloads in tool results.

Notes on implementation requirements:

- `ToolError` must implement `thiserror::Error` and `From<ploke_error::Error> for ToolError`
- `tools` crate does not exist yet, is currently `ploke_tui::tools` module (new crate `ploke-tools` someday, but beyond scope of ADR)
- Don't call User/System surfaces `render`, confuses with other `render` methods
  - see further main draw method for `App` in `crates/ploke-tui/src/app/mod.rs:400`
  - check whether should implement `RenderMsg` from `crates/ploke-tui/src/app/types.rs` and advise

