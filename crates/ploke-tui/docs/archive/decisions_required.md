# Decisions Required

Context: Advancing tool-call reliability and observability in live E2E tests and production.

1) Tool choice policy surface
- Question: Should we support tool_choice = "required" as a first-class toggle in LLMParameters?
- Options:
  - A) Keep "auto" only (today)
  - B) Add an enum ToolChoice { Auto, Required } with default Auto; tests/apps can select Required
- Recommendation: B (default Auto). This reduces test flakiness for tools and improves determinism.

2) Golden model for tooling E2E
- Question: Should CI pin a known tools-capable model for the live test?
- Options:
  - A) Heuristic discovery only (cheapest tools-capable endpoint)
  - B) Allow override via env PLOKE_E2E_GOLDEN_MODEL; fall back to heuristic
- Recommendation: B

3) Telemetry persistence in tests
- Question: Should we persist tool-call telemetry to DB during tests for richer assertions?
- Options:
  - A) Keep file artifacts only
  - B) Add DB rows for ToolCallReq/Done and ConversationTurn; assert counts by request_id
- Recommendation: B

4) Provider slug enforcement
- Question: Should we require provider_slug pinning for all tools-enabled requests in tests?
- Options:
  - A) Best-effort hint (today)
  - B) Hard requirement in tests; skip when not available
- Recommendation: B

Please review and approve defaults. If accepted, we will create follow-up PR(s) to implement 1–4.
