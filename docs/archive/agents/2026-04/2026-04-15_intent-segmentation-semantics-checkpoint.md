# Intent Segmentation Semantics Checkpoint

- date: 2026-04-15
- task title: intent segmentation semantics checkpoint
- task description: refine the `tool_call_intent_segmentation` contract so labeled segments, ambiguous segments, and uncovered regions are represented distinctly and inspectably
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_protocol-artifact-persistence-handoff.md`, `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`

## What Changed

The segmentation procedure is no longer forced into the old shape:

- labeled segments only
- uncovered call indices as a loose residual

It now distinguishes three cases explicitly:

1. labeled segments
2. ambiguous but still clustered segments
3. uncovered residual spans

This makes the procedure more faithful to the conceptual requirement that
ambiguity must not be silently collapsed either into a fake label or into an
unstructured omission.

## Contract Changes

### Segment proposals and normalized segments

`IntentSegmentProposal` and `IntentSegment` now carry:

- `status`
- optional `label`
- `confidence`
- `rationale`

`status` is:

- `labeled`
- `ambiguous`

Contract rule:

- `labeled` requires a concrete label
- `ambiguous` must not carry a label

Normalization now validates those combinations explicitly.

### Uncovered regions

Residual calls are now represented both as low-level indices and as explicit
contiguous spans:

- `uncovered_call_indices`
- `uncovered_spans`

This makes uncovered regions usable as a downstream procedure input rather than
only as a flat debugging detail.

### Coverage summary

`SegmentedToolCallSequence` now also includes `coverage` with:

- total calls
- labeled segment count
- ambiguous segment count
- labeled call count
- ambiguous call count
- uncovered call count

This provides a compact mechanized summary that downstream procedures can use
without re-deriving the partition shape manually.

## Prompt Discipline Change

The adjudication prompt now instructs the model to:

- emit `status`
- emit `label` only for `labeled` segments
- use `other` only for a coherent but out-of-taxonomy segment
- use `ambiguous` when a contiguous episode exists but the label is weakly
  supported
- leave calls uncovered only when even an ambiguous segment would overstate
  coherence

That is the main label-discipline fix in this slice.

## CLI Change

`ploke-eval protocol tool-call-intent-segments` now prints:

- labeled versus ambiguous segment counts
- labeled versus ambiguous call counts
- uncovered spans as contiguous regions
- per-segment descriptors that distinguish `labeled:<label>` from `ambiguous`

This keeps the inspection surface aligned with the new contract instead of
showing only a flat list of labels.

## Files Changed

- [crates/ploke-protocol/src/tool_calls/segment.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/tool_calls/segment.rs)
- [crates/ploke-protocol/src/lib.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/lib.rs)
- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)

## Verification

Completed:

- `cargo check -p ploke-protocol -p ploke-eval`

Observed warnings were pre-existing unrelated warnings in `syn_parser` plus one
pre-existing dead-code warning in `ploke-eval`.

## What Is Better Now

- ambiguity is represented explicitly rather than being forced into a label
- `other` is no longer the implicit bucket for uncertainty
- uncovered residuals are represented as regions, not only as a flat index set
- the segmentation result is a better downstream atom for later aggregation or
  cluster-level review procedures

## Best Next Step

Use `SegmentedToolCallSequence` as a real intermediate state and build one
downstream procedure over it.

Good candidates:

- cluster-level usefulness review
- search-thrash characterization over segments
- segment-to-target narrowing review
- a residual/uncovered audit procedure that asks whether the remaining uncovered
  regions are noise or evidence of protocol weakness
