# EDR-0003: Protocol Diagnosis Workflow Experiment

- status: active
- date: 2026-04-17
- owning_branch: `refactor/tool-calls`
- review_cadence: revisit if the workflow is promoted into a durable skill or if a later trial shows drift
- update_trigger: update when the workflow is materially revised, re-trialed, or promoted beyond the current active-doc form
- owners: orchestrator, future sub-agent operators
- hypothesis ids: `A1`, `A5`
- related issues/prs: none yet
- linked manifest ids: none
- linked artifacts:
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/packet-P0-campaign-protocol-triage.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_protocol-triage-implementation-report.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-synthesis.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_protocol-diagnosis-workflow.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_protocol-diagnosis-subagent-template.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-search-thrash.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-request-code-context.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-read-file.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-partial-next-step.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-artifact-errors.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-read-file-partial-next-step.md`

## Decision

Run a bounded parallel trial of a protocol-driven diagnosis workflow before
relying on it as the default way to turn campaign protocol data into production
`ploke` / `ploke-tui` improvement hypotheses.

## Why Now

The new campaign triage surface in `ploke-eval` makes it possible to move from
campaign-level protocol evidence to one bounded issue/tool/status slice, but the
workflow for using that surface was not yet stable enough to trust by default.

This was a materially diagnostic workflow change because the question was not
only "can we inspect the data?" but "can multiple agents use the same workflow
and reach similar, actionable conclusions without drifting into the wrong code
layer?"

The main risk was exactly what the trial needed to test:

- issue-only slices might collapse into `ploke-eval` / protocol-analysis work
- tool-focused slices might produce better production-facing recommendations
- artifact/status slices might need a different ownership path than tool
  diagnosis

## Control And Treatment

- control:
  ad hoc diagnosis prompts without a fixed workflow, ownership gate, or fixed
  report schema
- treatment:
  a bounded workflow using campaign triage, bounded slice selection, exemplar
  review, code-surface inspection, and a ranked intervention ladder
- frozen variables:
  same campaign (`rust-baseline-grok4-xai`), same `ploke-eval` campaign triage
  surface, same top-level report headings, same intervention ladder shape, and
  one slice per agent

## Acceptance Criteria

- primary:
  sub-agents can use the workflow to produce concise reports with one bounded
  problem family, one plausible owning surface, and one concrete recommended
  next move
- secondary:
  multiple sub-agents converge on similar intervention shapes for tool-focused
  slices, and the workflow reveals where issue-only slices are under-specified
- validity guards:
  each agent stays inside one slice, inspects only a small exemplar set, writes
  only its assigned report, and does not implement fixes during the trial

## Plan

1. Implement the campaign triage surface needed to support the workflow.
2. Run several sub-agents in parallel over different slice types.
3. Compare convergence, drift, and recommendation quality.
4. Revise the workflow only after the trial results are in.

## Result

- outcome:
  first bounded trial completed; external usefulness still unvalidated
- key metrics:
  qualitative workflow-comparison outcome only; no formal run manifests and no
  independent quality scoring of the resulting recommendations
- failure breakdown:
  - tool-focused and combined tool+issue slices produced the most
    production-facing recommendations
  - `status=error` worked well for artifact/schema ownership
  - issue-only slices (`search_thrash`, `partial_next_step`) drifted toward
    `ploke-eval` / protocol-calibration work unless ownership was forced
- surprises:
  - the workflow was stable enough to run in parallel without extra handholding
  - the combined slice (`read_file` + `partial_next_step`) constrained
    exemplars and ownership better than the pure issue slice
  - the main missing structure was the explicit ownership decision before code
    inspection
  - none of the above proves that the resulting recommendations are actually
    good, non-garbage, or superior to operator judgment

## Decision And Follow-Up

- adopt / reject / inconclusive:
  inconclusive
- next action:
  treat the current workflow and sub-agent template as experimental scaffolding
  only; do not treat the workflow as validated until a follow-up pass evaluates
  recommendation quality against operator judgment or downstream implementation
  outcomes. Keep the ownership-gate refinement, but require an explicit
  validation step before promotion.
