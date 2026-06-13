# Bug: Direct Google `malformed_function_call` finish reason misclassified as unknown tool name

**Date Discovered:** 2026-06-10  
**Crates Affected:** `ploke-llm`, `ploke-tui`, `ploke-records`, `ploke-eval`, `ploke-egui`  
**Severity:** High  
**Status:** Fixed + live-verified (output-token-budget floor = 16384); clean baseline in state8 (`gemini-2.5-pro`, direct-google). Classification + deserialize fixes retained. See "State8 live run".

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

Prototype 1 state5 run (`gemini-2.5-flash`, `apply_code_edit`):

- Instance:
  `~/.ploke-eval/instances/prototype1/p1-g25f-direct-protocol-2target-g0g2-1x3-state5-20260610-004953`
- Child:
  `BurntSushi__ripgrep-2209/runs/run-1781078219114-structured-current-policy-64627255`
- Diagnostic excerpt:
  `unknown variant 'malformed_function_call' ... refusal: Malformed function call: print(default_api.apply_code_edit(...`

Prototype 1 state6 recurrence (`gemini-2.5-pro`, `non_semantic_patch`):

- Instance:
  `~/.ploke-eval/instances/prototype1/p1-g25p-direct-protocol-2target-g0g2-1x3-state6-20260610-014509`
- Child:
  `BurntSushi__ripgrep-2209/runs/run-1781081390723-structured-current-policy-9b2c8ea1`
- `agent-turn-summary.json` terminal record:
  `code=MALFORMED_FUNCTION_CALL kind=model_behavior` after 4 attempts, with
  refusal `Malformed function call: print(default_api.non_semantic_patch(patches=[default_api.NonSemanticPatchPatches(file='crates/printer/src/util.rs', diff='''<multi-line unified diff>''', reasoning=...)]))`
- Confirms the malformation is not specific to `apply_code_edit` or to
  `2.5-flash`: it recurs on `2.5-pro` with `non_semantic_patch` whenever the
  tool argument carries a multi-line diff/code string under `tool_choice=auto`.
- Live reproduction (2026-06-10): `live_google_auto_tool_choice_multiline_patch_rejects_malformed_or_quota`
  reproduced `finish_reason=MalformedFunctionCall` on `gemini-2.5-flash` with
  the `{patches:[{file,diff}]}` schema and `tool_choice=auto` (completion_tokens=0).

## Fix

- `FinishReason::MalformedFunctionCall` in `ploke-llm`
- `parse_chat_outcome` surfaces `LlmError::FinishError` for that finish reason
- Chat loop classifies it as `MALFORMED_FUNCTION_CALL` model behavior (not
  `UNKNOWN_TOOL_NAME` repair)
- Defense: finish-reason deserialization errors no longer map to unknown tool
  names in `semantics::normalize_llm_error`

Tests (classification fix — synthetic):

- `google_malformed_function_call_finish_reason_deserializes`
- `parse_outcome_malformed_function_call_returns_finish_error`
- `deserialization_unknown_finish_reason_does_not_map_to_unknown_tool_name`
- `classify_finish_error_malformed_function_call_is_model_behavior_not_tool_name_repair`

## Repro coverage (added 2026-06-10)

Captured-payload + eval-shape tests built from the state6 incident. The
canonical refusal fixture (real captured multi-line `non_semantic_patch` text)
lives at:

- `crates/ploke-llm/test_data/google/malformed_non_semantic_patch_refusal.txt`
- `crates/ploke-tui/tests/fixtures/google/malformed_non_semantic_patch_refusal.txt`

Tests:

- `parse_outcome_malformed_non_semantic_patch_multiline_diff_replays_to_finish_error`
  (`crates/ploke-llm/src/manager/session.rs`) — replays the captured response
  (finish_reason `malformed_function_call` + multi-line diff refusal) through
  `parse_chat_outcome`; asserts `FinishReason::MalformedFunctionCall`, a
  `FinishError`, and that the multi-line diff body survives parsing. **Passes**
  with current code (locks the classification fix against this exact payload).
- `classify_finish_error_malformed_multiline_patch_is_model_behavior_code`
  (`crates/ploke-tui/src/llm/manager/loop_error.rs`) — feeds the real multi-line
  refusal through `classify_llm_error`; asserts code `MALFORMED_FUNCTION_CALL`,
  kind `ModelBehavior`, not `UNKNOWN_TOOL_NAME`, not a repair loop. **Passes**.
