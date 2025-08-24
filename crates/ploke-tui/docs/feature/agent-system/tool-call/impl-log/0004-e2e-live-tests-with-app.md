# Impl Log 0004 — E2E Live Tool-Cycle Tests with App Init and Pre-loaded DB

Date: 2025-08-24

Summary
- Added an end-to-end live test that:
  - Initializes a realistic App via test_harness (for env/config parity).
  - Restores a pre-loaded graph database from a verified fixture backup.
  - Builds a working RagService with BM25 rebuilt.
  - Interacts with a real OpenRouter endpoint using forced tool calls for our three tools.
  - Executes tool results locally (tempfile operations) and via real RAG for `request_code_context`.
  - Completes the second leg by posting a tool role message with the JSON result.

Rationale
- Establish a working E2E baseline so we can safely iterate on observability persistence and additional tool policies.
- Validate that our tool definitions and payload shapes interoperate with real provider endpoints.
- Confirm that RAG context assembly works over a realistic database snapshot, enabling stronger assertions in follow-on work.

What the new test covers
- Forces tool_calls for models with tools-capable endpoints (cheapest endpoint chosen by prompt+completion price).
- Round-trip flow (two-leg): request -> tool_calls -> local execution -> tool result posted -> completion.
- Typed RAG path: `request_code_context` uses RagService::get_context over the pre-loaded DB and returns structured AssembledContext stats in the JSON content.

What it does not cover (yet)
- In-crate tool dispatch via LlmTool::Requested and rag::dispatcher (will require wiring llm_manager in-test).
- Observability store updates (we'll add coverage once the E2E harness is locked down).
- Final assistant message semantics or quality; current scope is lifecycle correctness.

Open questions
- Should the E2E test drive the internal event loops (llm_manager/observability) to validate ToolEvent persistence?
- Where to set caps/timeouts to keep CI robust while still providing signal (e.g., max models, provider selection policy)?
- Do we standardize expectations for `request_code_context` content shape (e.g., minimal JSON contract) for forward compatibility?

Next steps
- Wire a minimal in-test loop running `llm_manager` to dispatch real tool handlers (rag::dispatcher) and verify on-bus outcomes.
- Add observability assertions: ensure ToolCallReq/Done are persisted with model and provider_slug fields.
- Promote deterministic assertions for `request_code_context` (e.g., count > 0, stats.tokens within budget).
- Introduce a config toggle to control retry-without-tools policy; expand live tests to cover strict and lenient modes.

Files added
- crates/ploke-tui/tests/e2e_openrouter_tools_app.rs
  - Live two-leg tool-cycle test with App init and pre-loaded DB.
- crates/ploke-tui/docs/feature/agent-system/tool-call/impl-log/0004-e2e-live-tests-with-app.md
  - This log and test documentation.

Test documentation (embedded in test)
- Describes what is being tested and why.
- Clarifies validations vs. non-goals.
- Explains what we learn and why the signal is reliable.
