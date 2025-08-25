E2E OpenRouter Tools Report (v2 Diagnostics)

Executive summary (failure report)
- Root cause of the prior "no tool calls" run: we never dispatched an LLM Request event for any model; only PromptConstructed events were emitted. The LLM manager only processes pending requests when a matching PromptConstructed arrives, so sending PromptConstructed alone was a no-op.
- Observability gaps: no persisted artifacts of the outgoing request plan, no persisted model outputs, and logs depended on the global tracing setup, making postmortem analysis cumbersome.

What changed in this revision
- Deterministic diagnostics directory:
  - A new env variable PLOKE_E2E_DIAG_DIR controls where artifacts are written. The test sets this to target/test-output/openrouter_e2e.
  - The LLM layer writes request/decision/response artifacts there for every attempt.
- Persisted artifacts for offline analysis:
  - <ts>-<parent_id>-request.json: includes model/provider info, tool capability decisions, parameters, messages, and tool definitions included.
  - <ts>-<parent_id>-decision.json: written when enforcement prevents a call (e.g., tools required but model not marked tool-capable), explaining why.
  - <ts>-<parent_id>-response.txt: raw assistant text for successful calls.
  - <ts>-<parent_id>-error.json: structured error for failed calls.
  - <ts>-<parent_id>-toolcall.json: observed tool-call envelope (name, arguments, vendor).
  - openrouter_tools_candidates.json: a map of tools-capable endpoints discovered for each model, saved at the end of the test.
- Console-first, terse status:
  - The test prints short [E2E] lines showing per-model outcomes (provider hint, whether a tool was observed, and short excerpts of responses or completions).
  - We intentionally avoid the usual tracing initializer for this test to keep output shareable.
- Proactive request dispatch:
  - For each tools-capable endpoint, the test now sends both the LLM Request and the matching PromptConstructed event (in that order) so the LLM manager proceeds.
  - Active provider selection is applied before dispatch when a provider slug hint is available.

How to read the artifacts
- Navigate to target/test-output/openrouter_e2e.
- For a given parent_id:
  - Check the request.json: verify the messages, tools array, and parameters (including tool-related flags).
  - If no response.txt exists and error.json is present, open the error and the decision.json to see what blocked the call.
  - If toolcall.json exists, examine its arguments field to confirm the tool schema interoperability.
- openrouter_tools_candidates.json summarizes which endpoints claimed tool support for each model ID.

Remaining limitations
- The exact wire payload used by the HTTP client may differ slightly from request.json because the final assembly occurs in RequestSession. We capture a faithful "request plan" (messages, tools, parameters, provider) at the point of dispatch.
- Tool enforcement policy and capability cache still gate whether tools are included; the artifacts now make such decisions explicit.

Next steps
- Use the saved artifacts to verify:
  1) The tools array is present and correctly formed for tool-capable models.
  2) The messages reflect your intended system and user prompts.
  3) The decision.json does not block requests unexpectedly (e.g., tools-required policy).
- If a specific model still never emits tool calls, share its request.json and toolcall.json (if any) for deeper analysis.

Paths
- Artifacts directory: target/test-output/openrouter_e2e
- Tools-capable endpoints snapshot: target/test-output/openrouter_e2e/openrouter_tools_candidates.json
