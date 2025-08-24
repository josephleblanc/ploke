# Plan: Align e2e_openrouter_tools_with_app_and_db With Goals (v0.1)

Date: 2025-08-24
Owner: Tooling/Tests

Summary
- This plan states test goals, identifies misalignments, and outlines a phased set of changes to bring the test into alignment. It also defines change-reporting and code-review practices to accompany each change.

1) Goals for e2e_openrouter_tools_with_app_and_db
- Prove a realistic, end-to-end tool-calling cycle against a live OpenRouter endpoint:
  - Step 1: Force a specific tool with tool_choice and a valid tool schema.
  - Step 2: Execute the tool locally (RAG or file IO), returning typed JSON results.
  - Step 3: Send a second request with the tool results and validate a coherent assistant response.
- Use a realistic, pre-loaded DB (fixture_nodes) and RAG service to assemble code snippets.
- Provide model-safe, policy-friendly arguments (ephemeral file paths, valid hashes).
- Log outcome categories and summarize results across models/providers.
- Optionally (via env), enforce a minimal success criterion for CI signal.

Non-goals
- Do not assert semantic quality of assistant prose.
- Do not depend on a single provider or model always honoring tool_choice.

2) Current misalignments (based on recent runs)
- Provider endpoint selection:
  - Some models selected endpoints that advertise tools but still ignore tool_choice; no allowlist or minimal success gating exists. 404 and 429 cases are common.
- request_code_context payload returned to the LLM:
  - Second-leg tool results currently omit actual code snippets; only counts/stats are sent. This prevents the model from citing code, weakening the test.
- apply_code_edit arguments:
  - Uses a placeholder expected_file_hash (all-zero hex). Providers may reject or ignore risky/implausible arguments.
  - The edit is not chained to prior metadata results in a single per-model scenario, missing a realistic flow.
- Additional issues:
  - No logging of finish_reason for leg 1, reducing diagnostics.
  - Multiple tool_calls (if returned) are not handled beyond the first; we do not log the count.
  - Runtime can be long and prone to 429s; lack of allowlist increases flakiness.
  - Minimal success criteria are not enforced, so CI signal is weak.

3) Plan to bring the test into alignment
Phase 1 — Test-only improvements (low risk; contained in the test file)
- Improve provider/model selection:
  - Add optional env allowlist: PLOKE_LIVE_MODEL_ALLOWLIST="author/slug,author2/slug2".
  - Keep PLOKE_LIVE_MAX_MODELS cap; prefer fewer defaults for speed.
  - Keep soft-skip behavior for 404/no tool_calls; log finish_reason for better diagnostics.
- Make the scenario realistic per model:
  - Use a single NamedTempFile per model iteration.
  - Compute SHA-256 of the temp file, pass it as expected_file_hash to apply_code_edit.
  - Use the same path for get_file_metadata and apply_code_edit to mirror real flows.
- Typed context echo for request_code_context:
  - In the test’s local tool execution, call rag.get_context and return a typed payload that includes AssembledContext.parts with text (not just counts).
  - Keep payload concise but include snippet text to enable models to “see” code.
- Diagnostics and robustness:
  - Parse and log choices[0].finish_reason for the first leg.
  - Log the number of tool_calls (not just the first).
  - Optionally assert a minimal success criterion when PLOKE_LIVE_REQUIRE_SUCCESS=1 (e.g., require at least one full round-trip for request_code_context or get_file_metadata).

Phase 2 — Library alignment (requires code edits outside this test; to be proposed and reviewed separately)
- rag::tools::handle_request_context:
  - Return RequestCodeContextResult with AssembledContext including snippet text (typed IO end-to-end).
- apply_code_edit tool:
  - Validate expected_file_hash and support tracking_hash from metadata when available; clear, typed errors for mismatches.
- llm::session:
  - Ensure RequestMessage::new_tool always receives typed results (serde-serialized structs), not ad hoc JSON strings.
- Observability:
  - Persist finish_reason for leg 1 and function names of tool_calls.

4) Change reporting and code review process (to apply per change)
- For each change/PR, add two short docs in docs/reports/:
  1) A “Change Report” (why, what, alternatives, risks).
  2) A “Code Review and Alignment” note (how the changes meet goals; open questions).
- Name format:
  - change_report_YYYYMMDD_NNN.md
  - code_review_alignment_YYYYMMDD_NNN.md
- Keep both concise (<= 1 page each). Link to this plan, list files touched, and summarize test outcomes if applicable.

5) File-level task list
Phase 1 (test-only)
- crates/ploke-tui/tests/e2e_openrouter_tools_app.rs
  - Add allowlist env handling (PLOKE_LIVE_MODEL_ALLOWLIST).
  - Restructure per-model scenario: one temp file; compute hash; reuse path and hash for apply_code_edit args.
  - Update rag_request_code_context to serialize a typed payload including AssembledContext.parts text.
  - Log finish_reason; log number of tool_calls; add optional minimal success assertion (PLOKE_LIVE_REQUIRE_SUCCESS=1).
- docs
  - Add change and code-review templates for consistent reporting.

Phase 2 (follow-up; requires file access outside this chat message)
- crates/ploke-tui/src/rag/tools.rs — return typed AssembledContext in handle_request_context.
- crates/ploke-tui/src/llm/session.rs — typed-only tool results and configurable fallback.
- crates/ploke-tui/src/observability.rs — persist finish_reason and tool_call function names.
- crates/ploke-tui/src/rag/tools.rs — stricter apply_code_edit validation and preview diffs.
- crates/ploke-db (helper) — optional typed helper for NOW-snapshot queries used by editing.

6) Risks and mitigations
- Provider variability: Use allowlist + minimal success gate (opt-in) to stabilize CI.
- Token/cost/time: Lower model cap by default; keep small LLM token budgets.
- Payload size: AssembledContext payload could be large; respect token/char budgets and limit top_k.

7) Acceptance criteria
- At least one model/provider completes a full two-leg cycle for request_code_context or get_file_metadata (enforced when PLOKE_LIVE_REQUIRE_SUCCESS=1).
- Logs include finish_reason and tool_calls count for leg 1.
- request_code_context second leg includes snippets in the tool result payload (test-local first; later via rag/tools.rs proper).
- Summary line reports totals, successes, no_tool_calls, 404, 429.

Appendix A — Concrete misalignment checklist (for reviewers)
- [ ] Provider supports tools and honors tool_choice (at least one endpoint).
- [ ] request_code_context sends snippets (not just metadata).
- [ ] apply_code_edit uses realistic expected_file_hash for the same ephemeral file.
- [ ] Logging includes finish_reason and tool_calls count.
- [ ] Optional minimal success enforcement via env.

Appendix B — Next change set (Phase 1)
- Implement allowlist and minimal success env gates.
- Rework per-model scenario to tie get_file_metadata -> apply_code_edit.
- Return typed AssembledContext payload with snippet text from rag_request_code_context.
- Log finish_reason and tool_calls count.

This plan will be updated as we implement changes and review live outcomes.
