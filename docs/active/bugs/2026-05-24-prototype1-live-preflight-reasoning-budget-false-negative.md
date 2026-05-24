# Prototype 1 Live Preflight Reasoning Budget False Negative

Status: fixed in source; live preflight recheck passed
Discovered: 2026-05-24

## Summary

`prototype1-doctor --live-protocol-preflight` can falsely block a protocol route
that accepts the request shape when the model requires hidden reasoning tokens.

The preflight previously capped `max_tokens` at 64:

```rust
let max_tokens = policy.max_tokens.min(64).max(1);
```

For `google/gemini-3.5-flash` through OpenRouter `google-ai-studio`, the current
source correctly omits the explicit reasoning-disable control, but the provider
spends nearly all 64 completion tokens on reasoning and returns only visible
content `Here` with `finish_reason = "length"`. Doctor reports this as a JSON
parse failure and blocks the parent, even though HTTP status was 200 and the
original request-shape blocker is no longer present.

## Concrete Evidence

Campaign:

```text
p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Command:

```text
target/debug/ploke-eval loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --live-protocol-preflight \
  --format json
```

Doctor output:

```text
protocol_preflight.outcome = failed
model_id = google/gemini-3.5-flash
provider = google-ai-studio
route_source = openrouter
reasoning = omit
max_tokens = 64
detail = provider_response: JSON parse failed: expected value at line 1 column 1
phase = blocked
```

Latest request log:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260524_033510_949196.log
```

The log shows:

- request body omitted `reasoning`;
- request used `response_format = {"type":"json_object"}`;
- response status was 200;
- response finish reason was `length` / `MAX_TOKENS`;
- visible assistant content was `Here`;
- usage reported 58 reasoning tokens out of 60 completion tokens.

## Broken Contract

The live protocol preflight is supposed to distinguish provider/auth/request
shape blockers from safe-to-advance protocol routes. It should not turn a
budget-starved reasoning response into an indistinguishable request-shape
blocker.

## Expected Behavior

The preflight should either:

- allocate enough completion budget for reasoning-mandatory models to return the
  sentinel JSON, or
- classify `finish_reason = length` / exhausted visible JSON as a response-budget
  failure with a clear operator action instead of a generic JSON parse failure.

The check should continue to report the exact model, provider, route source,
reasoning policy, and bounded outcome without logging credentials.

## Likely Fix Direction

- Replace the hard cap of 64 tokens with a safer canary budget derived from the
  admitted protocol budget, with a minimum high enough for reasoning models.
- Preserve the small-live-call intent by using a bounded cap rather than the full
  protocol budget.
- Add a unit test for the preflight token-budget selection.
- If feasible, improve classification for parse failures caused by provider
  length finishes.

## Fix Status

Implemented in `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`:

- `run_protocol_live_preflight` now chooses `max_tokens` through a named
  `protocol_live_preflight_max_tokens` helper.
- Omitted or explicit-effort reasoning policies use the admitted protocol budget
  up to a bounded 512-token canary, while respecting admitted budgets below the
  256-token reasoning floor.
- Explicit disabled reasoning keeps the previous small 64-token canary.

Focused reproduction before the helper fix:

```text
cargo test -p ploke-eval protocol_live_preflight_budget -- --nocapture
```

Failure signal:

```text
protocol_live_preflight_budget_bounds_reasoning_canary
left: 64
right: 128
```

Focused verification after the fix:

```text
cargo test -p ploke-eval protocol_live_preflight_budget -- --nocapture
```

Outcome:

```text
2 passed; 0 failed
```

Remaining live verification for the orchestrator:

```text
target/debug/ploke-eval loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --live-protocol-preflight \
  --format json
```

For the current default admitted protocol budget and `reasoning = omit`, the
preflight should now report `max_tokens = 512` instead of `64`.

Live verification after the fix:

```text
cargo run -p ploke-eval -- loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --live-protocol-preflight \
  --format json
```

Outcome:

```text
protocol_preflight.outcome = passed
reasoning = omit
max_tokens = 512
phase = baseline_protocol
blockers = []
allowed_actions = [doctor, continue, step]
```

## Related Code

- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs::run_protocol_live_preflight`
- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs::classify_protocol_preflight_error`

## Related Reports

- [`2026-05-24-prototype1-protocol-reasoning-config-blocker.md`](./2026-05-24-prototype1-protocol-reasoning-config-blocker.md)
