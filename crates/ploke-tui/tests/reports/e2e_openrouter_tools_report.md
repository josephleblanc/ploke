E2E OpenRouter Tools Report

Summary
- The test completed successfully but did not observe any tool-call executions.
- Total outcomes reported by the test: total_outcomes=82, successes=0, no_tool_calls=82, http_404_first_leg=0, http_429_any_leg=0.
- The run took a long time (approximately 429s) due to a 10-second observation loop for many models and a missing increment of the processed counter, which bypassed the intended PLOKE_LIVE_MAX_MODELS cap.

Where was the data saved for the LLM endpoints with tool call support?
- We cached tools-capable endpoints in-memory only:
  - A lazy_static Mutex<HashMap<String, Vec<String>>> named TOOL_ENDPOINT_CANDIDATES inside tests/e2e_openrouter_tools_app.rs.
  - This cache is process-local and not persisted to disk.
  - Visibility of the data was via tracing logs like:
    tools-capable endpoints cached for <model_id>: <provider1 | endpoint1>, <provider2 | endpoint2>, ...
- No file or database persistence is implemented for these candidates yet.

What worked
- Provider discovery and endpoint filtering:
  - We correctly called GET /models/<author>/<slug>/endpoints, filtered by supported_parameters containing tools and tool_choice, and sorted candidates by a price hint (prompt + completion).
  - We derived a conservative provider_slug fallback when a direct mapping was not found and logged it.
  - We cached the discovered tools-capable endpoints in the in-memory map for later diagnostics.
- Observability:
  - Tracing instrumentation was added in update_message and used throughout the test for visibility.
  - The test subscribed to the event bus and printed informative logs for LLM/Tool events when they occur.
  - RAG and BM25 rebuild were initialized and completed with logs, demonstrating a realistic environment.
- Robust configuration bootstrap:
  - UserConfig defaults and registry initialization worked (including reading API keys from env).
  - LLM manager and state manager tasks started without runtime errors.

What did not work
- No tool-call lifecycle was triggered:
  - The test never sent an AppEvent::Llm(Event::Request ...), so llm_manager had no actual request to process after PromptConstructed.
  - As a result, prepare_and_run_llm_call was not executed for network calls, and no ToolCall events could be emitted.
- Model capping and test runtime:
  - processed was not incremented; the max_models cap was effectively ignored.
  - Combined with a 10-second observation loop per model, this led to a long test duration.
- Persistence/Artifacts:
  - TOOL_ENDPOINT_CANDIDATES were not persisted to disk for later analysis; only logs captured them.
- Assertions:
  - The test did not assert that tool-calls occurred or that a second leg completed; it only logged observations and produced a summary.

What is verified
- Endpoint discovery and filtering by tool support are functional against live OpenRouter endpoints.
- The event subscription and logging pipeline are working; PromptConstructed events were observed, and llm_manager recorded contexts received.
- The system initializes RAG/BM25 and a realistic AppState without panics.
- Tracing instrumentation provides useful, structured logs.

What is invalidated (or not yet proven)
- We did not verify that any live model actually emitted a tool_call under our test execution path.
- We did not verify the second-leg tool result handling (sending back tool role JSON) via the test.
- We did not verify that selecting an endpoint was honored by the live request path (no request was made).
- Success rate claims for tool-call round trips cannot be made from this run.

How we can improve
- Drive a real tool-call:
  - After choose_tools_endpoint_for_model returns an endpoint and optional provider_slug, dispatch StateCommand::SelectModelProvider to set the active provider and model.
  - Then simulate a user prompt by sending StateCommand::AddUserMessage via cmd_tx. This triggers add_msg_immediate, which emits AppEvent::Llm(Event::Request ...) and starts the real LLM flow.
  - Ensure provider_registry.require_tool_support is set appropriately, and the active provider is marked tool-capable in the registry so tools are actually included in the request payload.
  - Optionally force tool usage by setting tool_choice="required" in llm_params for the active provider when running this test.
- Control runtime:
  - Increment processed on each iteration to respect PLOKE_LIVE_MAX_MODELS.
  - Reduce the observation window (e.g., from 10s to 2–3s) and break early once a ToolEvent::Completed or ToolEvent::Failed is seen.
- Persist diagnostics:
  - At the end of the test, write TOOL_ENDPOINT_CANDIDATES to a JSON file under target/test-output/openrouter_tools_candidates.json or tests/output/ for later inspection and CI artifacts.
- Strengthen assertions:
  - For at least one known tools-capable model, assert that a ToolCall was observed.
  - Assert that we eventually see a ToolEvent::Completed with a non-empty content payload and HTTP 2xx in the second leg.
- Clean warnings:
  - Remove unused imports and variables flagged by cargo fix suggestions.
  - Consider skipping or refactoring other tests with todo!() code that produce unreachable warnings when those tests are included.

Next steps (concrete)
- In the test loop:
  1) Increment processed after each model evaluated.
  2) After selecting an endpoint, send StateCommand::SelectModelProvider with the chosen model and provider_slug.
  3) Send StateCommand::AddUserMessage with a prompt that is likely to trigger the request_code_context tool.
  4) Narrow observation to break when ToolEvent::Requested/Completed are seen or after a shorter timeout.
- Persist TOOL_ENDPOINT_CANDIDATES to a JSON file at test-end so we have a machine-readable artifact of tools-capable endpoints per model.

Appendix: Observed behavior indicators from the run
- Many models reported no tools-capable endpoints (especially :free tiers).
- Where tools-capable endpoints existed, they were successfully cached and logged but not exercised because no live request was made.
- The summary confirmed 0 successes and 82 no_tool_calls, aligning with the lack of real request dispatch.
