# Implementation Log 002 — Live tool-call example and test hardening

Date: 2025-08-24
Author: ploke-ai assistant
Related plan: docs/feature/agent-system/agentic_system_plan.md (Milestone 0/1)

Problem
- Our live E2E test against OpenRouter did not observe any tool-call executions even for tools-capable endpoints.
- Root causes:
  - Tools were not included in requests unless the capabilities cache explicitly marked the model as supports_tools.
  - Observation window and test flow did not reliably capture the two-leg cycle (request → tool_call → tool completion).

Goals
- Make the test a minimal, robust example of a production request that results in a real tool call and completion.
- Persist diagnostics for postmortem and improve signal quality while keeping runtime bounded.

Changes
- Capability refresh and injection:
  - Added a best-effort registry.refresh_from_openrouter() at test setup.
  - After selecting a tools-capable endpoint, force-mark the active model as supports_tools in the ProviderRegistry for this run, ensuring tools are included in the request payload.
- Prompt nudging:
  - Appended a strong instruction to the user prompt: “If tools are available, you MUST call request_code_context with {token_budget: 256}…”
- Observation loop:
  - Increased wait window to 20s and treat ToolEvent::Completed/Failed as evidence of a tool cycle (set saw_tool = true).
  - Break the model loop early once a tool call is observed to cap runtime and reduce flakiness.
- Assertion:
  - Assert at least one tool call was observed when OPENROUTER_API_KEY is present (skips remain in place if the key is missing).
- Warning cleanup:
  - Fixed a minor unused_mut warning in test_harness.rs and removed unused imports in the E2E test.
- Diagnostics (existing):
  - We continue writing request/decision/response/toolcall artifacts to target/test-output/openrouter_e2e for offline analysis.

Quality checklist (snapshot)
- Correctness:
  - [x] Tools included when endpoint declares support (via refresh + forced capability).
  - [x] Observes requested/completed/failed tool events and sets flags consistently.
  - [x] Clean compile with reduced warnings.
- Reliability:
  - [x] Early break on success to avoid long runs.
  - [x] Live skip when OPENROUTER_API_KEY is unset.
  - [ ] Deterministic tool_choice=required option (future; see Decisions).
- Observability:
  - [x] Request plans and toolcall envelopes are persisted.
  - [x] Concise console summaries.
- Safety:
  - [x] No changes to production code paths that loosen policy.
  - [x] Test-only capability forcing (scoped to test run).
- Doc coverage:
  - [x] This implementation log.
  - [x] Decisions list updated.

Open questions requiring a decision
1) Should we add a configurable tool_choice mode (auto vs required) on LLMParameters?
   - Recommended: yes (default auto; tests can set required).
2) Should tests pin a known tools-capable model to reduce flakiness?
   - Recommended: yes; keep a fallback heuristic that tries N models, but prefer a curated “golden” model ID in CI.
3) Persist observability to DB in tests?
   - Recommended: add a flag to enable DB-backed telemetry persistence during tests for richer assertions.
4) Tighten provider routing?
   - Option to require provider_slug pinning for all live tests to avoid routing variance.

Next steps (from the roadmap)
- Milestone 1:
  - Add tool_choice flag to LLMParameters and plumb through RequestSession::build_openai_request.
  - Expand tool test coverage: assert that request_code_context returns structured AssembledContext with non-empty parts.
- Milestone 2:
  - Add semantic_search tool and file_content_range tool with strong arg validation and tests.
- Milestone 3:
  - Introduce validation gates for apply_code_edit (fmt/clippy/test) and surface results in ToolCallCompleted payloads.

References
- docs/feature/agent-system/agentic_system_plan.md
- tests/e2e_openrouter_tools_app.rs
