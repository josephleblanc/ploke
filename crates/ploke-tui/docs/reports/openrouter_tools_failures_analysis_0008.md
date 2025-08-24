# OpenRouter Tools Failures — Focused Analysis and Recommendations (Report 0008)

Date: 2025-08-24

Summary of observed issues from the latest run
- Models respond as if we asked to modify /etc/hosts.
- Assistant outputs include code fences labeled as go (```go) even though our domain is Rust.
- Many first-leg responses ignore tool_choice and return no tool_calls; some 404s (no tools) and 429s appear.
- Some assistant content summarizes “metadata” rather than returning clear Rust snippets.

Root causes traced in our tests
1) Why does the model say we’re editing /etc/hosts?
   - Because our test explicitly passes apply_code_edit arguments with file_path "/etc/hosts":
     - In tests/e2e_openrouter_tools_app.rs: ace_args uses "/etc/hosts" and an invalid UUID-styled expected_file_hash, which triggers provider safety/policy concerns.
     - Similarly in tests/openrouter_live_tools_roundtrip.rs we also used "/etc/hosts".
   - Even though we execute edits locally on a NamedTempFile, the model only sees the forced tool arguments we send. That is sufficient for a safety refusal or a non-tool response.

2) Why are code fences labeled as go?
   - Our system message doesn’t constrain the language formatting. The assistant chooses formatting heuristically and occasionally labels code as go.
   - We didn’t instruct “All code is Rust; use ```rust fences.” This is a prompting gap.

3) Why are some second-leg responses just metadata summaries?
   - For request_code_context we currently send a minimal JSON (“hint, parts, stats” in one test) or a plain local JSON rather than a typed AssembledContext payload with the actual code text.
   - The LLM summarizes what it can see; absent explicit snippets, it paraphrases metadata.

Other anomalies and concerns
- Misleading file semantics to the model:
  - Using a system path (/etc/hosts) and a UUID instead of a 64-hex hash for expected_file_hash invites refusal and policy gating. Better to use a harmless /tmp path and a hex-looking hash to avoid triggering “UUID != hash” objections.
- Unused field warnings:
  - ToolRoundtripOutcome carries fields (tool_name, model_id, provider_slug, body_excerpt_first) that aren’t used in the final summary; minor, but noisy. We can add #[allow(dead_code)] or log more detail in summaries.
- No explicit language guardrails:
  - Without a strong instruction, providers pick arbitrary code-fence languages based on heuristics.

Answers to prior report questions
- What can we learn from this test?
  - Our request construction and two-leg flow are accepted by multiple endpoints.
  - Provider heterogeneity is the norm; many ignore tool_choice or enforce safety policies.
  - Our RAG path works and assembles context; however, the tool result returned to the model should include actual snippets to improve assistant output usefulness.
- What is going well?
  - Network flow, retries, and soft-skip behavior are good; we now have an outcomes summary that increases observability.
  - RAG service, BM25 rebuild, and context assembly function end-to-end with the DB.
- What is going poorly?
  - apply_code_edit arguments are provoking the models’ safety policies.
  - Some endpoints ignore tool_choice, and our current prompts do not mitigate variability well.
  - Assistant outputs occasionally use incorrect language fences and produce summaries rather than actionable Rust code.
- Improvements to tool call flow
  - Safer arguments and clearer prompts improve odds of tool_calls.
  - Provide typed AssembledContext with code text in tool results, not just stats/metadata.
  - Add minimal success criteria (config-gated) and keep the roll-up summary.
- Is tool_call_flow.md the right plan?
  - Yes, broadly. It advocates strongly typed schemas, RAG-first context, provider-aware fallbacks, and better observability. We need to finish typed IO for request_code_context (return AssembledContext) and refine prompts.
- Is the test achieving the end-to-end goal?
  - Partly. It validates live two-leg cycles with cooperating providers and surfaces heterogeneity. However, success is opportunistic, and apply_code_edit is not reliable cross-provider.
- Are prompts well-constructed?
  - Minimal and correct, but missing guardrails:
    - State that code is Rust and require ```rust fences.
    - Avoid system-file edits; tell the assistant we only operate on ephemeral paths.
- Are returned values to the LLM good?
  - Acceptable structure, but not optimal:
    - Return typed AssembledContext contents to allow the model to cite exact Rust snippets.
- Do we have good logging?
  - Yes for per-call behavior and summary; we could also log finish_reason on leg 1 and capture tool_call function names when present.

Are we using the OpenRouter tool-calling API correctly?
- Yes: We include tools in both legs, use tool_choice to force a tool, add provider.order when possible, and send a tool role message with results. We tolerate provider variability with soft-skips, matching the docs in docs/openrouter/tool_calling.md.

Biggest failures and drawbacks
- Triggering safety filters with /etc/hosts edits and mismatched hash format.
- No explicit language constraint in prompts causes non-Rust code fences.
- Some second-leg content lacks code text because we don’t pass the AssembledContext snippets.
- Lack of minimal success criteria (optionally gated by env) reduces CI signal strength.

Actionable changes made now
- Tests updated to:
  - Replace /etc/hosts with /tmp/ploke_tools_test.txt.
  - Use a valid-looking 64-hex hash placeholder rather than a UUID for expected_file_hash.
  - Provide a plausible byte range (start=0, end=5) and simple replacement to avoid “empty edit” confusion.
  - Strengthen the system prompt to:
    - Prefer tools,
    - Enforce Rust code fences with ```rust,
    - Avoid system file edits, use ephemeral test paths,
    - Avoid fabrications when tools are unavailable.
- Added #[allow(dead_code)] on the ToolRoundtripOutcome struct in e2e_openrouter_tools_app.rs to silence warnings (keeping code concise).

Recommended next steps (follow-ups)
1) Typed context payload:
   - Return RequestCodeContextResult with AssembledContext (including code text) for the tool result, not just stats.
2) Minimal success criteria:
   - Config-gated requirement that at least one model completes a full round-trip for (request_code_context OR get_file_metadata).
3) Provider allowlist:
   - Allow env-driven allowlist to reduce flakiness in CI and focus on stable tool-call providers.
4) Additional logging:
   - Parse and log finish_reason for leg 1; log tool_call function names (if any) for clarity.
5) Observability:
   - Persist outcome categories and provider_slug for trend analysis.

Criteria to judge test quality (applied to this run)
- Request correctness: Good.
- Provider coverage: Good, but opportunistic; add allowlist for CI.
- End-to-end cycles: Present, but not guaranteed; add minimal success criteria.
- Logging and diagnostics: Good per-request + summary; can add finish_reason.
- Determinism: Improve with allowlist and lower model caps for CI.

Conclusion
- The failures are primarily test-input and prompt design issues (system path, hash format, and language fence). We’ve adjusted tests and prompts accordingly. The core tool-calling flow matches OpenRouter guidance. Next we should deliver typed tool outputs with actual code and add minimal success criteria to boost the test’s usefulness in CI and development.
