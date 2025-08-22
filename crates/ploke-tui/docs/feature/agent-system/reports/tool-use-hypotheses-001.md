# Tool Use — Hypotheses and Experiments (HYP-001)

Purpose
Track concrete hypotheses about failures and validate them through small, targeted experiments.

Hypotheses
1) OpenRouter accepts a limited provider preference shape on chat/completions
   - H1a: "provider": { "order": ["<slug>"] } is accepted and respected.
   - H1b: "allow"/"deny" are rejected on chat/completions; the 400 was correct for those keys.
   - H1c: No provider object is accepted; pinning must be achieved differently.

   Experiment
   - Send 3 minimal requests with identical message/model and vary only provider object:
     - A: provider omitted (control).
     - B: provider: { "order": ["openai"] }.
     - C: provider: { "allow": ["openai"] }.
   - Observe HTTP status and response. Success criteria: B succeeds and demonstrably routes, while C 400s; or all 400 → reject the hypothesis.

2) Endpoint tool support must be actively steered
   - H2: A given model (e.g., qwen/qwen3-32b) has at least one endpoint that supports "tools" and at least one that does not; the router chooses non-tool by default without preferences.

   Experiment
   - Call /models/:author/:slug/endpoints for the chosen model and inspect "supported_parameters".
   - If multiple endpoints exist, select one with "tools" and attempt a tool call with preference pinning (per H1).

3) require_tool_support should block fallback-to-no-tools
   - H3: Disabling tools after a 404 confuses the user and masks misconfiguration.
   - Expected: If policy is ON, failing fast with advisory messaging leads to better UX and quicker remediation.

   Experiment
   - Add a toggleable path in code (feature flag or config) to disable fallback when require_tool_support = true.
   - Compare user flow and turnaround time to fix routing.

4) Client lacks timeouts; responses can hang
   - H4: No reqwest timeout yields long idle periods that look like stalls.
   - Expected: With a 30-60s timeout, we surface actionable errors instead of open-ended waits.

   Experiment
   - Set a 45s timeout on the client and retry the same flow; logs should show a timeout error instead of indefinite idle.

5) CLI ProviderSelect does not affect routing
   - H5: The selection path writes a placeholder slug ("-") and does not translate to a runtime routing hint.
   - Expected: Fixing this path to set either a valid slug or a dedicated ProviderConfig that changes request behavior should change results.

   Experiment
   - Instrument state to log the effective routing parameters (model, base_url, provider preference).
   - Run end-to-end after selecting a known provider slug (e.g., "openai"); verify these parameters propagate into the request payload.

Testing Plan (Incremental)
- Phase A: Live, manual probes behind a small CLI/dev tool to validate H1 quickly (no code churn).
- Phase B: Introduce an ignored cargo test (requires OPENROUTER_API_KEY) that:
  - Selects a model known to support tools (via docs or endpoints API).
  - Builds a minimal tool schema and expects tool_calls in the response.
- Phase C: Add wiremock-based tests to assert request payloads and our fallback/enforcement policy logic.
- Phase D: CI gating on mocked tests; live tests are developer-run only.

Exit Criteria
- We can deterministically select a provider endpoint that supports tools for a given model.
- When policy requires tool support, we never downgrade silently to no-tools.
- Tests cover both the live smoke path (ignored by default) and mocked payload/policy logic.

Learnings So Far (from exec_live_tests)
- Endpoints parsing now succeeds; pricing fields come back as strings in OpenRouter JSON but deserialize cleanly via string-or-number helpers into f64.
- Duplicate global tracing init can happen in multi-test runs; switching to try_init avoids SetGlobalDefaultError panics.
- Basic provider preference experiment ran without client-side failures. Additional runs are needed to verify routing enforcement across providers/models.
- Tool smoke test returns structured responses and captures headers/body to logs/, providing artifacts for error forensics.

Planned Test Matrix (Tools)
We will record frequency of tool_calls across the following axes:
- System prompts:
  - neutral: “You are a helpful assistant.”
  - prefer-tools: “Prefer calling a tool when available.”
  - tools-only: “Use tools exclusively; no natural language unless tool results are present.”
- User tasks:
  - code-search: “Find code that mentions ‘serde_json::from_str’...”
  - knowledge: “What is the weather in Paris?” (no matching tool available)
  - repo-search: “Search the workspace for references to trait implementations of Iterator.”
- Tool choice:
  - "auto"
  - force a specific function
- Provider preference:
  - none
  - provider.order=["openai"] (example)

Success criteria:
- tool_calls array present and references one of our offered functions.
- Aggregate success rate reported to logs and saved as JSON for analysis.

Next:
- Wire actual tool schemas from our registry to replace placeholders.
- Add optional second-leg call (posting tool results) and measure final-answer quality separately from tool_call frequency.
