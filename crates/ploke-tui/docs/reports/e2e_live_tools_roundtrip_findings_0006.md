# E2E Live Tools Roundtrip — Findings and Next Steps (Report 0006)

Date: 2025-08-24

Scope
- Test run: cargo test -p ploke-tui -- --test e2e_openrouter_tools_with_app_and_db
- Live endpoint: OpenRouter chat/completions
- Tools exercised: request_code_context, get_file_metadata, apply_code_edit
- DB: Pre-loaded fixture_nodes backup; BM25 rebuilt; RAG used for request_code_context
- Behavior: Force tool_choice; soft-skip when provider declines tools; log diagnostics; two-leg cycle when tool_calls returned

Key observations from logs
- Models/endpoints are heterogeneous:
  - deepseek/deepseek-chat-v3.1 (provider: Chutes): request_code_context returned 200 OK but no tool_calls (provider ignored tool_choice); get_file_metadata succeeded (tool_calls + second leg 200 OK); apply_code_edit declined (no tool_calls).
  - deepseek/deepseek-v3.1-base: no tools-capable endpoints; skipped.
  - qwen/qwen3-30b-a3b-instruct-2507: no tools-capable endpoints; skipped.
  - z-ai/glm-4.5 (Chutes): request_code_context: 200 OK -> second leg 200 OK with summarized content; get_file_metadata: OK; apply_code_edit: no tool_calls.
  - z-ai/glm-4.5-air:free (Z.AI): 404 “No endpoints support tool use” for all tools; soft-skipped as designed.
  - qwen/qwen3-235b-a22b-thinking-2507 (Chutes): 200 OK with no tool_calls for multiple tools; soft-skipped.
  - qwen/qwen3-coder:free (Chutes): later responses returned 429; two-leg follow-up for a previous request_code_context also hit 429; soft-skipped.
- RAG path: request_code_context invoked RagService::get_context with BM25 rebuild and produced assembled context stats; DB path resolution attempted per part without panicking.
- Test resilience: No panic on missing tool_calls or provider error bodies; warnings emitted; entire test completed OK in ~66 seconds.

What we verified
- Connectivity and request construction:
  - OpenAI-compatible payloads with tools and tool_choice are accepted by multiple endpoints.
  - provider.order hinting is included when known.
- Tool interoperability (partial):
  - Some endpoints return tool_calls and accept the second-leg tool result.
  - get_file_metadata commonly succeeds; request_code_context sometimes succeeds; apply_code_edit frequently declined (no tool_calls).
- RAG integration:
  - RagService works over pre-loaded DB; BM25 rebuild runs; assembled context stats serialized and handed to the second leg.
- Robust handling of provider variability:
  - 404 “no tools support” soft-skipped.
  - 429 rate-limit responses are logged and do not fail the suite.
  - Endpoints advertising tools may still ignore tool_choice; treated as soft-skip.

What we did not verify
- Internal tool dispatch/event loop:
  - Tests do not run the in-app llm_manager or rag::dispatcher; tool execution is performed inline by the test harness.
- Observability persistence:
  - No assertions that ToolCallReq/Done or model/provider_slug are persisted.
- Semantic correctness:
  - No assertions on assistant completion content quality or specific JSON shape beyond parsing.
- Determinism and coverage:
  - No allowlist of stable models/providers; behavior varies by provider; success is opportunistic.
- apply_code_edit operational fidelity:
  - Only temp files edited; no end-to-end staging/approval paths exercised or verified against observability.

Accuracy of the comment in tests/e2e_openrouter_tools_app.rs
- The header claims:
  - “A real App initialization (test_harness)” — Not accurate currently. The test harness is not used (cfg(test) gated; integration test can’t import it). We simulate environment but do not initialize the full App state.
  - “Provider returns tool_calls when forced” — Too strong. Some endpoints ignore tool_choice despite tool capability; we changed the test to soft-skip rather than assert.
  - “Second leg completes successfully” — True only when the first leg returns tool_calls; otherwise skipped. Also subject to rate-limit (429).
  - “RAG builds typed context over a pre-loaded DB” — Accurate; we do invoke RagService::get_context and serialize context stats.

