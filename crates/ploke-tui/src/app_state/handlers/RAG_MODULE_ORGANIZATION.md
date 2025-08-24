# RAG module organization plan

This document proposes a modular structure for the current handlers in `rag.rs`. This is a planning document only; it does not relocate code yet.

Goals:
- Keep responsibilities cohesive and extensible as more tools/capabilities are added.
- Make it easier to test each unit and reason about EventBus side-effects.
- Avoid circular dependencies across handler modules.

Proposed module layout (all under `app_state::handlers`):

- rag::tools
  - Purpose: Tool entrypoints used by the LLM to request metadata, code edits, or retrieval.
  - Functions:
    - get_file_metadata_tool
    - apply_code_edit_tool
    - handle_request_context

- rag::dispatcher
  - Purpose: Central tool-call dispatch used by legacy SystemEvent::ToolCallRequested path.
  - Functions:
    - handle_tool_call_requested

- rag::editing
  - Purpose: Edit proposal lifecycle management and user approvals/denials.
  - Functions:
    - approve_edits
    - deny_edits
  - Types and helpers:
    - Uses WriteSnippetData (from ploke_core), EditProposal/BeforeAfter/PreviewMode (from app_state::core).

- rag::context
  - Purpose: Build augmented prompts using retrieved context and conversation state.
  - Functions:
    - process_with_rag
    - construct_context_from_rag
    - assemble_context
  - Constants:
    - PROMPT_HEADER
    - PROMPT_CODE
    - PROMPT_USER

- rag::search
  - Purpose: Retrieval functions exposed to commands or tools.
  - Functions:
    - bm25_rebuild
    - bm25_status
    - bm25_save
    - bm25_load
    - bm25_search
    - hybrid_search
    - sparse_search
    - dense_search

- rag::utils
  - Purpose: Small helpers and local types used by multiple rag submodules.
  - Functions:
    - calc_top_k_for_budget
    - json_lit
  - Types:
    - ApplyCodeEditArgs
    - Action
    - EditInput
    - PerEditResult
    - ApplyCodeEditResult
    - ToolCallParams
  - Constants:
    - ALLOWED_RELATIONS

Notes:
- Modules reference AppState and EventBus by Arc to avoid global singletons.
- Database and IO interactions remain delegated (Database, IoManager, RagService); this module orchestrates and translates to AppEvents/SystemEvents.
- Keep dependencies one-directional where possible: tools/dispatcher depend on editing/search/context/utils, not vice versa.