- `live_google_auto_tool_choice_multiline_patch_rejects_malformed_or_quota`
  (`crates/ploke-llm/src/router_only/google/mod.rs`, gated behind
  `live_api_tests` + `#[ignore]`) — mirrors the production request shape:
  `gemini-2.5-flash`, multiple tools (`read_file` + `non_semantic_patch` whose
  `diff` arg is a multi-line string), `tool_choice=auto`, and a prompt that
  induces a multi-line unified diff. Asserts the turn does **not** terminate
  with `MALFORMED_FUNCTION_CALL` (genuine 429/`RESOURCE_EXHAUSTED` is an
  accepted skip; malformed is **not** a skip). **Expected to FAIL/reproduce**
  until a provider-side fix lands; confirmed reproducing on 2026-06-10.

## Root cause + fix (verified 2026-06-10)

The malformation is **output-token truncation of the structured tool call**, not
the argument schema and not the `tool_choice` mode. When the function-call
emission exceeds the request's output-token budget, Vertex returns
`finish_reason = malformed_function_call`. The production trigger is that the
broad direct-Google patch turn sends **no `max_tokens` at all**
(`LLMParameters::default().max_tokens == None`, and the field is
`skip_serializing_if`), so Vertex applies a small default that truncates the
call. Thinking models (`gemini-2.5-pro`, state6) make it worse: reasoning tokens
consume the budget before the call is even emitted.

### Live spike that isolated the cause (`gemini-2.5-flash`, no quota/auth interference)

| Variant | tools | max_tokens | tool_choice | diff schema | result |
|---|---|---|---|---|---|
| V1 control | read+patch | 1024 | auto | string | **FAIL malformed** |
| V2 | read+patch | 8192 | auto | string | OK tool call |
| V3 | patch only | 8192 | auto | string | OK tool call |
| V4 | patch only | 8192 | required | string | OK tool call |
| V5 | patch only | 8192 | auto | array-of-lines | ERR (`unexpected_tool_call`; model hallucinated fn `NonSemanticPatchPatches`) |
| V6 | patch only | 8192 | required | array-of-lines | OK tool call |

Only the token budget flips V1→V2 (same string-diff schema, same `auto`). The
"array-of-lines" arg shaping (the original fix-A idea) is *worse* (V5). An
earlier paired live test also confirmed `tool_choice=required` does **not**
eliminate the malformation at a small budget.

### Fix (implemented)

- `ploke-tui::llm::model_overrides::ParamOverrides` gained `max_tokens_floor`
  plus `effective_max_tokens()` (raises only when unset/lower; never lowers a
  larger explicit budget).
- `google_gemini::resolve` now returns `tool_choice: None` and
  `max_tokens_floor: Some(MAX_TOKENS_FLOOR)` for direct-Google
  `gemini-2.5*`/`3.5*`. The empirically-useless, loop-trapping `Required`
  recommendation was dropped. `MAX_TOKENS_FLOOR` was initially 8192 (proven to
  remove the malformation) and **raised to 16384** after the state7 live run (see
  below) showed the thinking model truncating at 8192.
- The chokepoint (`prepare_and_run_llm_call`) applies the floor to
  `llm_params.max_tokens`. Raising the budget is termination-safe (unlike
  forcing `Required`). The `tool_choice` plumbing is retained behind
  `FORCE_OVERRIDE_TOOL_CHOICE` for a possible future per-turn site.
- New `FinishReason::UnexpectedToolCall` (+ `FinishReasonRecord::UnexpectedToolCall`)
  so the V5 `unexpected_tool_call` string no longer fails response
  deserialization; `parse_chat_outcome` surfaces it as a `FinishError`
  (model-behavior), with arms added in the ploke-tui retry/summary matches and
  the ploke-eval/ploke-egui record mappings.

### Tests for the fix

- Offline: `effective_max_tokens_raises_only_when_unset_or_below_floor` and
  `resolve_sets_max_tokens_floor_without_tool_choice_for_direct_google_gemini_families`
  (`ploke-tui`); `google_unexpected_tool_call_finish_reason_deserializes`
  (`response/shape_tests.rs`); `parse_outcome_unexpected_tool_call_returns_finish_error`
  (`ploke-llm/src/manager/session.rs`).
- Live (gated `live_api_tests` + `#[ignore]`, `crates/ploke-llm/src/router_only/google/mod.rs`):
  - `live_google_low_token_budget_multiline_patch_reproduces_malformed_or_quota`
    (negative: asserts malformed reproduces at 1024).
  - `live_google_floor_token_budget_multiline_patch_avoids_malformed_or_quota`
    (positive: asserts the 8192 floor avoids malformed). Mirrors `MAX_TOKENS_FLOOR`.

