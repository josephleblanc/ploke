# Prototype 1 Protocol Reasoning Config Blocker

Status: open; blocks the Gemini 3.5 Flash loop run at `baseline_protocol`.
Discovered: 2026-05-24

## Summary

The `p1-gemini35-flash-multigen-2g3x3-20260523-223658` loop run cannot advance
past `baseline_protocol` because protocol JSON adjudication unconditionally
sends:

```json
"reasoning": {
  "effort": "none"
}
```

The selected protocol route was OpenRouter with provider
`google-ai-studio` for `google/gemini-3.5-flash`. That endpoint rejected the
request because reasoning is mandatory and cannot be disabled. No protocol
artifacts were written, so the required protocol procedures stayed missing and
`prototype1-step` stopped with a no-progress diagnostic.

## Concrete Evidence

Campaign:

```text
p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Bounded advance command:

```text
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

Observed failure:

```text
baseline_protocol blocked: campaign p1-gemini35-flash-multigen-2g3x3-20260523-223658 made no protocol progress
model google/gemini-3.5-flash
route OpenRouter/google-ai-studio
tool-call-intent-segments missing
tool-call-review missing
tool-call-segment-review missing
API error (status 400): Reasoning is mandatory for this endpoint and cannot be disabled.
```

Post-failure state:

```text
eval.status = complete
protocol.status = missing
protocol artifacts written = 0
parent worktree = clean
prototype1-doctor phase = baseline_protocol
```

## Root Cause

The run profile and campaign select the protocol model/provider, but the actual
JSON adjudication request is built in `ploke-protocol` with a hard-coded
reasoning override:

```rust
.with_reasoning(ReasoningConfig::default().with_effort(ReasoningEffort::None))
```

Current code path:

- `crates/ploke-eval/src/cli.rs::protocol_llm_config` resolves the protocol
  model, route, provider, timeout, attempts, and `max_tokens`.
- `crates/ploke-protocol/src/llm.rs::base_json_request` builds the final
  request for both OpenRouter and direct Google routes.
- `base_json_request` always sets `ReasoningEffort::None`.
- OpenRouter serializes that to the request body as
  `{"reasoning":{"effort":"none"}}`.

This makes the protocol layer incompatible with provider endpoints that require
reasoning or reject explicit reasoning-disable controls.

## Why It Matters

This is a loop-progress blocker, not just a provider annoyance. The eval run
already completed, but the self-evolving loop cannot reach protocol closure,
selection, or successor work because the required protocol artifacts are never
created.

It also exposes a configuration-authority gap: the run profile can configure
protocol token budget, but it cannot configure protocol reasoning behavior even
though reasoning controls are provider- and model-specific.

## Expected Behavior

Protocol reasoning behavior should be configurable from the admitted run
profile or campaign policy for the model used by protocol adjudication.

The request builder should support at least these states:

- omit reasoning controls entirely;
- set a provider-supported reasoning effort such as `low`, `medium`, or
  `high`;
- explicitly disable reasoning only for models/providers that accept it.

The default should not silently disable reasoning for all protocol models.

## Fix Direction

Add a protocol reasoning policy to the admitted run-profile surface and carry it
through the existing protocol config path.

Likely shape:

```toml
[protocol.reasoning]
mode = "omit"      # omit | effort | disabled
effort = "low"    # only used when mode = "effort"
```

Implementation path to verify before editing:

1. Extend the run-profile protocol policy and campaign commitment schema.
2. Thread the selected reasoning policy through `protocol_llm_config` into
   `JsonLlmConfig`.
3. Change `base_json_request` so it applies reasoning only according to that
   policy.
4. Add unit tests for omitted reasoning, explicit disabled reasoning, and an
   explicit effort.
5. Add a focused regression for the current OpenRouter JSON request shape.

## Doctor Check

Add a `prototype1-doctor` preflight that validates the configured protocol
model/provider/reasoning tuple before the loop reaches `baseline_protocol`.

The live check should be small and diagnostic:

- use the exact protocol model, provider, route source, max token policy, and
  reasoning policy that the campaign will use;
- send a minimal JSON-only prompt with a tiny response budget;
- classify provider auth/quota errors separately from request-shape errors;
- report whether the request was accepted and whether the response was parseable
  JSON;
- avoid logging credentials or provider metadata that is not needed for the
  operator.

This check should be optional or explicitly live-gated, but when run it should
catch request-shape errors like `reasoning.effort = "none"` before a full loop
advance attempts paid protocol work.

## Related Code

- `crates/ploke-protocol/src/llm.rs::base_json_request`
- `crates/ploke-eval/src/cli.rs::protocol_llm_config`
- `crates/ploke-eval/src/cli.rs::resolve_protocol_route`
- `crates/ploke-eval/docs/prototype1-run-profile.md`

## Related Reports

- [`2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md`](./2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md)
- [`2026-05-22-prototype1-protocol-segmentation-truncated-json.md`](./2026-05-22-prototype1-protocol-segmentation-truncated-json.md)
