# OpenRouter Live Tools Routing Test Report — 2025-08-22

Scope
Summarize the outcomes from the latest live test runs against OpenRouter, focusing on tool-calling reliability, endpoint/tool support signals, and provider routing behaviors.

Test Invocations
- cargo test -p ploke-tui openrouter_tools_model_touchpoints -- --nocapture
- cargo test -p ploke-tui exec_live_tests -- --nocapture

Environment + Client
- Base URL: https://openrouter.ai/api/v1
- Headers: HTTP-Referer=https://github.com/ploke-ai/ploke, X-Title=Ploke TUI Tests
- Timeout: 45s reqwest client across tests
- Selection: Auto-picks a tools-capable model from /models/user when PLOKE_MODEL_ID is not set

High-Level Result
- All tests in the exec_live_tests group passed (8/8).
- Touchpoints test passed (1/1).
- Tool use is reliable on many routes; variability remains across prompts/providers.

Key Findings

1) Endpoints tool support is the ground truth
- openrouter_model_tools_support_check on qwen/qwen-2.5-72b-instruct:
  - endpoints total: 6
  - tools-capable: 2 (supported_parameters includes "tools")
- Additional probe later in the suite showed another model with endpoints total=8, tools-capable=3.
- Takeaway: Not all provider endpoints for a model support tools. Tests must confirm endpoint tool support before asserting tool behavior.

2) Provider preferences on chat/completions
- A: provider omitted → 200 OK
- B: provider.order=["openai"] → 200 OK
- C: provider.allow=["openai"] → 400 Bad Request ("Unrecognized key(s) in object: 'allow'")
- Takeaway: order appears accepted on chat/completions; allow is rejected for this API. Keep order for routing hints; do not send allow on chat/completions.

3) Forced tool choice works when the selected route supports tools
- openrouter_tools_forced_choice_diagnostics:
  - Status: 200 OK
  - finish_reason: tool_calls
  - used_tool: true
  - Body saved to logs/tools_forced_diag_latest.json
- Takeaway: With a tools-capable route and a minimal function schema, forced tool_choice produces tool_calls as expected.

4) Quick model touchpoints (fast cross-section of models)
- Chosen primary (auto): deepseek/deepseek-chat-v3.1 → 200 OK, used_tool=true, finish_reason=tool_calls
- google/gemini-2.0-flash-001 → 200 OK, used_tool=true, native_finish_reason=STOP
- qwen/qwen-2.5-72b-instruct → 404 Not Found (error: "No endpoints found that support tool use")
- anthropic/claude-3.5-sonnet → 200 OK, used_tool=true, finish_reason=tool_calls
- Summary: 3/4 models successfully produced tool_calls; the qwen variant hit a toolless route (consistent with endpoints mix).

5) Tools success matrix (small matrix across prompts/prefs)
- totals: 36 cases, success=27, failure=9
- forced tool cases (force_search_workspace): 13/18 used_tool
- Observations:
  - Repo/code prompts frequently trigger tool_calls.
  - Weather-style prompts often complete with "stop" (no tools) even when tools available, which is expected task-dependent behavior.
  - provider.order did not degrade tool behavior; many cases still produced tool_calls.

6) Error shapes captured and confirmed
- 404 Not Found: "No endpoints found that support tool use" for toolless routes
- 400 Bad Request: provider.allow on chat/completions
- Occasional 429 in earlier runs (not prominent in this run); backoff remains advisable.

Concrete Artifacts (latest aliases)
- Endpoints: logs/openrouter_endpoints_latest.json
- Model tool support check: logs/model_tools_support_check_latest.json
- Forced tool-choice diag: logs/tools_forced_diag_latest.json
- Provider preference experiment: logs/provider_pref_{A__omitted|B__order|C__allow}_latest.json
- Touchpoints: logs/tools_model_touchpoints_latest.json and logs/tools_touchpoint_<model>_latest.json
- Matrix summary: logs/tools_success_matrix_latest.json

Interpretation and Guidance

- Model selection:
  - Prefer models that positively advertise "tools" at /models/:author/:slug/endpoints.
  - If a particular model inconsistently produces tool_calls, verify endpoint selection and consider pinning to a provider endpoint with tools support.

- Provider routing:
  - Use provider.order for steering; do not use provider.allow on chat/completions (results in 400).
  - Record the resulting provider name slug from /providers when possible for forensics.

- Prompt/task shaping:
  - Tool usage is task dependent. Prompts that clearly require structured lookup/search are more likely to trigger tool_calls.
  - "Prefer tools" style system prompts help but do not guarantee tool usage for all tasks.

- Test strategy:
  - Keep a fast touchpoints test over 3–4 representative models.
  - Maintain a matrix run that saves detailed per-case bodies for regression forensics.
  - Fail hard only on invariant checks (e.g., forced tool path for a known tools-capable route).

Action Items

- Productizing:
  - Add backoff/retry for 429 responses in live tests.
  - Expose endpoint pinning in the UI/CLI and persist chosen provider slug when known.
  - Surface the provider/endpoint actually used in responses for better postmortems.

- Testing:
  - Integrate real registry tools in place of synthetic schemas.
  - Track and stabilize a per-model baseline: minimally acceptable tool_call rate by task class.
  - Add a preflight endpoint check in the agent path; if require_tool_support is ON and no tools-supporting endpoints exist, fail-fast with clear advice.

Appendix — Selected Line Items

- qwen/qwen-2.5-72b-instruct endpoints: total=6, tools_capable=2
- provider prefs:
  - A: 200 OK
  - B: 200 OK (order)
  - C: 400 Bad Request (allow)
- forced tool choice (deepseek-chat-v3.1): 200 OK, used_tool=true, finish_reason=tool_calls
- touchpoints used_tool summary: 3/4 models
- matrix summary: 36 cases, success=27, failure=9; forced used_tool=13/18
