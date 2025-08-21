# Implementation log 024 — Reassessment of tool-calling approach and corrective plan (2025-08-21)

Summary
- Tool use frequently fails due to routing to endpoints without tool support and inadequate enforcement of user intent when tools are required.
- Removal of provider preferences eliminated our ability to steer OpenRouter toward tool-capable endpoints.
- CLI provider selection does not currently translate into routing decisions.
- Tests do not cover the end-to-end path and cannot reveal these issues early.

Key Findings
- The system retries without tools after receiving "No endpoints found that support tool use.", contradicting `tools-only` enforcement.
- Provider configs show `provider_slug: "-"` and `id` equal to the model id, indicating placeholder data and miswiring.
- No client timeout leads to long idle periods with poor diagnostics.

Decisions
- Reintroduce a supported provider preference or an equivalent deterministic routing mechanism.
- If `require_tool_support` is true, disable fallback-to-no-tools; fail fast with clear remediation steps.
- Ensure ProviderSelect configures routing in a way that the network request honors the chosen upstream provider.

Planned Changes
- Routing
  - Validate the accepted provider preference shape for OpenRouter chat/completions; implement the minimal supported shape (likely `provider: { order: ["<slug>"] }`).
  - Enforce that `provider_slug` values are actual slugs; reject placeholder values.
  - Wire `provider select` to update effective routing parameters.

- Enforcement and UX
  - When `require_tool_support` and endpoint rejects tools → fail fast with a message listing viable endpoints and a suggested command sequence to switch.

- Resilience
  - Add a 45s reqwest client timeout for chat/completions and clearer timeout errors.

- Testing
  - Add an ignored, live smoke test that calls OpenRouter with a simple tool and asserts `tool_calls` presence.
  - Add wiremock-based tests for payload shape and enforcement policy.

Artifacts
- RCA: docs/feature/agent-system/reports/tool-use-rca-001.md
- Hypotheses/Experiments: docs/feature/agent-system/reports/tool-use-hypotheses-001.md

Risks
- Provider preference shape may differ; we will run targeted probes to confirm.
- Some models might not have any tool-capable endpoints; our UI must communicate that clearly.

Next Steps
- Execute the hypotheses in HYP-001, implement routing + enforcement changes, and land smoke/mocked tests.
- Monitor logs for elimination of 404 tool-unsupported fallback and verify tool_calls appear in responses for supported endpoints.
