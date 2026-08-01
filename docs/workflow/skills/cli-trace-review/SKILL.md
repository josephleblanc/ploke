---
name: cli-trace-review
description: Use this skill when reviewing a `ploke-eval` run through CLI inspection commands only, especially to read tool-call traces as a behavioral sequence, identify tool robustness and workflow gaps, and avoid unsupported model blame.
---

# CLI Trace Review

Use this skill for CLI-first review of a `ploke-eval` run when the goal is to
understand what the trace says about tool robustness, tool workflow design, and
model behavior without reading implementation code.

## Boundaries

- Use `ploke-eval` CLI inspection commands only.
- Do not read or edit `crates/ploke-eval/` source while applying this skill.
- Treat the latest run as the default target unless a packet names another run.
- Prefer actionable system-side explanations over model-only blame when the
  evidence is ambiguous.
- Read the trace as a sequence with turning points, not as isolated rows.

## Walkthrough

1. Start with `ploke-eval inspect tool-calls`.
2. Select the smallest set of notable call IDs needed to explain the run's
   trajectory.
3. Inspect each selected call with:
   - `ploke-eval inspect tool-calls <id>`
   - `ploke-eval inspect tool-calls --full <id>`
4. Group repeated failures into clusters when they are clearly the same pattern.
5. Identify turning points:
   - first hard failure
   - first credible recovery
   - repeated thrash or loop behavior
   - late-stage failure or recovery
6. Classify notable failures with a bias toward the earliest credible cause:
   - exposed tool robustness gap
   - exposed tool workflow or UX gap
   - ambiguous shared-responsibility failure
   - likely model-only failure
7. Treat `likely model-only failure` as the hardest bucket to assign.

## What To Look For

- Missing or unhelpful `retry_hint`
- Wrong error classes for recoverable situations
- “Successful but misdirective” tool results
- Repeated semantically similar calls that suggest search thrash
- Missing next-step affordances after a tool response
- Evidence that a failure became recoverable only after the tool finally gave a
  usable hint

## Output

Return these sections:

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

## Guardrails

- Prefer one highest-leverage intervention over a long backlog of ideas.
- If a failure can be explained by weak recovery semantics, unclear tool
  workflow, or missing affordances, say that before blaming the model.
- Do not claim a root cause that the CLI trace does not support.
- Keep the output usable in a packet report or postmortem with minimal rewrite.
