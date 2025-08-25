# Tool Call Flow: Reflection and Critique

This document reviews the current implementation of the tool-call flow, highlights what’s working, identifies gaps and style issues, and proposes focused improvements and next steps.

## What’s implemented well

- Event flow cohesion
  - Typed path: LlmTool::Requested is dispatched; rag::dispatcher routes by tool name; handlers emit SystemEvent::ToolCallCompleted/Failed; llm::session::await_tool_result correlates (request_id, call_id) with timeout.
  - Concurrency: Tool calls are executed concurrently via JoinSet and outcomes are sorted deterministically by call_id.

- Request/response plumbing
  - RequestSession::run enforces bounded history via token/char budget, builds OpenAI-compatible chat/completions payloads, supports tools, and appends tool results as “tool” role messages for iterative cycles.
  - build_openai_request produces stable, snapshot-tested JSON; provider.order is included when tools are enabled and provider_slug is known.

- Error reporting and resilience
  - Clear mapping of 401/429/5xx to actionable messages.
  - Providers that return “tool support not available” inside a 404 are handled with a one-shot fallback (retry without tools) plus guidance.

- Typed tool IO progress
  - request_code_context returns a typed RequestCodeContextResult with AssembledContext, assembling real snippets via RagService::get_context.
  - get_file_metadata and apply_code_edit return typed results (GetFileMetadataResult, ApplyCodeEditResult).

- Observability
  - observability.rs persists ToolEvent lifecycle with sha256 of args and captures latency by correlating start/end.

## Gaps and style issues

- Tool capability fallback policy
  - Current behavior retries once without tools on 404 “support tool” errors. It’s reasonable, but should be governed by config; ensure logs are concise and actionable.

- Strong typing coverage
  - We still pass JSON strings across several boundaries. Inputs are mostly typed, but ensure all outputs are type-checked where feasible and versionable for forward compatibility.

- DB queries in apply_code_edit
  - CozoScript string with inlined JSON is error-prone. Prefer parameterization or a helper in ploke-db to return EmbeddingData-like rows by canon/path, with stable schema and NOW snapshots.

- Token budgeting for history
  - Approx tokens = ceil(chars/4) is acceptable as a default; introduce a TokenCounter adapter when a model-specific tokenizer is available. Keep char budgeting as a fallback.

- Tracing and spans
  - Good coverage with structured fields. Consider adding per-request spans that include request_id, parent_id, call_id consistently across tool handlers for easier log correlation.

- Redundant compatibility paths
  - The SystemEvent::ToolCallRequested path is deprecated in the main flow; keep only if other callers still rely on it. Plan removal once migration is complete.

- Cloning and allocations
  - Some clones around tool specs/messages are benign now; audit once functionality settles.

## Open questions

- Policy: Should tools-only enforcement block calls to non-tool endpoints entirely, or should we always retry once without tools when requested tools are unsupported? Make this configurable.
- Versioning: Do we add a “version” field to typed tool results to allow evolution of payloads without breakage?
- Concurrency limits: Do we need a cap on parallel tool calls per request for resource fairness?

## Concrete improvements

1) Tighten typed IO
- Ensure all tool outputs are serialized from typed structs; add serde round-trip tests.
- Consider adding a version: u32 in result payloads for forwards compatibility.

2) Parameterized DB access for code edits
- Replace ad hoc CozoScript in apply_code_edit with a small query helper in ploke-db that returns the required fields with NOW snapshots.

3) Configurable fallback behavior
- Drive the 404 tool fallback from provider_registry.require_tool_support or a new editing/tools policy knob.

4) Token budgeting adapter
- Add a TokenCounter abstraction to cap_messages_by_tokens; keep ApproxCharTokenizer as default.

5) Observability enhancements
- Persist tool names, args digests, outcomes, and latencies (already mostly implemented). Add model, provider_slug, and error categories for richer analytics.

6) E2E and failure-path tests
- Add tests for: success tool cycle (single and multiple tools), tool timeout, unsupported vendor payloads, 404 fallback, and request payload snapshots with tools disabled/enabled.

## Next steps overview

- Tests: Add typed IO round-trips and E2E tool-cycle tests.
- DB: Introduce a safe, parameterized query helper for apply_code_edit.
- Config: Add a knob for tool fallback policy and update logs.
- Budgeting: Wire TokenCounter, keep char fallback.
- Docs: Update flow docs and provider capability guidance.
