# Ploke-Protocol Control Note

- date: 2026-04-15
- task title: ploke-protocol control note
- task description: durable control note for the `ploke-protocol` architecture thread across forks, checkpoints, and restart surfaces
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/agents/2026-04-15_protocol-cold-start-reference.md`, `docs/active/agents/2026-04-15_ploke-protocol-architecture-checkpoint.md`, `docs/active/workflow/handoffs/recent-activity.md`

## Purpose

This note is the lightweight continuity surface for the `ploke-protocol`
subtrack.

Use it to answer:

- which checkpoint is currently authoritative
- which fork/thread lineage the current work belongs to
- what the next intended slice is
- which older notes are context only

## Current Status

- workstream: `A1`
- status: active
- current_thread: `fork-5` of the 2026-04-15 protocol architecture line
- current_focus: the protocol stack now has a shared local-analysis packet
  boundary plus the first downstream segment-level review; the next work should
  make packet-constructor comparison and disagreement more explicit and start
  aggregating packet-level assessments upward

## Authoritative Artifacts

### Current authority

- [2026-04-15_segment-review-packet-checkpoint.md](./2026-04-15_segment-review-packet-checkpoint.md)

This is the authoritative implementation checkpoint for the current
packet-based local review and segment-downstream protocol state.

### Supporting references

- [2026-04-15_protocol-artifact-persistence-handoff.md](./2026-04-15_protocol-artifact-persistence-handoff.md)
- [2026-04-15_protocol-cold-start-reference.md](./2026-04-15_protocol-cold-start-reference.md)
- [2026-04-15_intent-segmentation-semantics-checkpoint.md](./2026-04-15_intent-segmentation-semantics-checkpoint.md)
- [2026-04-15_ploke-protocol-neighborhood-review-checkpoint.md](./2026-04-15_ploke-protocol-neighborhood-review-checkpoint.md)
- [2026-04-15_ploke-protocol-state-composition-checkpoint.md](./2026-04-15_ploke-protocol-state-composition-checkpoint.md)
- [2026-04-15_ploke-protocol-architecture-checkpoint.md](./2026-04-15_ploke-protocol-architecture-checkpoint.md)
- [2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md](./2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md)
- [2026-04-15_ploke-protocol-fork-1-usage-checkpoint.md](./2026-04-15_ploke-protocol-fork-1-usage-checkpoint.md)

## Thread Lineage

- `original-thread`
  - conceptual alignment
  - cold-start reconnaissance
  - protocol architecture rewrite
  - ended with the authoritative architecture checkpoint
- `fork-1`
  - resumes from the architecture checkpoint
  - pressure-tests the new protocol against the older CLI-first useful workflow
  - established that the first protocol now runs live but is not yet
    competitively informative
  - recommends refining protocol usefulness before protocol-artifact persistence
- `fork-2`
  - resumes from the architecture and usage checkpoints plus the formal
    procedure notation draft
  - rewrites the crate toward typed procedure states and explicit fork/merge
    semantics
  - preserves the current `ploke-eval protocol tool-call-review` compile path
    while changing the internal execution model
  - recommends implementing the first genuinely useful multi-step or multi-branch
    protocol before persistence work
- `fork-3`
  - resumes from the state-composition checkpoint and the formal procedure
    notation draft
  - implements the first adapter-backed useful neighborhood review protocol
  - replaces the weak isolated-call review with mechanized context, forked
    adjudication, and explicit merge
  - recommends moving next toward reusable adapters, a second protocol,
    persisted artifacts, and calibration-aware surfaces
- `fork-4`
  - resumes from the persistence handoff and the existing segmentation path
  - refines the intent-segmentation contract so ambiguity is represented
    explicitly rather than being forced into labels or left as flat residual
    indices
  - promotes uncovered residuals into explicit contiguous spans plus coverage
    summary
  - recommends building the next downstream procedure over
    `SegmentedToolCallSequence` as a real intermediate state
- `fork-5`
  - resumes from the segmentation-semantics checkpoint and the forward-looking
    planning exercise
  - introduces a shared `LocalAnalysisPacket` boundary for local review
    procedures
  - adapts focal-call review onto that shared packet
  - adds the first segment-level downstream review command on the same
    assessment vocabulary
  - recommends making packet-constructor comparison and disagreement explicit
    next

## Intended Next Slice

Preferred next implementation order remains:

1. add a comparison or disagreement surface between neighborhood-based and
   segment-based local review
2. improve packet constructors and signals so segment-level and
   focal-call-level packets are both sharper and more comparable
3. add bounded mechanized aggregation from packet-level assessments toward
   turn/run-level supporting metrics
4. move subject projection out of ad hoc CLI helpers into clearer reusable
   `ploke-eval` adapter surfaces
5. begin adding calibration/disagreement surfaces so the long-view `10`
   milestone does not fall out of sight

## Supersession Rule

When a new fork or checkpoint becomes authoritative:

1. add it here under `Authoritative Artifacts`
2. update `current_thread`
3. move the previous authority to supporting context if superseded
4. leave a one-line statement describing what changed and why the authority
   moved

## Notes

- This note is intentionally small.
- Detailed implementation status belongs in checkpoint notes, not here.
- If chat context and repo state disagree, prefer this note plus the
  authoritative checkpoint over conversational memory.
