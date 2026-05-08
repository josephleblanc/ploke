# Segment Review Packet Checkpoint

- date: 2026-04-15
- task title: segment review packet checkpoint
- task description: introduce a shared local-analysis packet boundary for protocol review procedures and add the first downstream segment-level review over `SegmentedToolCallSequence`
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_intent-segmentation-semantics-checkpoint.md`, `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`

## What Changed

This slice implements the next capability that had become more important after
the future-backward planning exercise:

1. one shared local-analysis packet shape
2. one shared assessment vocabulary
3. two packet constructors
4. one existing focal-call review adapted onto the shared packet
5. one new segment-level downstream review built on the same substrate

The important shift is that the review logic is no longer hard-coded to the old
bounded neighborhood subject.

## New Shared Boundary

`crates/ploke-protocol/src/tool_calls/review.rs` now defines:

- `LocalAnalysisTargetKind`
- `LocalAnalysisPacket`
- `LocalAnalysisSignals`
- `LocalAnalysisContext`
- `LocalAnalysisAssessment`

This is the new reusable protocol-facing boundary for local analysis.

Current packet constructors:

- `ContextualizeNeighborhood`
  - input: `ToolCallNeighborhood`
  - output: `LocalAnalysisContext`
- `ContextualizeSegment`
  - input: `SegmentReviewSubject`
  - output: `LocalAnalysisContext`

This is the first real implementation of the “same downstream review over
alternative admissible evidence constructions” idea.

## Existing Review Adapted

The old `tool_call_review` procedure now runs over the shared packet layer
instead of directly over neighborhood-specific prompt state.

This preserves:

- usefulness assessment
- redundancy assessment
- recoverability assessment
- mechanized merged overall verdict

But now its output is packet-based rather than neighborhood-specific.

## New Downstream Procedure

Added:

- `ToolCallSegmentReview`

This is the first downstream procedure that consumes segmentation as a real
intermediate state instead of stopping at the segmentation artifact itself.

The current flow in `ploke-eval` is:

1. build `ToolCallSequence`
2. run `tool_call_intent_segmentation`
3. select one `IntentSegment`
4. build `SegmentReviewSubject`
5. run `tool_call_segment_review`

CLI surface:

- `ploke-eval protocol tool-call-segment-review <SEGMENT>`

## Why This Matters

This is the first place where the crate starts proving the conceptual value of
the protocol architecture rather than only restating it.

We can now compare:

- focal-call neighborhood review
- segment-level local review

using:

- the same assessment vocabulary
- the same executor pattern
- the same artifact persistence surface

That is a much stronger substrate for later A/B or method-comparison work.

## Runtime Check

Completed:

- `cargo check -p ploke-protocol -p ploke-eval`
- `cargo build -p ploke-eval`
- `./target/debug/ploke-eval protocol --help`
- `./target/debug/ploke-eval protocol tool-call-segment-review 0`

Observed on the latest run:

- segment review completed end to end
- persisted a `tool_call_segment_review` artifact beside the run
- produced a segment-level judgment that was meaningfully different from the
  focal-call review surface

## Current Comparative Value

The comparison is already informative.

On the same latest run:

- focal-call review on call `0` was mixed
- segment review on segment `0` was focused progress

That is exactly the kind of distinction the architecture should eventually make
routine:

- a single call may look overlapping or recoverable
- the larger segment may still be productive as a unit

## Files Changed

- [crates/ploke-protocol/src/tool_calls/review.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/tool_calls/review.rs)
- [crates/ploke-protocol/src/lib.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/lib.rs)
- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
- [crates/ploke-eval/src/protocol_artifacts.rs](/home/brasides/code/ploke/crates/ploke-eval/src/protocol_artifacts.rs)

## What Is Better Now

- segmentation is no longer only a terminal descriptive artifact
- the review vocabulary can now operate over different context constructors
- the protocol stack can already express one important comparison:
  atom-like call review versus larger segment review
- persisted artifacts now cover that comparison surface too

## Best Next Step

Use the shared packet boundary to make the comparison more explicit rather than
adding another isolated review command.

Best next implementation order:

1. add a comparison or disagreement procedure between focal-call review and
   segment review when they cover related local work
2. improve the packet constructors so signals better reflect segment-level
   recovery, ambiguity, and residual coverage
3. add bounded mechanized aggregation from packet-level assessments toward
   turn-level or run-level supporting metrics
