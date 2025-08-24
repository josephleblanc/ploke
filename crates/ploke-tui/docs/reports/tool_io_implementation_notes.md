# Tool IO Implementation Notes

Scope
- Implement typed IO for tools starting with request_code_context.
- Keep the external OpenAI tool schema unchanged; only the internal payloads (tool role content) are made strongly typed.

Types (ploke-core)
- RequestCodeContextArgs { token_budget: u32, hint: Option<String> }
- RequestCodeContextResult { ok: bool, query: String, top_k: usize, context: AssembledContext }
- These live in ploke_core::rag_types alongside AssembledContext for cross-crate reuse.

Behavior (ploke-tui)
- rag::tools::handle_request_context now:
  - Parses args with serde into RequestCodeContextArgs (rejects 0 token_budget).
  - Builds a TokenBudget and calls RagService::get_context with RetrievalStrategy::Hybrid.
  - Wraps the AssembledContext into RequestCodeContextResult and serializes with serde_json for the tool role message.
  - Emits ToolCallCompleted with the typed JSON string.
- Local ad-hoc parsing code removed; stringly-typed "results" payload replaced with typed result.

Next steps
- Migrate other tools (get_file_metadata, apply_code_edit) to typed outputs in ploke_core::rag_types.
- Add serde round-trip tests for RequestCodeContextArgs/Result.
- Introduce a version field in tool results if we need forward-compatibility guarantees.
- Consider exposing a small crate ploke_tool_io if we outgrow rag_types.

Risks and mitigations
- Backward compatibility: The tool content remains JSON; prompts may need minor instruction updates.
- Performance: serde adds negligible overhead compared to retrieval. No hot-path regressions expected.
- Token budgeting: We currently use ApproxCharTokenizer via RagService defaults; real tokenizer adapters can be wired later.
