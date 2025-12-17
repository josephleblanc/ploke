# ADR 002: Audience-Aware Tool Errors and Validation

## Status
Accepted (2025-12-17)

## Context
- Tool calls need actionable, audience-specific diagnostics (User e.g. convo history vs LLM vs System e.g. logging) instead of opaque deserialization errors.
- We must avoid tightening coupling between `ploke-llm` and tool implementations as the tool set grows.
- Common validation cases (e.g., token/context limits) and tool-specific checks (e.g., unified diff format) should share a consistent error surface and serialization for LLM feedback.

## Decision
- Introduce an audience-aware error type in the tools layer (not in `ploke-llm`):
  - `Audience` enum `{ User, Llm, System }`
  - `ToolErrorCode` covering shared and tool-specific cases (e.g., `FieldTooLarge`, `WrongType`, `MalformedDiff`, `InvalidFormat`, `Io`, `Internal`).
  - `ToolError` struct `{ tool: ToolName, code, field, expected, received, snippet, audience }` with `render(audience)` and `to_llm_payload()`.
- Add an adapter hook to the `Tool` trait: `fn adapt_error(err: ToolInvocationError) -> ToolError` (default implementation keeps tools decoupled from transport errors; tools override for richer hints).
- Provide shared validators in the tools crate (e.g., bounded integer, token/context limits, unified diff format) that return `ToolError`.
- Chat loop maps transport/serde failures into `ToolInvocationError`, then calls the tool’s `adapt_error`, emitting:
  - User/System surfaces: `render(Audience::User/System)`
  - LLM payload: `to_llm_payload()` embedded in tool result JSON for corrective guidance.

## Core Structures / Files
- `crates/ploke-tui/src/tools/error.rs`: `Audience`, `ToolErrorCode`, `ToolError`, `ToolInvocationError`, helpers.
- `crates/ploke-tui/src/tools/mod.rs`: `Tool` trait hook `adapt_error` (default impl) and shared validator module wiring.
- `crates/ploke-tui/src/tools/validators.rs` (new): reusable validation helpers (e.g., token/context limit, diff format).
- `crates/ploke-tui/src/llm/manager/session.rs` (integration point): bridges `LlmError` → `ToolInvocationError` → `ToolError` and routes audience-specific renderings.

## Consequences
### Positive
- Clear, audience-tailored diagnostics for both users and the LLM; corrective payloads improve tool-call recovery.
- Reduced coupling: `ploke-llm` remains transport-focused; tool errors live in the tools crate.
- Reusable validators make adding new tools consistent and lower-friction.

### Negative
- Slightly more plumbing in the chat loop to pass errors through the adapter.
- Additional types to maintain as tool surface grows.

### Neutral
- Some validators will need periodic expansion as tools add new fields.

## References
- Tracked in `docs/active/todo/2025-12-17-audience-aware-errors.md`.
