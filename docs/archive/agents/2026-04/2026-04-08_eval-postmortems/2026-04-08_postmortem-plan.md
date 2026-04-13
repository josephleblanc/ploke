# Eval Postmortems Plan

- date: 2026-04-08
- task title: Eval Postmortems Workspace
- task description: Create a shared workspace for investigating eval failures, separating model mistakes from tool-design failures, and collecting open questions that should inform future tool and runner changes.
- related planning files:
  - [2026-04-08_postmortem-plan.md](2026-04-08_postmortem-plan.md)

## Purpose

This directory is for post-mortems of eval runs where the result was surprising,
misleading, or hard to diagnose from the final artifacts alone.

The main goal is to keep three things separate:

1. Model mistake
2. Tool-design friction
3. Runner or artifact interpretation issue

## Directory Scope

Use this directory for:

- benchmark run investigations
- tool call recovery problems
- misleading success/failure signals in run artifacts
- cases where logs show the model was locally coherent but still failed

For batch evals:

- create one subdirectory per batch under this workspace
- create one subdirectory per instance inside the batch directory
- keep each instance post-mortem self-contained, with explicit links to:
  - `run.json`
  - `execution-log.json`
  - `agent-turn-summary.json`
  - `agent-turn-trace.json`
  - `multi-swe-bench-submission.jsonl`
  - any official benchmark follow-up logs or reports, if available

Do not use this directory for:

- one-off bug notes with no run evidence
- feature planning that is not tied to a concrete failure
- generic implementation notes better suited to crate docs

## Postmortem Checklist

- Identify the stable source of truth for the run.
- Record whether this run was part of a batch and link the batch manifest.
- Record the exact artifact paths used for the write-up.
- Note whether run-directory artifacts are mixed or overwritten.
- Extract the first meaningful failure, not just the last warning.
- Separate tool errors that were recovered from tool errors that changed model behavior.
- Distinguish the minimal correct fix from the fix the model attempted.
- Distinguish local `ploke-eval` success signals from official benchmark outcomes when both exist.
- Record whether the failure was primarily:
  - model judgment
  - missing tool affordance
  - bad tool recovery instruction
  - misleading tool result payload
  - runner artifact ambiguity
- Capture open questions that require code inspection before changing behavior.

## Proposed Labels

- `model-drift`
- `tool-affordance-gap`
- `tool-retry-friction`
- `semantic-edit-limitation`
- `artifact-ambiguity`
- `provider-behavior`

## Initial Open Questions

- Does `apply_code_edit` have a first-class way to insert a new child node under an existing owner, or does it only replace existing canonical targets?
- What are the intended semantics for semantic delete and move operations in an auto-approve workflow?
- Are `method` vs `function` recovery hints helping, or are they pushing the model into oscillation without clarifying the real lookup target?
- Should post-run classification distinguish "correct diagnosis but wrong edit strategy" from "incorrect diagnosis"?
- Which fields should be added to batch summaries so post-mortems do not need to reconstruct run progress from per-instance artifacts?
