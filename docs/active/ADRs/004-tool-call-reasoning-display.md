# ADR 004: Tool-call and Reasoning Display in Chat

## Status
Proposed (2025-12-20)

## Context
- Tool-call responses often carry a `reasoning` field and empty `content` (especially OSS models). We currently mark the assistant placeholder `Completed` with empty content → validation errors and blank UI rows.
- Tool results are emitted as huge JSON dumps (e.g., full `node_info`), cluttering the chat.
- Users need a visible “working” status during tool calls; empty completions degrade UX.
- Different models produce short status-like reasoning (closed) vs long traces (open); we need a scalable way to show/hide detail.

## Decision
- Treat tool-call turns as in-progress, not completed. Do not send `Completed` when `content` is empty; keep the placeholder `Generating` until a real assistant reply arrives.
- Use `reasoning` (fallback to `content`) as the interim user-visible status for tool calls, truncating to ~500 chars with an ellipsis for rendering; store full text in metadata for expansion/debug.
- Replace raw tool dumps with structured summaries: tool name, compact parameters, brief result list or “none”.
- For each chat_step, maintain a single assistant message and append each tool summary to it (using `append_content`), rather than creating multiple tool message nodes. Finalize with `Completed` only when non-empty assistant content is available.
- Long-term: introduce verbosity controls (hotkey/config) for tool detail and reasoning detail (none → minimal → default compact → verbose structured), with OSS-model reasoning defaulting to a short preview and a hint to expand.

## Consequences
### Positive
- Eliminates empty completions and invalid status transitions (`Error -> Completed`) during tool calls.
- Chat remains readable with concise, structured tool summaries instead of JSON walls.
- Users see immediate “working” feedback via reasoning/status text.
- Future-proofing: verbosity controls and expandable reasoning/tool detail can be layered without changing transport.

### Negative
- Slightly more logic in the chat loop to manage append vs replace and truncation.
- Need to curate which params/results appear in the compact summary to avoid losing important context.

### Neutral
- Full reasoning/tool payloads remain available in metadata/logs; UI can opt into expansion later.

## Notes
- Implementation guided by `docs/active/todo/2025-12-20-tool-call-reasoning-ux.md`.
