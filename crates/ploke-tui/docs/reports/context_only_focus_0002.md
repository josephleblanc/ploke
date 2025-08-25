# Context-Only E2E Goal — Confirmation and Critical Scope (0002)

Date: 2025-08-24
Owner: Tests

Purpose (confirmed)
- Our immediate goal is to validate one thing end-to-end: the LLM can request additional code context via a single tool and receive realistic snippets from our code graph/RAG so it can solve user requests. No file edits, no file metadata — just context.

What we will test
- A single tool: request_code_context
  - First leg: force this tool on a live OpenRouter endpoint that supports tool calling.
  - Local execution: call RagService::get_context over the pre-loaded fixture_nodes DB to assemble real code snippets.
  - Second leg: return a typed RequestCodeContextResult with AssembledContext.parts including snippet text. Also include for each part: file_path and canonical path (canon) to guide future edits.

Why this is sufficient
- If the LLM reliably receives code snippets plus file and canonical paths, it has the minimal information needed to reason about changes. This is the narrowest, highest‑value slice to validate our plumbing and typed IO.

Crucial items going forward
- Tool definition
  - Only expose request_code_context in the E2E test for now.
- Typed payload
  - Return RequestCodeContextResult { ok, query, top_k, context: AssembledContext } where ContextPart.text is populated.
  - Include per-snippet { id, file_path, canon } alongside the typed payload to make location and identity explicit to the LLM.
- Data sources
  - Use the pre-loaded fixture_nodes database and live RagService::get_context assembly.
- Provider selection
  - Choose endpoints that support tool calls; soft-skip those that return no tool_calls or 404 “no tools” with clear logs.
- Observability and diagnostics
  - Keep structured logs and a per-run summary; later, persist finish_reason and tool_call function names.
- Out of scope (for this test)
  - apply_code_edit and get_file_metadata tools.
  - Any system-file paths or hashes.
  - Enforcing semantic quality of assistant prose.

Next steps
- Run the simplified E2E to confirm consistent tool_calls and typed context returns.
- Iterate on provider allowlist/minimal success gating to stabilize CI signal.

This document locks our scope to “context-only” so we stay focused and deliver a reliable end-to-end validation of tool-based context retrieval.
