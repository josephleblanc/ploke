# 2026-05-19 Orchestrator Survey Collapsed To Short Slice

## Trigger

The user said: "Ok, I don't think that's what I asked you to do. What did I ask?"

## User-Visible Failure

The agent was asked to orchestrate a survey of existing `ploke-eval` CLI
protocol/stat tooling and turn that survey into a `ploke-egui` observability
and introspection design for tool calls, protocols, metrics, and proof of loop
improvement. The final answer collapsed the result into a short verdict and one
recommended implementation slice.

## Touched Surface

- `.orchestrator/board.json`
- `.orchestrator/workers/*`
- `.orchestrator/reports/obs-*.md`

No tracked product source files were edited before the user objected.

## What The Agent Did

- Correctly used the requested orchestration skills and spawned read-only
  survey agents.
- Recorded board reports for the survey wave.
- Failed at the communication boundary by reducing the requested survey,
  representation design, and proof framing into a brief "do not port CLI
  reports" recommendation plus one first implementation slice.

## Skipped Or Underweighted Instructions

- The user asked for a survey of commands and how to represent their outputs in
  the UI; the response did not enumerate the surveyed command families and UI
  representations in enough detail.
- The user asked for high-quality data-analysis representation, allocation
  discipline, and proof framing; the response compressed those into a few chart
  bullets instead of a structured design.
- The light-thread orchestrator role should preserve the semantic frame and
  synthesize worker findings; it should not make board bookkeeping the main
  deliverable.

## Why This Was Risky

The next implementation work could start from an underspecified slice and miss
the larger product goal: `ploke-egui` as an evidence/debugging surface that
connects tool behavior, protocol adjudication, selection metrics, and
multi-generation improvement claims without overclaiming causality or creating
hot-path allocation churn.

## Prevention Rule

When the user asks for an orchestrated survey, the final answer must mirror the
requested survey dimensions before proposing implementation:

1. Commands and existing outputs surveyed.
2. Source facts and typed projection boundary.
3. UI panels and chart forms.
4. Performance/cache/allocation boundary.
5. Proof claim supported now versus later.
6. Concrete next implementation slices.

Do not collapse a broad survey into only the first implementation slice.

## Memory Hypothesis

Prior memory strongly emphasized "conclusion first" and compact verdicts for
operator questions. That was misapplied here. This was not a simple operator
health question; it was a design/survey synthesis request that needed more
structure.
