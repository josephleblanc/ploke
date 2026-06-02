# HyperAgents Prompt Posture For Broad Edit Surfaces

Status: draft notes from 2026-05-12 discussion.

## Context

The current Prototype 1 edit-surface direction should not regress into a
deterministic target-file picker. The live failure around
`workspace-except-ploke-eval` selecting archive Markdown files shows the wrong
shape: protocol or surface output was treated as if it knew the patch target.
That narrows broad self-improvement into a brittle splice fixture.

The intended route is closer to the HyperAgents/DGM-H setup:

- the outer loop owns archive, hydration, scoring, validity, and selection;
- the child edit harness receives a target checkout plus evidence from prior
  evaluation/protocol outcomes;
- the harness may choose what to edit inside the allowed artifact surface;
- `ploke-eval` checks the resulting proposal/touches against protected-core
  rules before admission;
- later evaluation and selection decide whether the edit was useful.

## Finding From HyperAgents

The HyperAgents paper prints a very small initial meta-agent prompt:

```text
Modify any part of the codebase at `{repo_path}`.
```

That prompt is not the whole mechanism. The operational mechanism is the outer
loop: selected parents are hydrated, modified, evaluated, validated, scored, and
returned to an archive. Previous evaluation results are present as files the
agent can inspect. Successful descendants later evolve their own evaluation
analysis, memory, prompt templates, performance tracking, and strategy logic.

The important lesson for Prototype 1 is subtlety: do not over-specify the edit
target in the prompt. Expose evidence and budget, describe the evaluation, and
let the harness choose the intervention.

## Proposed Prompt Posture

The prompt should point the agent at evidence rather than summarize it into a
command:

- target artifact/worktree path;
- prior evaluation/protocol output root;
- where benchmark/oracle results are recorded;
- where internal operational metrics are recorded;
- remaining generation or run budget;
- where the protected-core surface is defined in code.

The prompt should explain that the evaluation rewards future performance of the
resulting child artifact, not compliance with a particular suggested target. It
may say that protocol diagnoses are useful context, but they are not hard edit
targets.

The edit permission should be broad:

```text
You may edit any part of the target repository outside the protected core.
Use the evidence files to decide what improvement is most likely to help future
evaluations.
```

The protected-core list should be discoverable by pointing the agent at the code
that names it, rather than duplicating a stale list in the prompt. That location
should eventually carry a comment explaining that ordinary child edits to this
surface are expected to be rejected or to prevent the child process from being
admitted/started.

## Evidence Presentation

Prefer filesystem evidence roots over large prompt summaries. The harness should
be able to inspect:

- compact child outcome summaries;
- prior build/test/self-eval failures;
- protocol diagnoses and metrics;
- selection outcomes and scores;
- previous attempt evidence and rejected proposal reasons.

Held-out or selection-only oracle material should remain controlled. Training
or diagnostic evidence can be exposed if it is intended to teach the agent how
to improve.

## Non-Goals

- Do not convert protocol diagnoses into route-table file targets.
- Do not remove broad edit support to avoid bad target choices.
- Do not make `ploke-tui` proposal state or CLI text into authority.
- Do not duplicate the protected-core set in prompt prose as the source of
  truth.

## Working Contract Sketch

```text
You are modifying a child artifact for a self-improvement loop.

Target repository:
  <path>

Evidence you may inspect:
  <history/eval/protocol root>

Evaluation:
  <short benchmark/oracle description>

Remaining budget:
  <generations/children/attempts>

Protected core:
  See <code location that defines the protected surface>. Ordinary child edits
  touching that surface will be rejected or will prevent admission/startup.

Task:
  Inspect the repository and evidence. Choose and implement the improvement you
  believe is most likely to improve future evaluated descendants. Protocol
  diagnoses and metrics are guidance, not hard edit targets. You may edit any
  repository file outside the protected core.

Return evidence:
  - what files you changed;
  - what evidence guided the choice;
  - why the change should help future evaluations;
  - how the change should be checked.
```
