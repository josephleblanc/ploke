# Bug: Direct Google `malformed_function_call` finish reason misclassified as unknown tool name

**Date Discovered:** 2026-06-10  
**Crates Affected:** `ploke-llm`, `ploke-tui`  
**Severity:** High  
**Status:** Mitigated (deserialize + classify fix; provider-side malformation remains)

## Summary

On the Vertex OpenAI-compatible direct Google path (`google/gemini-2.5-flash`),
Gemini can return:

- `finish_reason: "malformed_function_call"`
- `message.refusal` containing Python-style code such as
  `print(default_api.apply_code_edit(...))` instead of structured `tool_calls`

`ploke-llm` did not deserialize that finish reason, so the chat loop treated the
deserialization error as `UNKNOWN_TOOL_NAME` for tool `malformed_function_call`
and exhausted the repair budget with `REPAIR_BUDGET_EXHAUSTED`.

## Evidence

Prototype 1 state5 run:

- Instance:
  `~/.ploke-eval/instances/prototype1/p1-g25f-direct-protocol-2target-g0g2-1x3-state5-20260610-004953`
- Child:
  `BurntSushi__ripgrep-2209/runs/run-1781078219114-structured-current-policy-64627255`
- Diagnostic excerpt:
  `unknown variant 'malformed_function_call' ... refusal: Malformed function call: print(default_api.apply_code_edit(...`

## Fix

- `FinishReason::MalformedFunctionCall` in `ploke-llm`
- `parse_chat_outcome` surfaces `LlmError::FinishError` for that finish reason
- Chat loop classifies it as `MALFORMED_FUNCTION_CALL` model behavior (not
  `UNKNOWN_TOOL_NAME` repair)
- Defense: finish-reason deserialization errors no longer map to unknown tool
  names in `semantics::normalize_llm_error`

Tests:

- `google_malformed_function_call_finish_reason_deserializes`
- `parse_outcome_malformed_function_call_returns_finish_error`
- `deserialization_unknown_finish_reason_does_not_map_to_unknown_tool_name`
- `classify_finish_error_malformed_function_call_is_model_behavior_not_tool_name_repair`

## Remaining provider risk

Google may still emit malformed function-call text under patch pressure,
especially with large arguments or long contexts. Mitigations from Google docs
worth evaluating for direct Google requests:

- `tool_choice: "required"` / `FunctionCallingConfig.mode = ANY` with
  `allowed_function_names` when forcing a specific tool
- `tool_choice: "validated"` (Google-specific OpenAI-compat mode)
- Lower temperature for tool-heavy turns
- Prompt guidance to emit strict JSON tool arguments

See also
[`2026-04-21-provider-tool-call-argument-malformation-without-repair.md`](./2026-04-21-provider-tool-call-argument-malformation-without-repair.md).
