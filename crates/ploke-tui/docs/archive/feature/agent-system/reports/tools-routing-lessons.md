# Tools Routing — Lessons Learned From Live Tests (August 2025)

Purpose
Distill the practical knowledge we gained while getting reliable tool-calling through OpenRouter, capturing what finally worked, what failed, and how we verified it.

Key Outcomes (What Finally Worked)
- Endpoints-based tools support: We positively detect tool-capable routes by checking whether supported_parameters contains "tools" at:
  - GET /api/v1/models/:author/:slug/endpoints
- Catalog-driven model pick: When no explicit model is provided, we auto-select a tools-capable model from GET /api/v1/models/user by inspecting either:
  - model.supported_parameters or provider.supported_parameters within each entry.
  - During our sessions, deepseek/deepseek-chat-v3.1 was selected and consistently produced tool_calls.
- Headers and timeouts: Using a reqwest client with:
  - 45s timeout and default headers:
    - HTTP-Referer: https://github.com/ploke-ai/ploke
    - X-Title: Ploke TUI Tests
  - This surfaced actionable errors (e.g., 404, 400) instead of hanging.
- Provider preferences on chat/completions:
  - provider.order is accepted (e.g., {"order":["openai"]}).
  - provider.allow on chat/completions is rejected with a 400 (Unrecognized key(s) in object: 'allow').
- Forced tool choice is honored when the selected route supports tools:
  - With a simple function schema (e.g., "search_workspace") and tool_choice forcing that function, we observed finish_reason = "tool_calls" and non-empty message.tool_calls on many models/routes.
- Rate limits and variability:
  - We encountered 429 Too Many Requests occasionally; retries or backoff may be required for robust suites.
  - Some prompts/tasks do not trigger tools even when available, which is expected behavior.

Validated Observations From Logs
- Endpoints probe for qwen/qwen-2.5-72b-instruct returned 6 endpoints; 2 advertised tools via supported_parameters.
- Provider preferences experiment:
  - A: provider omitted → 200 OK
  - B: provider.order=["openai"] → 200 OK
  - C: provider.allow=["openai"] → 400 Bad Request
- Tools smoke for a model without tools route → 404 Not Found with message: "No endpoints found that support tool use."
- Tools success matrix summary example run:
  - total=36, success=26, failure=10
  - force_search_workspace -> used_tool 12/18 (forced cases)
  - Takeaway: even with forced tool_choice, behavior varies by model/provider/prompt.

How We Select a Model That Can Use Tools
- If PLOKE_MODEL_ID is set, we first try to confirm tools capability via endpoints (supported_parameters includes "tools").
- Else, we scan /models/user and pick the first model whose model-level or provider-level supported_parameters indicates tools support.
- Fallback: use google/gemini-2.0-flash-001 when no catalog entry can be confirmed.

Error Shapes Worth Remembering
- Toolless routing: 404 Not Found with message about no endpoints supporting tool use.
- provider.allow on chat/completions: 400 Bad Request (Unrecognized key(s) in object: 'allow').
- Rate limiting: 429 Too Many Requests.

Recommended Testing Approach Going Forward
- Keep a fast “touchpoints” test that tries a handful of models (not a full matrix) and records whether tool_calls were observed.
- Preserve body/headers and a human-readable summary in logs/ for quick inspection (we already write *_latest.json).
- Only fail hard when the intent is to verify an invariant (e.g., a specific forced-tool path for a known tools-capable model).

How To Re-Run The Useful Tests
- Tools capability check: openrouter_model_tools_support_check
- Forced choice diagnostic: openrouter_tools_forced_choice_diagnostics
- Small set smoke across models: openrouter_tools_model_touchpoints (new)
- Full-ish matrix (slow): openrouter_tools_success_matrix

Environment
- Requires OPENROUTER_API_KEY.
- Optional PLOKE_MODEL_ID=<author>/<slug> to steer model selection.

Next Steps
- Wire real tool registry schemas instead of synthetic placeholders.
- Add basic backoff for 429 responses in the live tests.
- Persist provider and endpoint chosen in each run to improve forensics across providers.
- Optionally assert minimum acceptable tool_call rate per-model once baselines are consistent.
