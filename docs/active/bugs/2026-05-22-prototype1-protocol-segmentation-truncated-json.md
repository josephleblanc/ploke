# Prototype 1 Protocol Segmentation Truncated JSON

Status: fixed in source checkout; focused regression verified; live campaign
retest pending.

## Summary

The `p1-smoke-broad-harness-1g2x3-20260522-2` run blocked in
`baseline_protocol` because tool-call intent segmentation received a truncated
Google JSON response and treated the parse failure as terminal for the selected
run. No protocol artifact was written, so closure made no progress and the
batch selection failed.

## Evidence

Failed worktree:
`/home/brasides/.ploke-eval/worktrees/p1-smoke-broad-harness-1g2x3-20260522-2`

Observed doctor state:

```text
phase: baseline_protocol
prompt_preflight: passed
allowed_actions: doctor, continue, step
```

Campaign files showed:

```text
model_id: google/gemini-2.5-flash
route: DirectGoogle
protocol.max_tokens: 2000
protocol status: missing
tool-call-intent-segments: missing
```

The blocking error was:

```text
failed to parse json response: EOF while parsing a string at line 9 column 48
content was: {
  "segments": [
    {
      "start_index": 0,
      "end_index": 10,
      "status": "labeled",
      "label": "locate_target",
      "confidence": "high",
      "rationale": "The agent repeatedly reads '
```

## Root Cause

The first failure boundary is protocol intent segmentation, not selection. The
baseline protocol phase selected the run and tried to create the missing
`tool_call_intent_segmentation` artifact. The Direct Google response stopped in
the middle of `segments[0].rationale`, before a complete
`SegmentationJudgment` could be parsed or persisted.

Two implementation details made this block durable:

- `execute_protocol_intent_segments_quiet` retried only normalized segmentation
  shape errors, not LLM JSON EOF parse errors from the right fan-out branch.
- The admitted smoke profile inherited the old `protocol.max_tokens = 2000`
  default, which is too small for this segmentation path when the provider
  spends part of the completion budget before returning JSON.

There was also a policy-authority mismatch: the Prototype 1 baseline protocol
step read the campaign manifest protocol policy instead of the admitted
run-profile protocol policy, even though the run-profile is the active admitted
control surface.

## Fix

- Treat `ProtocolLlmError::ParseJson` with `EOF while parsing` as a retryable
  tool-call intent segmentation error.
- Raise the default Prototype 1 protocol adjudication budget from `2000` to
  `4000` and update the run-profile documentation.
- Make the Prototype 1 baseline protocol advance path use the admitted
  run-profile protocol policy.
- Add regression coverage:
  `intent_segmentation_truncated_json_parse_is_retryable`.

## Verification

Verified focused regression:

```text
cargo test -p ploke-eval intent_segmentation_truncated_json_parse_is_retryable -- --nocapture
```

Pending:

- updated failed campaign profile/commitment if needed for live retest
- live `prototype1-step` from the failed parent worktree
