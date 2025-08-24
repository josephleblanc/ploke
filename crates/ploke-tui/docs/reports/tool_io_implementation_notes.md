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
- Add serde round-trip tests for RequestCodeContextArgs/Result, GetFileMetadataResult, ApplyCodeEditResult.
- Introduce a version field in tool results if we need forward-compatibility guarantees.
- Consider exposing a small crate ploke_tool_io if we outgrow rag_types.

Risks and mitigations
- Backward compatibility: The tool content remains JSON; prompts may need minor instruction updates.
- Performance: serde adds negligible overhead compared to retrieval. No hot-path regressions expected.
- Token budgeting: We currently use ApproxCharTokenizer via RagService defaults; real tokenizer adapters can be wired later.

Follow-up review and gaps
- Fixed missing imports in rag::tools (PathBuf, Arc); removed unused TrackingHash import.
- Removed an unused serde::Serialize import in llm::session to resolve warnings.
- Removed an unused calc_top_k_for_budget import in rag::dispatcher.
- Implemented a single retry fallback in llm::session: on 404 "support tool" responses, we now add a system message and retry once without tools before surfacing an error. This aligns with the checklist decision.
- Removed SystemEvent::ToolCallRequested path; unified on LlmTool::Requested in llm_manager.
- Migrated get_file_metadata and apply_code_edit to typed outputs in ploke_core::rag_types and updated tool handlers to emit ToolCallCompleted with typed JSON strings.
- Add serde round-trip tests for RequestCodeContextArgs/Result; add e2e tests for tool-call cycle with typed payloads.

Update (this commit)
- Added cap_messages_by_tokens in ploke_tui::llm and switched RequestSession to token-based history budgeting with char-budget fallback.
- Migrated get_file_metadata and apply_code_edit to typed results; apply_code_edit now emits ToolCallCompleted after staging with auto-confirm hint.
- No request payload shape changes; existing snapshot tests remain valid.
- Updated tool_call_flow.md to reflect typed IO, the implemented 404 fallback policy, token-based budgeting, and removal of the deprecated SystemEvent::ToolCallRequested path.
