# Prototype 1 Overlapping Protocol Step Duplicates Artifacts

Status: open
Discovered: 2026-05-25

## Summary

Prototype 1 does not currently prevent two `prototype1-step` processes from
running the same baseline protocol phase for the same campaign/run at the same
time. When this happened during orchestration of
`p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746`, closure collapsed
the run to protocol `complete`, but the protocol artifact directory contained
duplicate per-call review evidence.

This is a protocol execution and accounting bug. The state machine should make a
concurrent protocol advance impossible, idempotent, or visibly blocked.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746
```

Run:

```text
run-1779709154252-structured-current-policy-72157381
```

After two overlapping `prototype1-step` sessions exited, doctor reported:

```text
phase = child_plan
blockers = []
allowed_actions = ["doctor", "continue", "step"]
```

Closure reported all required protocol procedures complete:

```text
protocol.status = complete
tool-call-intent-segments.complete_total = 1
tool-call-review.complete_total = 1
tool-call-segment-review.complete_total = 1
protocol_counts.total_calls = 100
protocol_counts.reviewed_calls = 100
protocol_counts.total_segments = 8
```

The artifact directory was not a clean single protocol pass:

```text
tool_call_intent_segmentation artifacts: 1
tool_call_review artifacts: 200
tool_call_segment_review artifacts: 8
```

The 200 per-call review files are consistent with two protocol workers each
writing review artifacts for the same 100-call run. Closure/status did not flag
the duplicate surface.

## Expected Behavior

For a campaign/run already in baseline protocol, the controller should do one of:

- acquire a per-run protocol lock and reject or wait on the second process;
- detect that a compatible protocol pass is already in progress and report that
  state without starting another pass;
- write deterministic/idempotent artifacts keyed by procedure and call/segment
  identity so a retry cannot multiply evidence files; or
- have doctor/closure flag duplicate protocol artifacts as a blocker before the
  campaign can advance to child planning.

## Impact

This can make a campaign look cleanly complete while the durable protocol
surface is not actually a single coherent pass. That weakens later run reviews,
LLM adjudication analysis, and loop-transition evidence because reviewers have
to distinguish normal per-call artifacts from duplicate execution output.

For the discovered isolated campaign, do not treat the post-review protocol
directory as clean evidence for a single successful protocol run.

## Disposition

Non-blocking for source development, but blocking for using the affected
campaign as clean loop evidence. The next clean run should start from a fresh
campaign after this is either fixed or explicitly accepted as an operator
orchestration risk.
