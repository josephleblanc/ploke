**Context**
- Action: in ploke-tui, run `/model search <model>` (e.g., `gpt`, also seen with `kimi`).
- Result: chat shows “Failed to query OpenRouter models: error decoding response body”.
- Default/chat calls still work (see successful completions in api_responses tail), so core chat is up; the model search endpoint/handling is failing.

**Repro**
1) Start ploke-tui.
2) `/model search gpt` (or `/model search kimi`).
3) Observe chat error: “Failed to query OpenRouter models: error decoding response body”.

**Expected vs Actual**
- Expected: model search returns list of models without error.
- Actual: model search fails with “error decoding response body”; downstream chat continues using default model.

**Evidence (logs)**
- Command: `rg -n "Failed to query OpenRouter models" crates/ploke-tui/logs`
- Findings:
  - `crates/ploke-tui/logs/api_responses.log.2025-12-18:29733` (and 29737) system prompt lines with the error text.
  - `crates/ploke-tui/logs/ploke.log.2025-12-18:83789` shows PromptConstructed containing two system messages: “Failed to query OpenRouter models: error decoding response body”.
- Request excerpt (api_responses log): model `moonshotai/kimi-k2`, messages include two system entries with the error text.

**Hypotheses**
- The `/model search` OpenRouter response shape changed (or includes unexpected fields), and our decoder rejects it.
- We might be hitting a non-200/HTML or error payload and trying to decode as the expected schema.
- Potential double-injection of the error message into the prompt (two system lines) suggests retry/duplication in error handling.

**Next Actions**
- Inspect the OpenRouter model search response handling/structs in ploke-llm/ploke-tui; align decoder with current API.
- Add a log of the raw response on decode failure (redact keys) to confirm payload shape.
- Add a small integration test/fake fixture for model-search decoding to catch schema drift.
- Ensure we surface a single, clean error message to the user and avoid polluting the chat prompt with duplicate system lines.
