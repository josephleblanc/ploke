# 2026-04-13 Inspect CLI UX Orchestration Note

- date: 2026-04-13
- task title: bounded orchestration for inspect CLI UX follow-up
- task description: keep the current `ploke-eval inspect` UX lane coherent across review, test hardening, commit, loop-view implementation, and CLI-vs-non-CLI diagnostic comparison without expanding the broader control plane unless packet state materially changes
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/plans/evals/eval-design.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md`, `docs/active/agents/2026-04-13_inspect-turns-and-loop-ux-note.md`

## Purpose

This note is a temporary orchestration surface for the current inspect-CLI UX lane.
It is intentionally narrower than the main eval sprint control plane. It exists so
the next few sub-agent packets share one acceptance frame without creating broad
workflow churn.

## Lane Scope

Workstream:

- `A1` tool-system design and inspect-surface usability
- supporting `A5` only where inspection fidelity or measurement access affects
  trustworthy diagnosis

Boundaries:

- production changes remain inside `crates/ploke-eval/`
- no broad inspect CLI redesign
- no control-plane changes unless the lane produces a materially new accepted
  workflow conclusion

## Current Accepted Substrate

Already landed in the worktree:

- `inspect conversations` works as a compact turn-selection surface
- `inspect turns` exists as an alias-compatible clearer mental model
- `inspect turn 1` positional selection is supported
- `inspect turn 1` renders a compact dotted summary
- `inspect turn --show messages` supports role filtering
- next-step hints are present between overview and drilldown views

Known caveat:

- `turn.messages()` is prompt/response reconstruction, not the full agent-tool loop
- therefore the next bounded implementation slice is `inspect turn N --show loop`

## Sequencing

1. review the landed turn-selection UX slice for gaps and risks
2. add the highest-value missing tests for that slice
3. if the slice still stands, commit it as the stable baseline
4. implement `inspect turn N --show loop`
5. run a bounded CLI-UX evaluation wave:
   - CLI-only diagnostic pass
   - non-CLI comparison pass
   - metrics-focused CLI audit against `eval-design.md` outcome/validity metrics
6. synthesize whether the CLI meaningfully improves diagnostic access and what
   important metrics remain undersurfaced

## Current Packets

### Packet `ICU-REVIEW-1`

- owner_role: worker
- status: ready
- scope:
  - inspect the current `crates/ploke-eval/src/cli.rs` UX slice already in the
    worktree
  - identify concrete bugs, regressions, unclear UX edges, and missing tests
- non_goals:
  - do not implement the loop view yet
  - do not redesign unrelated inspect surfaces
- owned_files:
  - read-only review of `crates/ploke-eval/src/cli.rs`
- acceptance_criteria:
  1. produce a bounded findings list ordered by severity
  2. separate actual risks from speculative follow-up ideas
  3. identify the highest-priority test additions, if any
- required_evidence:
  - cited file references
  - specific CLI behavior or parsing path references
  - explicit `not_checked` and `risks`

### Packet `ICU-TEST-1`

- owner_role: worker
- status: ready
- scope:
  - based on the current landed UX slice, add the highest-priority missing tests
    inside `crates/ploke-eval/`
  - keep the test scope narrowly tied to the accepted turn-selection UX behavior
- non_goals:
  - no loop-view implementation yet
  - no unrelated CLI refactors
- owned_files:
  - tests under `crates/ploke-eval/`
  - if required, minimal supporting changes in `crates/ploke-eval/src/cli.rs`
- acceptance_criteria:
  1. cover the highest-risk behavior identified by review or direct inspection
  2. include meaningful assertions on observable CLI behavior
  3. report exactly what remains untested
- required_evidence:
  - changed file list
  - test command executed
  - concise output summary
  - explicit `not_checked` and `risks`

## Orchestrator Decision Rule

- commit the current slice only after the review and test wave do not surface a
  blocking correctness or usability issue
- if a gap is found, prefer the smallest patch that keeps the current command
  ladder coherent
- after commit, treat `inspect turn N --show loop` as the next implementation packet

## Resume Point

If this lane is resumed after interruption:

1. read this note
2. read `docs/active/agents/2026-04-13_inspect-turns-and-loop-ux-note.md`
3. inspect the latest `ICU-REVIEW-1` and `ICU-TEST-1` outputs
4. decide commit readiness before opening the loop-view implementation packet
