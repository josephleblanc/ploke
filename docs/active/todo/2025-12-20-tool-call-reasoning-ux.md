# 2025-12-20 — Tool-call + reasoning UX and validation fixes

## Why
- Users see empty assistant replies when the model emits tool calls with only `reasoning` (no `content`); we still mark the placeholder `Completed` → validation errors and blank UI rows.
- Tool call outputs are walls of JSON (full `node_info` etc.), hard to scan.
- Invalid status transitions (`Error -> Completed`) come from retrying after the empty completion failure.
- We need a clearer, user-visible “working” path during tool calls, while keeping full data for debugging/expansion.

## Goals (near-term)
1) Never send `Completed` with empty content; keep placeholders in-progress during tool calls.
2) Surface concise, non-empty status during tool calls using `reasoning` (or `content`) truncated to ~500 chars with ellipsis; store full text elsewhere.
3) Replace raw tool dumps with structured summaries (tool name, params, returned items or “no results”).
4) Use a single assistant message per chat_step and append tool summaries to it; finalize only when real assistant text arrives.
5) Preserve full `reasoning` and tool payloads in metadata/logs for later expansion/debug.
6) Lay the groundwork for verbosity controls (tool detail + reasoning detail) via config/hotkeys.

## Scope
- `crates/ploke-tui/src/llm/manager/session.rs` (tool-call branch, placeholder updates).
- `crates/ploke-tui/src/llm/manager/mod.rs` (`finalize_assistant_response` behavior).
- `crates/ploke-tui/src/chat_history.rs` (append vs replace content, status handling).
- UI rendering uses existing message content; no new UI wiring now beyond safer updates.

## Plan
- Validation safety
  - Guard in `finalize_assistant_response`: if `content.trim().is_empty()`, do **not** send `Completed`; leave status unchanged (still `Generating`) or emit a clear status/error string without completing.
  - In tool-call flow (`ChatStepOutcome::ToolCalls`), update placeholder with `status: None` (keep `Generating`) and non-empty status text derived from `reasoning`/`content`.
- Tool-call status text
  - Build `status_text = truncate(reasoning_or_content, 500, "…")`.
  - `UpdateMessage { content: Some(status_text), status: None }` for the placeholder on first tool-call turn.
  - For each tool result, append structured summary to the same message via `append_content`.
- Structured tool summary (MVP)
  - Shape: `- [tool_name] args: <compact>; result: <items>|(none)` per call.
  - Compact args: selected fields only; avoid full JSON/`node_info`.
  - Results: brief list or “no edges/items”.
- Single message per chat_step
  - Keep one assistant message for the step; append each tool summary as it completes.
  - Leave status as `Generating` until final assistant text; then `Completed` with non-empty content replaces current text (or appends after a separator).
- Reasoning handling
  - Use reasoning for interim visible status; truncate at ~500 chars.
  - Store full reasoning + token counts in metadata for later expansion (no UI exposure yet).
  - For long reasoning (open models), consider first sentence only in status; backlog the expand-on-hotkey behavior.
- Verbosity groundwork (backlog, not in this patch)
  - Config flag/hotkey to select: no tool output | minimal (names only) | default (compact summary) | verbose (structured detail).
  - Similar levels for reasoning (none | short | expand on demand).

## Deliverables (now)
- Safer update logic (no empty completion → `Completed`, no `Error -> Completed` retries).
- Tool-call status text (non-empty) sourced from reasoning/content, truncated.
- Structured tool summaries appended to the placeholder, minimal fields.
- Docs: this plan + ADR 004 capturing the decision.

## Open questions (backlog)
- Exact formatting for tool summaries (bullets vs table) once UI feedback arrives.
- Where to persist full reasoning/tool payloads for expansion (metadata vs hidden sysinfo).
- Hotkey/config plumbing for verbosity levels and reasoning expansion UX.
