# Walk LLM Inspection Waits Behind Live Typestate Mutation

Status: source repaired and concurrency regression verified; fresh live revalidation pending
Discovered: 2026-07-14

## Summary

During the Stage 5 R5-to-R6 live baseline edge, `loop walk status` remained
responsive while `loop walk llm lanes` waited until the entire baseline eval
and protocol operation completed. The client connection stayed established
with empty socket queues; neither side was blocked on network I/O.

All persisted LLM inspection commands shared the same failure mode, including
`llm show`, `timeline`, `prompt`, `protocol`, and `tool`.

## Evidence

The live edge ran under walk session:

```text
session_id = abe149b1-07fe-42bf-bfe1-d5d43e1b9a37
R5 -> R6 operation = d7d3d5e4-77c2-40dc-95b4-cca256df52d9
```

While the edge was active, the mutable baseline trace was being updated under:

```text
prototype1/debug/tool-loop
```

The filesystem checkpoint was readable directly. `llm lanes` did not return
until the outer operation released the controller, then reported no nested
broad-harness lanes because the baseline trace is a different live surface.

## Root Cause

`run_step_job` holds `Arc<tokio::Mutex<WalkController>>` across the awaited
typestate edge. That lock is intentional mutation authority and must not be
released mid-transition. The read-only LLM request arms nevertheless awaited
the same mutex before opening persisted tool-loop checkpoints.

`status` avoided the problem by using durable session evidence and a
non-blocking controller/cache fallback. LLM inspection had no equivalent
separation, and the client had no response timeout, so the authority wait
looked like an indefinite inspector hang.

## Broken Contract

Read-only observation of already-persisted LLM/tool-loop checkpoints must
remain available while a live typestate mutation owns controller authority.
Inspection must not weaken, split, or temporarily release mutation authority.
Navigation focus and cursors are observer state, not typestate state.

## Fix

Persisted LLM navigation now lives in a separate `LlmInspector` containing the
repository root, lane focus, and checkpoint cursors. Read-only inspector and
navigation requests lock that state and obtain their displayed phase from the
durable session snapshot. The typestate controller retains its original lock
lifetime.

Effectful `llm step` and `llm finish` remain subject to the existing mutation
job admission/transfer gate. Start/reset clears inspector navigation without
moving typestate authority into the inspector.

## Regression

The server concurrency regression holds the typestate controller lock to model
an active live edge, then issues real `LlmLanes` and `LlmShow` requests against
persisted tool-loop checkpoints. Both must return a typed response within a
short timeout while the controller remains held.

## Remaining Verification

On the next fresh live baseline edge, use the CLI and UI to inspect both a lane
list and a detail checkpoint before the edge completes. Baseline-eval trace
discovery also remains a separate product gap: the current lane root describes
nested broad-harness tool loops, not the baseline eval's live trace artifact.
