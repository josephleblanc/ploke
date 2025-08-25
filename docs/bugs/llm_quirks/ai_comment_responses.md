# LLM Implementation Notes: Mistakes Observed and First-Pass Guardrails

Date: 2025-08-22

This document responds to the in-code AI comments and records concrete guardrails to avoid similar issues in the future. It also explains the changes made where appropriate.

## Issues and responses

1) Unnecessary cloning of `Option<SymlinkPolicy>`
- Symptom: Cloned `symlink_policy` before moving it into a task.
- Why it’s wrong: `SymlinkPolicy` is `Copy`; thus `Option<SymlinkPolicy>` is also `Copy`. Cloning is needless.
- Root cause: Habitual pattern of calling `clone()` in closures to satisfy the borrow checker, without checking trait bounds.
- Fix: Pass `symlink_policy` by value (copy) into the `move` closure. Updated code accordingly.
- Guardrail:
  - Prefer copy semantics when types are `Copy`; only `clone()` if deep copy is semantically required.
  - Add a review checklist item: “Is this clone necessary? Is the type Copy?”

2) map_or vs map_or_else
- Symptom: Used `Option::map_or` where `map_or_else` could be used.
- Why it’s questionable: When the default value is expensive to compute, `map_or_else` delays computing the default. Here the default is a constant `true`, so both are fine; the comment requested `map_or_else`.
- Fix: Switched to `map_or_else` and removed the AI comment.
- Guardrail:
  - Prefer `map_or_else` if the default branch performs work; otherwise either is acceptable. Follow local style consistently.

3) Function with too many parameters (`run_tool_roundtrip`)
- Symptom: Many positional parameters made call sites noisy and error-prone.
- Action: Recorded as a style improvement in this doc per the instruction.
- Guardrail:
  - Use a parameter object (struct) or builder when 4+ related parameters are passed together.
  - Group cohesive data (client, base_url, api_key) into a config struct.

4) Duplicate event emission in error path
- Symptom: Both `tool_call_failed` closure and `AppEvent::LlmTool(ToolEvent::Failed)` were emitted for the same condition.
- Risk: Double logging and confusing downstream consumers.
- Action: Logged here; no code change requested in the comment.
- Guardrail:
  - Centralize an error-reporting helper that emits exactly once (returns the formatted error).
  - Add test coverage for “single failure event per failure.”

5) Redundant reference usage
- Symptom: `starts_with(&canonicalize_best_effort(root))` – the `&` was unnecessary.
- Fix: Removed the redundant `&`.
- Guardrail:
  - Lean on Clippy (e.g., `needless_borrow`) and treat it as a warning budget to address.

6) `get(0)` vs `first()`
- Symptom: Used `get(0)` to obtain the first element of a slice.
- Fix: Switched to `first()` at the call site flagged by the comment.
- Guardrail:
  - Prefer `first()` for clarity and intent; it also avoids magic numbers.

7) Redundant closure in `map_err`
- Symptom: `.map_err(|e| Error::from(e))` instead of `.map_err(Error::from)`.
- Fix: Simplified to use the function pointer form.
- Guardrail:
  - Prefer function pointers when adapting error types without extra logic.

## Why these mistakes happened

- Over-reliance on muscle memory: defaulting to `.clone()` or `.get(0)` patterns without re-checking trait bounds and idioms.
- Rushing through implementation details: small nits sneak in when focusing on higher-level flow.
- Lack of immediate lint feedback in-context.

## Prevention plan (first-pass quality)

- Clippy-first: run `cargo clippy --all --all-features` locally and wire into CI, at least as warnings for:
  - needless_borrow
  - needless_closure
  - map_err_ignore (prefer function pointer)
  - pedantic subset we agree on (documented exceptions)
- Coding checklist for PRs:
  - Are we cloning a `Copy` type?
  - Any functions with > 4 parameters that are cohesive enough for a param struct?
  - Are we using `.first()`/`.last()` instead of `get(0)`/`get(len-1)`?
  - Are error events emitted exactly once?
- Prefer small, typed parameter objects for complex calls (e.g., `OpenRouterCall { client, base_url, api_key, model_id, provider_hint, tool }`).
- Write focused unit tests for event emission paths to catch duplicates early.

These changes have been applied where explicitly requested; other items are captured here for tracking and future refactors.