Gaps and risks
- Provider/tool heterogeneity:
  - Forced tool_choice does not guarantee tool_calls even for tools-capable models/endpoints.
  - Some endpoints offer tools but still reply with assistant text; some give 404 error bodies; some rate-limit (429).
- Weak assertions:
  - The test currently logs and continues; it does not assert minimal success criteria (e.g., “at least one full round-trip per run”).
- Missing metrics/diagnostics:
  - We don’t summarize success/failure by tool/provider; no structured aggregate metrics are persisted.
- No observability checks:
  - We don’t verify that tool lifecycle events are persisted with model and provider_slug fields.
- apply_code_edit:
  - Often declined by providers; schema, safety, and policy are likely factors. Not a reliable E2E candidate across providers.

What is tested well
- Network resilience and provider variability are explored and tolerated with soft-skips.
- RAG pipeline over pre-loaded DB with BM25 rebuild is exercised successfully.
- Two-leg tool roundtrip is validated when tool_calls are actually returned (positive cases observed).
- Backoff for 429 is in place; 404 tool unsupported is handled without failure.

What is tested poorly or not at all
- End-to-end integration with the in-app event loops (llm_manager, rag::dispatcher) and observability persistence.
- Deterministic success criteria and minimal guarantees (e.g., at least one model must produce a valid tool roundtrip).
- apply_code_edit viability in live settings (providers often decline edits; no alternate provider selection strategy).
- Provider-specific behavior reporting (no structured per-provider/per-tool outcomes that would guide policy).

Actionable next steps
1) Introduce minimal success criteria and per-run summary
- Assert that at least one model/provider produced a full tool roundtrip for request_code_context or get_file_metadata.
- Emit a final structured summary (counts by tool: success, no-tool-calls, 404, 429).

2) Add an allowlist of known stable tools-capable endpoints
- Environment-driven list (e.g., PLOKE_LIVE_MODEL_ALLOWLIST) to focus on providers where tool_calls are reliably returned.
- Keep discovery mode as an option, but use allowlist by default in CI for stability.

3) Capture structured diagnostics
- Log provider_slug, model_id, status_code, and a short body excerpt.
- Optionally persist a JSON artifact for post-run analysis (under target/).

4) Validate minimal JSON contract for request_code_context result
- Ensure the second-leg content acknowledges the tool result fields (e.g., parts count, token stats) or return a typed echo payload.
- Keep assertions lenient (presence-only) to accommodate provider differences.

5) Wire observability checks (follow-up test)
- After we integrate an in-test loop with llm_manager and rag::dispatcher, assert ToolCallReq/Done persistence and captured model/provider_slug fields.

6) Update test header comment for accuracy
- Note that a real App harness is not currently used.
- Clarify that providers may ignore tool_choice; test implements soft-skip.

Key takeaways to carry forward
- Provider behavior varies widely: tools capability, policy, and rate limits differ; soft-skips and allowlists are essential.
- RAG path over a pre-loaded DB works well in tests and should be the first target for stronger assertions.
- apply_code_edit is not a reliable cross-provider E2E signal yet; use get_file_metadata and request_code_context for baseline guarantees.
- Observability and in-app integration are the next value-adds once the E2E baseline is stable.
- Maintain clear diagnostics and summaries to guide provider selection and policy tuning over time.

Proposed small improvements to the current test (non-breaking)
- Track counters for each tool outcome and log a final summary at the end.
- Gate “skip on missing tool_calls” with an env flag allowing strict mode for local runs.
- Reduce flakiness with a lower default for PLOKE_LIVE_MAX_MODELS and a default allowlist for CI.

Files to consider for upcoming work (not modified here)
- crates/ploke-tui/tests/e2e_openrouter_tools_app.rs
  - Add per-run summary and minimal success criteria (optional strict mode).
  - Update header comment for accuracy.
- crates/ploke-tui/src/observability.rs and ploke-db observability store
  - Persist provider_slug, model, error category; add assertions in a follow-up test.
- crates/ploke-tui/src/llm/session.rs
  - Future: configurable retry-without-tools policy injection for parity with app behavior.
- crates/ploke-tui/src/test_harness.rs
  - Optionally re-export under a feature for use in integration tests, or mirror minimal init in-test.
