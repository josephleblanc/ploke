# OpenRouter Tools Roundtrip — Postmortem and Recommendations

Date: 2025-08-22

Executive Summary
- We successfully forced tool calls for two tools on the same provider endpoint:
  - request_code_context → tool_calls observed, round-trip passed.
  - get_file_metadata → tool_calls observed, round-trip passed.
- The third tool, apply_code_edit, did not produce tool_calls. The model returned a natural-language response and hit finish_reason="length".
- Root cause is most likely provider/model policy or mapping heuristics that resist executing file-edit semantics (even with “local execution only” semantics in our test), combined with a relatively complex schema. This is consistent with observed provider variability around tool invocation patterns.

Observed Evidence
- Endpoint chosen: deepseek/deepseek-chat-v3.1 via provider “Chutes | deepseek/deepseek-chat-v3.1”.
- request_code_context: 200 OK; tool_calls present; second-leg completion OK.
- get_file_metadata: 200 OK; tool_calls present; second-leg completion OK.
- apply_code_edit: 200 OK; no tool_calls; assistant wrote explanatory prose; finish_reason="length".
- This indicates the endpoint supports tool calling generally, but the specific tool “apply_code_edit” was not invoked.

Likely Causes
1) Tool semantics sensitivity
   - Tools that imply filesystem modification or privileged actions can be de-emphasized or declined by safety/alignment layers, even if the capability exists.
   - Using a path such as “/etc/hosts” may be perceived as risky/unrealistic and nudge the model to refuse.

2) Schema complexity friction
   - apply_code_edit exposes a nested array with multiple required fields. In practice, some routers/providers favor simpler schemas when learning whether to call a tool.
   - The first two tools have simple, single-object schemas and were accepted readily.

3) Provider-specific routing heuristics
   - Even with tool_choice forcing, some providers may not honor a forced function call in specific contexts or may transform the payload in a way that discourages certain tools.
   - We already set provider.order when a slug is known. Since two tools succeeded, routing isn’t the primary issue, but still a contributing factor.

4) Prompt framing and token budget
   - The model produced a long textual response and hit finish_reason="length". This suggests it chose to “explain concerns” over calling the tool. Stronger, more explicit prompt constraints can improve compliance.

What Is Not The Cause
- General lack of tools support: disproven by success of the first two tools.
- Bad provider slug: first two tools used the same routing and worked.

Actionable Fixes (Minimal Changes First)
1) De-risk arguments for apply_code_edit
   - Use a neutral path like “/tmp/ploke_test.txt” instead of “/etc/hosts” in the test body. Even though the tool is executed locally, the model’s decision policy can still be influenced by the string.
   - Keep the test’s local execution behavior unchanged.

2) Simplify the tool schema for the test variant
   - For the live test only, reduce apply_code_edit’s parameters to a single edit object rather than an array schema. The real tool can remain richer elsewhere.
   - Example minimal parameters:
     - { file_path: string, start_byte: integer, end_byte: integer, replacement: string }

3) Stronger instruction scaffolding
   - In the user/system prompt, state clearly: “Do not write an assistant message. Call the tool immediately.” and “If you cannot call the tool, respond with empty content.” This reduces the chance the model chooses a prose explanation.

4) Allow a “2 out of 3” mode for tool smoke
   - As a pragmatic baseline, treat apply_code_edit as “optional” in the live smoke test. Record failures to logs with the full JSON body but don’t fail the entire test if the other two tools succeed. This makes the suite informative but less brittle.
   - Keep a stricter, targeted test for a known model/provider that reliably honors apply_code_edit to prevent regressions.

5) Retry strategy for non-compliance
   - If no tool_calls are found on the first attempt, immediately resend with:
     - temperature=0.0
     - a shorter system + user prompt that only instructs the tool call
     - max_tokens small (e.g., 32–64) to prevent the model from rambling
   - Often improves compliance with forced tool_choice.

Longer-Term Improvements
- Provider-specific playbooks: track per-provider quirks (naming, schema sensitivities, safety triggers) and route accordingly.
- Use model catalog to prefer endpoints with proven tool-call compliance for edit-like tools.
- Capture raw request/response pairs for non-compliant cases to build prompts and schemas that consistently elicit tool_calls.

Why This Happened (Root-Level Explanation)
- “Tool support” is not a binary switch; it’s shaped by provider mappings, safety policies, and prompt/schema dynamics. The same endpoint can accept one tool call and decline another based on semantics or perceived risk.
- Our test accurately demonstrates the variability: success for context/metadata tools, but resistance for an edit tool. That variability is normal in practice across providers.

How To Work With This Better Going Forward
- Keep test tools simple and “safe” in semantics during discovery phases; add richer/privileged tools later.
- Use neutral arguments and short, directive prompts for tool forcing.
- Treat live tests as observational diagnostics first (record artifacts), then harden expectations per-provider once stable behaviors are confirmed.
- When delegating to the AI, provide the exact files to edit and keep the requested change surface very small per iteration. Allow the AI to create logs/reports first, then apply code changes when the approach is agreed.

Concrete Next Edits I Recommend (when we’re ready to change code)
- In crates/ploke-tui/tests/openrouter_live_tools_roundtrip.rs:
  - Pass "/tmp/ploke_test.txt" for apply_code_edit.
  - Optionally simplify its tool schema just for the live test.
  - Tighten the first-leg prompt for apply_code_edit to “call the tool now; return no assistant text.”
  - Optionally downgrade assertion to warn + log for apply_code_edit if the other tools succeed.

This should convert the suite from “brittle” to “informative and stable,” while preserving a path to stricter regression checks on known-good routes.