Re-run live:

```
gcloud auth application-default login   # interactive, one-time, if ADC lapsed
GOOGLE_PROJECT_ID=... GOOGLE_REGION=... \
cargo test -p ploke-llm --features live_api_tests multiline_patch -- \
  --ignored --nocapture --test-threads=1
```

### State7 live run (2026-06-10): malformation eliminated; floor raised to 16384

A fresh prototype1 run replicating the exact state6 config (`gemini-2.5-pro`
broad + protocol, direct-google) with the fix in place produced a clean
before/after on the failing config:

- `llm-full-responses.jsonl`: `finish_reason=tool_calls` ×11,
  `finish_reason=length` ×2, **zero `malformed_function_call`**. The malformation
  is gone; the model makes real structured tool calls.
- The run then `blocked` on `code=OUTPUT_TRUNCATED` (baseline eval, 2.5-pro):
  reasoning tokens consumed the 8192 budget before the patch completed, so the
  turn hit `finish_reason=length`.

Length-retry investigation (why `length` was terminal, not silently retried):

- The length-continue/retry path DID fire — the 2 `length` responses are the
  original + 1 continuation retry (`length_continue_prompt`). After the retry was
  exhausted, the loop returned a terminal `FinishError(OUTPUT_TRUNCATED)` and
  baseline-eval correctly treats an incomplete turn as a failure → `blocked`.
- Effective length-retry budget on the baseline `agent-single-turn` is **1**:
  `benchmark_chat_policy()` (`ploke-eval/src/runner/mod.rs`) builds
  `ChatPolicy::default()` and never sets `length_retry_limit` (default 1), whereas
  the headless `configure_sparse_strict_rag` sets 5. **Known inconsistency**
  (baseline=1 vs headless broad-child=5); not changed here.
- More retries would not reliably help: a thinking model re-spends reasoning
  tokens on each continuation within the same budget, so each continued turn
  truncates again. Continuation adds turns, not per-turn budget. The fix is a
  larger `max_tokens` floor (16384), not more retries.

Re-run after the floor bump uses a fresh campaign (state7 is `blocked` on
recorded failed baseline evidence and must not be rerun in place).

### State8 live run (2026-06-10): clean baseline at 16384 floor

Fresh campaign `p1-g25p-direct-protocol-2target-g0g2-1x3-state8-20260610-155046`
(identical state6/7 config: `gemini-2.5-pro` broad + protocol, direct-google) with
`MAX_TOKENS_FLOOR=16384`:

- `prototype1-step` advanced `baseline_eval → baseline_protocol`, **blockers
  empty** (state7 was `blocked` at this exact phase on OUTPUT_TRUNCATED).
- Baseline `llm-full-responses.jsonl` finish reasons: `tool_calls` ×5, `stop` ×2 —
  **zero `malformed_function_call`, zero `length`/OUTPUT_TRUNCATED**.

The 16384 floor resolves both the malformation and the thinking-model truncation
at baseline end-to-end.

---

## Fix B: model-overrides framework + `ToolChoice::Required` (2026-06-10)

> SUPERSEDED by the token-budget fix above. `Required` was empirically shown not
> to fix the malformation and would trap the session loop; the override
> framework was repurposed to carry the `max_tokens_floor` instead. Kept below
> for history.

Implemented the **fix B** primitive and a non-ad-hoc registry to carry it:

- `ploke-llm`: extended `ToolChoice` (`request/endpoint.rs`) with a `Required`
  variant that serializes to the OpenAI-compatible string `"required"` (maps to
  Vertex `functionCallingConfig.mode = ANY`). Round-trip covered by
  `tool_choice_serde_variants`.
- `ploke-tui`: new `llm::model_overrides` module — a typed per-model
  request-quirk registry. `resolve(router, model_id) -> Option<ModelOverride>`;
  `ModelOverride { tool_choice: Option<ToolChoice>, params: ParamOverrides }`
  (`ParamOverrides` is an empty, documented extension point). First entry
  (`google_gemini.rs`) matches the direct-Google (Vertex) route
  (`RouterVariants::Google`) AND model family prefix `gemini-2.5*`/`gemini-3.5*`
  and recommends `tool_choice = Required`. OpenRouter-routed Google models and
  non-Gemini models do not match.
- Wired at the single chokepoint `prepare_and_run_llm_call`
  (`ploke-tui/src/llm/manager/mod.rs`), after `model_id`/`active_router`/base
  `tool_choice` are resolved.

### Termination-safety decision (important)

