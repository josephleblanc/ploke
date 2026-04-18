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
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_edr-0003-downstream-validation-plan.md`
  - `docs/active/agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_edr-0003-baseline-cohort-report.md`

## Decision

Run a bounded downstream cohort-replay experiment to test whether the
protocol-driven diagnosis workflow can select a production-harness change that
improves real Multi-SWE-bench outcomes on a fixed cohort.

## Why Now

The workflow trial established that the process is runnable, but not that its
recommendations are good. We already have a local adjudication system
(`ploke-protocol`) for trace evidence and a local Multi-SWE-bench harness for
hard downstream outcomes. That means the next validation step should use those
existing surfaces directly rather than inventing a new recommendation-scoring
oracle.

The real question is now:

- can the workflow choose a bounded production-harness intervention
- that intervention can be implemented
- and the resulting change improves hard benchmark results on a fixed cohort

The current pilot uses a protocol-derived `request_code_context +
search_thrash` cohort with existing non-empty submissions and a local
Multi-SWE-bench runner path.

## Control And Treatment

- control:
  current production harness and current submission artifacts for the fixed
  cohort
- treatment:
  one bounded production-harness change selected through the structured
  protocol diagnosis workflow and evaluated on the same cohort
- frozen variables:
  same cohort, same benchmark harness, same dataset file, same evaluation
  config shape, and the same adjudicated protocol evidence surface used to
  choose the intervention

## Acceptance Criteria

- primary:
  treatment improves `resolved_instances` on the fixed cohort relative to the
  baseline submission set
- secondary:
  treatment does not increase benchmark `error_instances` or
  `incomplete_instances`, and at least one targeted protocol-slice signal moves
  in the expected direction on rerun traces
- validity guards:
  fixed cohort, one bounded production change, same benchmark harness config
  shape, and explicit `inconclusive` handling if provider/harness instability
  dominates interpretation

## Plan

1. Establish a hard Multi-SWE-bench baseline for the fixed cohort.
2. Run the diagnosis workflow on the selected protocol slice and choose one
   bounded production-harness intervention.
3. Implement that intervention.
4. Regenerate submissions for the same cohort.
5. Re-run Multi-SWE-bench on the same cohort and compare the before/after
   result plus the targeted protocol-slice deltas.

## Result

- outcome:
  workflow trial completed and hard downstream baseline established; treatment
  not yet run
- key metrics:
  - workflow trial: qualitative only
  - downstream baseline cohort: `0/5` resolved, `5/5` unresolved
- failure breakdown:
  - tool-focused and combined tool+issue slices produced the most
    production-facing recommendations
  - `status=error` worked well for artifact/schema ownership
  - issue-only slices (`search_thrash`, `partial_next_step`) drifted toward
    `ploke-eval` / protocol-calibration work unless ownership was forced
  - downstream pilot baseline on the selected `request_code_context +
    search_thrash` `clap` cohort produced:
    - `3/5` invalid benchmark reports with no fix-stage test results captured
    - `2/5` benchmark reports where fix-stage tests ran but solved nothing
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
  baseline is now established; run the structured diagnosis workflow on the
  selected slice, choose one bounded production-harness intervention, implement
  it, regenerate the cohort submissions, and compare the treatment
  `final_report.json` plus rerun protocol-slice deltas against the recorded
  baseline.
