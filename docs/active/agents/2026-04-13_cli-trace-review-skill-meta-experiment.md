# CLI Trace Review Skill Meta-Experiment

- date: 2026-04-13
- task title: CLI trace review skill meta-experiment
- task description: Compare several instruction templates for `ploke-eval` CLI-only run-trace review before promoting the workflow into a durable repo-local skill.
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/README.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/plans/evals/phased-exec-plan.md`, `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md`, `docs/workflow/skills/postmortem-protocol/SKILL.md`, `docs/workflow/skills/experiment-cycle/SKILL.md`, `docs/active/workflow/edr/EDR-0002-cli-trace-review-skill-experiment.md`

## Purpose

This is a bounded meta-experiment for a repeated eval workflow:
review the latest `ploke-eval` run through CLI inspection commands only, treat
the tool-call trace as a behavioral narrative, avoid premature model blame, and
identify where tool robustness or tool workflow design likely constrained the
run.

The goal is not to freeze the final skill immediately. The goal is to compare a
few plausible instruction sets, evaluate the outputs, revise once, and then
promote the winning workflow into `docs/workflow/skills/` when the method is
stable enough.

## Hard Boundaries

- Use `ploke-eval` CLI inspection commands only.
- Do not read or edit `crates/ploke-eval/` source while running this workflow.
- Treat the latest run as the default target unless the packet explicitly names
  another run.
- Prefer actionable system-side explanations over model-only blame when the
  evidence is ambiguous.
- The trace should be read as a sequence with turning points, not as isolated
  rows.

## Fixed Evidence Surface

All instruction variants should use the same basic drill-down sequence unless a
packet narrows it further:

1. `ploke-eval inspect tool-calls`
2. Select one or more notable call IDs from the table.
3. `ploke-eval inspect tool-calls <id>`
4. `ploke-eval inspect tool-calls --full <id>`
5. Repeat for additional calls that materially change the interpretation.

Optional follow-up commands may be added later, but the first comparison round
should keep the evidence surface close to this sequence so prompt quality is the
main variable.

## Shared Output Schema

Every variant should return the same top-level sections:

- `run_scope`
- `commands_executed`
- `trace_summary`
- `turning_points`
- `notable_failures`
- `recoverability_gaps`
- `tool_workflow_gaps`
- `model_failure_candidates`
- `unsupported_model_blame`
- `proposed_tool_improvements`
- `claims`
- `evidence`
- `unsupported_claims`
- `not_checked`
- `risks`
- `smallest_credible_follow_up`

### Field Expectations

- `run_scope`
  - identify which run or default surface was inspected
- `commands_executed`
  - list the exact CLI commands used during the review
- `trace_summary`
  - summarize the run as a short behavioral narrative
- `turning_points`
  - list moments where the run changed direction, got stuck, or recovered
- `notable_failures`
  - list the highest-signal failed or misleading tool calls
- `recoverability_gaps`
  - identify places where the tool surface should have helped the model recover
- `tool_workflow_gaps`
  - identify missing affordances, weak tool sequencing, or unclear UX
- `model_failure_candidates`
  - list possible model mistakes, but only when the tool support was otherwise
    adequate
- `unsupported_model_blame`
  - list any tempting model-blame claims that the evidence does not yet justify
- `proposed_tool_improvements`
  - propose the narrowest useful improvements suggested by the trace
- `claims`
  - numbered propositions tied to concrete evidence
- `evidence`
  - exact commands, call IDs, and short artifact observations
- `unsupported_claims`
  - claims considered but rejected because the trace did not support them
- `not_checked`
  - anything outside the bounded CLI evidence surface
- `risks`
  - realistic ways the interpretation could still be wrong
- `smallest_credible_follow_up`
  - the next best bounded action

## Comparison Rubric

Use this rubric when comparing outputs across variants:

1. Did the review stay within the CLI-only evidence boundary?
2. Did it treat the trace as a sequence rather than a pile of rows?
3. Did it identify recoverability and tool-UX gaps precisely?
4. Did it avoid unsupported model blame?
5. Did the proposed follow-up actions feel narrow and actionable?
6. Did the output fit the repo's claims/evidence/risks reporting style?
7. Would this output be usable inside a packet report or postmortem without
   major rewriting?

## Variant A: Narrative-First

### Intent

Optimize for reading the run as a story with turning points, repeated attempts,
and points where the harness or tool surface failed to steer the model.

### Instruction Template

You are reviewing the latest `ploke-eval` run through CLI inspection commands
only. Do not read source code. Start with `ploke-eval inspect tool-calls`, then
drill into the smallest number of call IDs needed to explain the run's
trajectory. Treat the tool-call trace as a behavioral narrative. Identify where
the model's working hypothesis changed, where it became stuck, and where tool
design or recovery affordances likely shaped the outcome.

Bias away from premature model blame. If a failure could plausibly be explained
by weak error recovery, ambiguous tool semantics, poor next-step affordances, or
missing guidance in the tool surface, call that out before assigning likely
model-only failure.

Return the shared output schema. Keep `trace_summary` and `turning_points`
strong. Use `model_failure_candidates` narrowly.

## Variant B: Failure-Classification-First

### Intent

Optimize for disciplined classification of notable failures with a strong bias
toward actionable tool-side interpretation.

### Instruction Template

You are reviewing the latest `ploke-eval` run through CLI inspection commands
only. Do not read source code. Start with `ploke-eval inspect tool-calls`, then
inspect the highest-signal failed, repeated, or suspicious call IDs with
`ploke-eval inspect tool-calls <id>` and `--full <id>`.

For each notable event, classify it into one of four buckets:

- exposed tool robustness gap
- exposed tool workflow or UX gap
- ambiguous shared-responsibility failure
- likely model-only failure

Use the earliest credible cause over downstream noise. Treat `likely
model-only failure` as the hardest bucket to assign. If the tool emitted an
unhelpful error class, lacked a recovery hint, exposed misleading semantics, or
failed to support the obvious next step, prefer a tool-side classification.

Return the shared output schema. Keep `notable_failures`,
`recoverability_gaps`, and `unsupported_model_blame` strong.

## Variant C: Intervention-First

### Intent

Optimize for identifying the smallest product or harness improvements that would
most likely have prevented the observed failure chain.

### Instruction Template

You are reviewing the latest `ploke-eval` run through CLI inspection commands
only. Do not read source code. Start with `ploke-eval inspect tool-calls`, then
drill into the call IDs most likely to reveal why the run got stuck or took a
wasteful path.

Your goal is not only to explain the run. Your goal is to identify the smallest
high-leverage intervention that would have made the run more recoverable or more
efficient. Prefer concrete tool-surface improvements over abstract criticism:
better error classes, populated retry hints, "did you mean" recovery, improved
tool descriptions, or outputs that better set up the next likely step.

Return the shared output schema. Keep `proposed_tool_improvements` and
`smallest_credible_follow_up` strong. Do not skip the causal interpretation just
because you are intervention-focused.

## First-Round Execution Plan

1. Use one sub-agent per variant against the same latest-run target.
2. Keep the CLI evidence surface fixed unless one variant clearly needs one
   extra command to justify a claim.
3. Compare the three outputs with the rubric above.
4. Revise the strongest variant once rather than averaging all three together.
5. Run a second round against a fresh sample or the same sample with a tighter
   rubric.
6. Promote the winning variant into a durable skill only after the second round
   is credibly stable.

## Promotion Criteria For A Durable Skill

Promote this workflow into `docs/workflow/skills/` when all of the following are
true:

1. One variant consistently produces useful outputs without code-reading drift.
2. The output schema is stable enough to reuse in packet reports or postmortems.
3. The workflow reliably identifies tool robustness or workflow gaps before
   resorting to model-only blame.
4. The remaining prompt changes are editorial rather than conceptual.
