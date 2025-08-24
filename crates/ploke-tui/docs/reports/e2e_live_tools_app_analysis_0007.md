# E2E Live Tools App — Analysis and Recommendations (Report 0007)

Date: 2025-08-24

Summary
- The e2e_openrouter_tools_app test successfully executed full two-leg tool cycles for some endpoints and tools (notably request_code_context and get_file_metadata).
- Many providers/endpoints ignored forced tool_choice for apply_code_edit and sometimes for other tools; responses were 200 OK with no tool_calls, 404 “No endpoints support tool use,” or 429 rate limits.
- The test now treats “no tool_calls” as a soft skip and logs the body excerpt, which is appropriate for provider heterogeneity.
- Logging is informative for per-request behavior but lacks a final roll-up summary and coarse-grained success criteria; we add these below.

What we learn from the run
- Successes
  - Live OpenRouter requests with tools and explicit tool_choice are well-formed and accepted by multiple endpoints.
  - For request_code_context, the second leg completed 200 OK with a reasonable natural language summary (indicating that the model recognized and utilized the tool result).
  - get_file_metadata often completed both legs successfully and provided coherent assistant content.
  - RAG pathway works: BM25 rebuild runs, get_context is invoked, and assembled context is serialized.
- Failures/variability
  - apply_code_edit frequently returned no tool_calls (policy/safety skepticism from providers). Even with plausible arguments, endpoints declined to call the tool.
  - Some endpoints ignored tool_choice for request_code_context or get_file_metadata, returning plain assistant text or “thinking” content instead of tool_calls.
  - Some endpoints returned 404 “no tools” for specific providers; rate-limits (429) also occurred.
- Policy implications
  - Forced tool_choice does not guarantee tool_calls across providers. Soft-skip behavior is necessary. An allowlist and/or provider.order hint improves outcomes but is not sufficient universally.

Are we using the OpenRouter tool-calling API correctly?
- According to docs (docs/openrouter/tool_calling.md), best practices include:
  - Include tools in both the first and second requests: Achieved.
  - Use tool_choice to force a specific tool: Achieved (we send {"type":"function","function":{"name":...}}).
  - Provide provider.order when possible: Achieved (we include when we have a slug).
  - Return a “tool” role message in the second leg with the tool’s results: Achieved.
  - Accept provider variability (404, 429, policy declines): The test implements a soft skip for no tool_calls and logs details. Good.
- Conclusion: The request construction and two-leg flow follow the documented API patterns.

Is tool_call_flow.md the right plan?
- Yes, broadly:
  - Strongly typed tool IO with serde is the right direction (already implemented for core types and tested via serde_roundtrip tests).
  - RAG-first for request_code_context with AssembledContext is correct and observed to work.
  - Configurable fallback for unsupported tools aligns with real-world endpoint behavior (recommended to expose this as a knob).
  - Observability and structured summaries are needed; this test lacked a final roll-up (we add it here).
- Gaps:
  - apply_code_edit needs a provider strategy or allowlist for E2E runs; it is often declined.
  - Add minimal success criteria and summaries per run to avoid silent partial success.

Prompts and returned values
- Prompts
  - The system prompt is minimal and correct: “Prefer calling a tool when one is available.”
  - The user prompt explicitly requests calling a specific tool with provided arguments and waiting for results; this helps ensure consistent tool invocation for cooperative providers.
- Returned values
  - We pass structured JSON back to the model in the tool role message; providers that do a proper second leg produced coherent assistant output.
  - For request_code_context, the second leg content occasionally read like a summary of metadata rather than showing code. This is expected since we currently send typed fields; moving toward a typed echo (AssembledContext) content is recommended so the model can read the code payload more directly.

Logging quality
- What’s good
  - Per-call logs capture model, provider (via chosen endpoint name), first-leg status, and whether second leg occurred.
  - Body excerpts for no tool_calls cases are logged, providing actionable context for why providers refused tools.
- What’s missing
  - No top-level summary collating counts by outcome (success, no tool_calls, 404, 429).
  - Minimal structured signal across the entire run (e.g., successes per tool, failures per provider).
- Improvements: We add a summary roll-up (totals, successes, no_tool_calls, 404s, 429s) at the end of the test run.

Biggest drawbacks and failures
- apply_code_edit is not a reliable E2E tool across providers (policy safety, schema, and content sensitivity).
- The test measures viability on an opportunistic basis without minimal success criteria (e.g., “at least one tool completes a full two-leg round-trip”).
- No structured aggregate metrics; diagnosis is per-request, which is fine for investigations but not for pass/fail policy or trend analysis.

Criteria for evaluating test quality
- Request construction correctness
  - Tools present in both legs; forced tool_choice; provider.order applied where known.
- Provider compatibility coverage
  - Chosen endpoints have tools capability; selection favors lower price. Soft-skip pathways for 404 and no tool_calls are in place.
- End-to-end cycle success
  - At least one tool achieves a two-leg cycle (preferably request_code_context or get_file_metadata).
- Logging and diagnostics
  - Summary counts per run: successes, no tool_calls, 404, 429.
  - Body excerpts for failure/soft-skip paths.
- Determinism for CI
  - Optional allowlist of stable tools-capable endpoints via env; cap processed models; short timeouts.

Assessment of current test against criteria
- Request correctness: Meets criteria.
- Provider coverage: Good but partly opportunistic; selection logic is sound.
- E2E success: Achieved for some tools and models (e.g., request_code_context and get_file_metadata) in the provided run.
- Logging: Good per-call logs, missing final summary (added below).
- Determinism: Reasonable caps; recommend allowlist for CI stability.

Actionable improvements
1) Summarize outcomes across the run
- Count successes (tool_called true and second leg 2xx), no tool_calls, 404, and 429 across all tools/models; log a summary.
- Implemented in this change.

2) Minimal success criteria (follow-up)
- Optional assertion: require at least one full round-trip for request_code_context or get_file_metadata.
- Gate with env (e.g., PLOKE_LIVE_REQUIRE_SUCCESS=1) to keep CI lenient by default.

3) Allowlist for stable endpoints
- Use env (PLOKE_LIVE_MODEL_ALLOWLIST) to focus on known tools-capable providers for CI predictability.

4) apply_code_edit strategy
- Treat as diagnostic by default; exclude from required-success criteria until we have a dependable provider.
- Consider provider-specific prompt hints or separate schema shape for safety acceptance.

5) Typed content echo
- For request_code_context, consider returning a typed JSON echo of AssembledContext content to encourage models to cite specific snippets in the second leg.

6) Observability (later)
- Persist provider_slug, model_id, outcome category, and body excerpt digest for longitudinal analysis.

Relevant OpenRouter doc points (tool_calling.md)
- Include tools in every request (Steps 1 and 3): Done.
- Use tool_choice to force a specific tool: Done.
- Validate tool schema on each call: Our tool def JSON is built from typed schemas; OK.
- Expect provider variability: Some providers ignore tool_choice or do not support tools; soft-skip is appropriate.
- Tool results must be provided as a tool role message in second leg: Done.

Conclusion
- The test is on the right path: it exercises realistic behavior, tolerates provider variability, and confirms our request/response handling for cooperating endpoints.
- Adding a summary roll-up and optional minimal success criterion will improve signal and CI utility.
- apply_code_edit remains unreliable cross-provider; emphasize request_code_context and get_file_metadata for baseline E2E guarantees.
