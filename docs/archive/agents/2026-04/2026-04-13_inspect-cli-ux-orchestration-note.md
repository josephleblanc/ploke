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

### Packet `ICU-LOOP-1`

- owner_role: worker
- status: ready
- scope:
  - implement `ploke-eval inspect turn N --show loop`
  - keep the implementation inside `crates/ploke-eval/`
  - add bounded tests for the new mid-level rendering path
- non_goals:
  - no broad redesign of other inspect surfaces
  - no attempt to redefine `messages` into a full transcript surface
- owned_files:
  - `crates/ploke-eval/src/cli.rs`
  - minimal supporting tests under `crates/ploke-eval/`
- acceptance_criteria:
  1. `inspect turn N --show loop` exists and is discoverable through the turn drilldown surface
  2. the default loop rendering provides a mid-level chronological view between the compact tool-call table and full single-call detail
  3. each loop step shows `input`, `status`, and `summary`
  4. failure steps include an error code and key diagnostics when available without dumping the full payload
  5. success steps include one or two informative tool-specific fields when available
  6. the loop view advertises the next narrower command shape
- required_evidence:
  - changed file list
  - targeted checks/tests run
  - concise live-output or renderer summary
  - explicit `not_checked` and `risks`

### Packet Family `ICU-COMPARE-*`

- owner_role: worker or explorer depending on packet
- status: proposed
- purpose:
  - test whether the CLI materially improves eval inspection relative to direct artifact reading
  - identify undersurfaced metrics and workflow gaps against `eval-design.md`
- intended packets:
  - `ICU-COMPARE-CLI`: analyze the latest eval using only `ploke-eval` CLI surfaces
  - `ICU-COMPARE-NONCLI`: analyze the same eval without using `ploke-eval` CLI surfaces
  - `ICU-COMPARE-METRICS`: assess the current CLI against core outcome and validity metrics from `eval-design.md` and related workflow docs
- expected output:
  - three short reports plus one orchestrator synthesis comparing evidence access, friction, blind spots, and missing CLI surfaces

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
