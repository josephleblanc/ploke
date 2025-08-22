# Testing Guidance: OpenRouter Endpoint and Tool Use

Purpose
Design tests that inform us about the API surface, verify properties we rely on, and catch silent regressions.

Principles
- Informative on failure: print concise diagnostics to stderr (captured by cargo test) and persist full bodies under logs/.
- Grounded in docs: align request shapes with OpenRouter reference; prefer minimal examples that isolate a hypothesis.
- Evidence over speculation: tests should save raw responses for later inspection and reference log paths in assertions.

How to Run Verbosely
- Failure-only logs: cargo test -p ploke-tui
- Always show logs: RUST_LOG=info cargo test -p ploke-tui -- --nocapture

Key Live Tests (require OPENROUTER_API_KEY)
- openrouter_endpoints_live_smoke
  - Validates endpoints schema and saves response for selected model (PLOKE_MODEL_ID overrideable).
- openrouter_model_tools_support_check
  - Lists endpoints and whether they advertise "tools" support; prints provider names and context lengths.
- openrouter_tools_forced_choice_diagnostics
  - Sends a single forced tool_choice request; logs payload, status, finish_reason, tool_calls presence; saves response.
- openrouter_tools_success_matrix
  - Measures tool_call frequency across prompt/task/provider variants; saves an aggregate summary and per-case bodies.
  - On failure, prints summary path and hints for next steps.

Choosing Models
- Auto-detect: tests now probe /models/user and select a model that advertises tools if no explicit choice is provided.
- Override: export PLOKE_MODEL_ID="google/gemini-2.0-flash-001" (or another tools-capable model per endpoints probe).
- Fallback: when catalog detection fails, we fall back to google/gemini-2.0-flash-001.

What to Look For
- finish_reason=tool_calls and presence of message.tool_calls in responses.
- Endpoints advertising tools via supported_parameters; reconcile with observed behavior.
- Provider routing impact (order vs omitted) on tool behavior.

Next Steps
- Replace synthetic tool schemas with our real Tool Registry export.
- Add assertions for minimal acceptable tool_call rates per model once we establish reliable baselines.
- Track native_finish_reason and headers to disambiguate provider behavior on ambiguous failures.
