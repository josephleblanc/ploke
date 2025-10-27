# Tool Use Failure — Root Cause Analysis (RCA-001)

Date: 2025-08-21

Summary
- Tool-calling attempts through OpenRouter regress to non-tool responses or stall after a fallback.
- Our provider selection and routing are not deterministically steering requests to a tool-capable endpoint.
- Tests are synthetic and do not exercise the end-to-end path used by the TUI against the live API.

Observed Symptoms
- First request with tools enabled returns a 404-like error body: "No endpoints found that support tool use."
- Code then retries without tools (fallback), contradicting the user's explicit intent to use tools.
- Long idle (~22s) with no completion; the session appears to stall or complete without tool usage.
- Logs show provider_slug: "-" and a ProviderConfig id equal to the model id (e.g., "qwen/qwen3-32b"), suggesting miswiring between selection UI and runtime routing.

Key Log Evidence
- tool_unsupported_fallback: {... "No endpoints found that support tool use." ...} followed by immediate retry without tools.
- provider_slug is a placeholder "-" and not a real upstream provider (e.g., "openai", "anthropic", "mistralai").
- Request path is always POST {base_url}/chat/completions with model set, but no accepted upstream provider selection mechanism is in play.
- No client-side timeout configured; prolonged idle periods observed.

Likely Root Causes
1) Provider preference removal without viable alternative
   - We removed the provider field (allow/deny/order) because the API 400’d with "Unrecognized key(s) in object: 'allow', 'deny'".
   - Without an accepted way to prefer/pin a specific upstream provider, the router can choose an endpoint with no tool support.
   - Result: 404 "support tool" + fallback disables tools, defeating the use case.

2) Provider selection wiring is not effective
   - Command ProviderSelect passes a provider_id derived from a "provider_slug" token, but state plumbing (not shown here) likely expects an internal id.
   - Logs reveal ProviderConfig { id: "qwen/qwen3-32b", provider_slug: "-" }, which suggests:
     - We created/selected a provider entry whose id == model id and stored a placeholder slug.
     - The selected slug is never used in the request (since provider preferences were removed).
   - Net effect: "provider select" has no influence on which upstream endpoint executes the request.

3) Capability enforcement semantics are inconsistent with user intent
   - When require_tool_support is enabled, we still fall back to non-tool calls after a "no tool support" error.
   - This violates the policy’s intent and hides misconfiguration or routing issues from users.

4) Missing HTTP timeouts and resilience
   - No per-request timeout configured; long idle observed (22s) with ambiguous end state.
   - Streaming is not used; large responses or slow providers can stall the feedback loop.

5) Tests are not representative
   - Unit tests assert static payload shapes; they neither contact the live endpoint nor validate tool behavior.
   - No "smoke" test that exercises: model selection -> tool schema -> actual tool_call in a live or mocked integration scenario.

Contributing Assumptions That Proved False
- "Removing provider preferences entirely is acceptable for OpenRouter chat/completions."
  - False for our use case: tool support is provider-endpoint-dependent; we need a way to steer routing.
- "Model-level 'supports tools' implies all associated endpoints support tools."
  - False. Some endpoints for a model do not support tools; the router may pick those without explicit guidance.
- "CLI provider selection is sufficient even without provider preferences in the request."
  - False. Selection must affect the request routing or the chosen base_url; otherwise it has no effect.
- "Fallback to no-tools is acceptable when user explicitly wants tools."
  - False for enforced mode; this undermines user intent and masks routing problems.

Impact
- Users cannot proceed with agentic workflows relying on tool calls.
- Manual testing cycles consume significant time and do not converge, blocking further progress.

Corrective Actions (Prioritized)
P0 — Restore deterministic provider routing and enforcement
- Reintroduce an accepted provider preference mechanism for OpenRouter chat/completions. If the "allow/deny" keys are rejected, determine the supported shape by:
  - Consulting current OpenRouter docs or
  - Running a targeted live probe (see Hypotheses/Experiments) to find an accepted "provider" structure (e.g., "order": ["openai"]).
- If require_tool_support is true and we receive "no tool support" for endpoint:
  - Do NOT fallback to no-tools; instead, fail fast with a clear message that the selected endpoint cannot perform tool calls.
  - Offer guidance/actions: list available endpoints for the model that support tools; allow switching.

P1 — Make provider selection actually influence routing
- Map CLI "provider select <model_id> <provider_slug>" to a ProviderConfig that either:
  - sets a provider preference in the request body, or
  - selects a specific, preconfigured ProviderConfig whose base_url or routing guarantees the upstream provider.
- Eliminate placeholder slugs like "-"; validate against known provider slugs or a discovery endpoint (/providers).
- When selections are ambiguous, present a short list of viable options (those supporting tools).

P1 — Apply client timeouts and robust error handling
- Configure reqwest::Client with a sane timeout (e.g., 30-60 seconds) for chat/completions.
- Emit clear diagnostics on timeout vs server errors.

P2 — Introduce end-to-end smoke tests
- Add an opt-in, ignored test that:
  - Requires OPENROUTER_API_KEY and a known model with tools.
  - Sends a minimal tool schema and verifies a tool_calls finish_reason or tool_calls array.
  - Fails fast if the provider routing rejects tool use.
- Add a mocked integration with WireMock to validate payload shapes for tool usage and policy logic without live calls.

Confidence Assessment
- Confidence to resolve with the above steps: Medium-High (0.75).
- Main risk is the exact provider preference shape accepted by OpenRouter for chat/completions; once established, the rest is straightforward.
- With E2E smoke tests and timeouts, regressions should become visible early.

Action Items Checklist
- [ ] Determine correct provider preference shape accepted by OpenRouter chat/completions.
- [ ] Wire provider selection so it impacts routing deterministically (no placeholder slugs).
- [ ] Enforce require_tool_support by failing fast on non-tool endpoints.
- [ ] Add request timeout and improve error reporting.
- [ ] Implement live (ignored) and mocked integration tests that exercise tool calls end-to-end from the TUI path.

Appendix — Why we failed repeatedly
- We optimized for removing 400 errors (by dropping provider preferences) but inadvertently removed our ability to steer routing, which is necessary for tools.
- The provider selection UI and CLI were added, but the data carried (provider_slug) was not wired into the request (due to removal) and in some cases stored as a placeholder.
- Without a live smoke test, we could not validate the real, end-to-end behavior.