`prepare_and_run_llm_call` builds **one** request whose `tool_choice` is reused
for **every** turn of the in-session tool-call loop (`run_chat_session`). That
loop only terminates when the model returns a **no-tool `Content`** response;
there is no terminal "finish" tool. Forcing `Required` for the whole session
would make every turn emit a tool call, so the model could never produce a
terminal prose answer and the loop would run until the tool-call-chain limit (a
trap). Because this chokepoint cannot distinguish a "finalizing" turn from an
intermediate one, the override's recommended `Required` is **wired but
DEFAULTED OFF** here (guarded by a clearly-commented `FORCE_OVERRIDE_TOOL_CHOICE`
constant). The effective session-wide mitigation must therefore be patch-arg
shaping (fix A) or a per-turn application site that can isolate a single
forced-edit turn.

### Tests

- `crates/ploke-tui/src/llm/model_overrides/mod.rs`:
  `resolve_some_for_direct_google_gemini_families` (asserts
  `Some(ToolChoice::Required)` for `gemini-2.5-flash`/`gemini-3.5-flash`),
  `resolve_none_for_openrouter_google_route`,
  `resolve_none_for_non_gemini_direct_google_model`. **Pass.**
- `crates/ploke-llm/src/router_only/google/mod.rs`:
  `live_google_required_tool_choice_multiline_patch_avoids_malformed_or_quota`
  (gated behind `live_api_tests` + `#[ignore]`) — same eval-shape request as the
  Auto repro but with `tool_choice=required`; asserts the turn does NOT finish
  with `MalformedFunctionCall` and that a structured tool call is emitted
  (quota/429 accepted as skip).

### EMPIRICAL live result (2026-06-10): UNVERIFIED — ADC could not mint a token

The live Required test (and the Auto repro) could **not** be exercised in this
environment. `GOOGLE_PROJECT_ID`/`GOOGLE_REGION` were set and an ADC file was
present, but ADC token resolution failed:

```
failed to resolve bearer token: ... failed to resolve Google application
default credentials, original: Request to fetch the token failed ...
```

`gcloud auth application-default print-access-token` confirmed the cause:
`Reauthentication failed. cannot prompt during non-interactive execution`
(requires an interactive `gcloud auth application-default login`). Both tests
failed at the HTTP **send** phase before reaching Vertex; network egress to
`aiplatform.googleapis.com` itself was fine.

**Therefore the key question — does `tool_choice=required` eliminate
`MALFORMED_FUNCTION_CALL`, and can the model still terminate — remains
UNVERIFIED live.** Re-run with valid ADC:

```
gcloud auth application-default login   # interactive, one-time
PLOKE_RUN_LIVE_TESTS=1 cargo test -p ploke-llm --features live_api_tests --lib -- \
  --ignored --nocapture \
  live_google_required_tool_choice_multiline_patch_avoids_malformed_or_quota \
  live_google_auto_tool_choice_multiline_patch_rejects_malformed_or_quota
```

### Recommendation (pending live confirmation)

`Required` is at best a **partial** mitigation and is **not** a sufficient
standalone fix as a session-wide default: even if it eliminates the malformation
(unverified), it cannot be forced across the whole agent loop without trapping
termination. The recommended path is **fix A (provider-aware patch-arg
shaping)** as the durable session-wide mitigation, with `Required` reserved for
a future per-turn forced-edit application site once one exists.

## Remaining open item: provider-side multi-line-patch malformation

The deserialize + classify fix only makes the failure legible; the provider
still emits Python `print(default_api.<patch_tool>(...))` instead of structured
`tool_calls` whenever a tool argument carries a multi-line diff/code string
under `tool_choice=auto`. Ranked candidate fixes (not yet implemented — repro
tests land first, then decide):

- **A. Provider-aware patch-arg shaping** for direct Google requests (e.g.
  avoid handing Gemini a single large multi-line `diff` string argument under
  Auto; reshape/segment patch args or route patch turns differently). Preferred:
  addresses the root malformation rather than the trigger.
- **B. `tool_choice: required`/`validated` variant** with
  `allowed_function_names` (Google `FunctionCallingConfig.mode = ANY`) when an
  edit is forced, so Gemini is constrained to structured function calls.

Supporting mitigations worth evaluating alongside A/B:

- Lower temperature for tool-heavy turns
- Prompt guidance to emit strict JSON tool arguments

See also
[`2026-04-21-provider-tool-call-argument-malformation-without-repair.md`](./2026-04-21-provider-tool-call-argument-malformation-without-repair.md).
