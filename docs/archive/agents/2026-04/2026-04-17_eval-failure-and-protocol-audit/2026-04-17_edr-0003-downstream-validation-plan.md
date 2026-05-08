# 2026-04-17 EDR-0003 Downstream Validation Plan

- date: 2026-04-17
- task title: downstream validation experiment for the protocol diagnosis workflow
- task description: validate the workflow behind `EDR-0003` against hard
  Multi-SWE-bench outcomes on a fixed protocol-derived cohort rather than
  against a newly invented recommendation oracle
- related planning files:
  - [2026-04-17_protocol-diagnosis-workflow.md](./2026-04-17_protocol-diagnosis-workflow.md)
  - [2026-04-17_protocol-diagnosis-subagent-template.md](./2026-04-17_protocol-diagnosis-subagent-template.md)
  - [2026-04-17_workflow-trial-synthesis.md](./2026-04-17_workflow-trial-synthesis.md)
  - [EDR-0003-protocol-diagnosis-workflow-experiment.md](../../workflow/edr/EDR-0003-protocol-diagnosis-workflow-experiment.md)

## Purpose

Validate the workflow by asking a hard downstream question:

> if the workflow selects the next production-harness intervention, does that
> intervention improve actual Multi-SWE-bench outcomes on a fixed cohort?

This avoids inventing a new recommendation-quality oracle. The protocol is
already the adjudication layer for trace evidence. The remaining validation step
is whether workflow-guided interventions move downstream benchmark results and
the protocol-derived slice metrics they claim to target.

## Existing Local Assets

The experiment uses only surfaces that already exist locally:

- protocol-adjudicated campaign data through
  `./target/debug/ploke-eval inspect protocol-overview --campaign ...`
- per-run submission artifacts under
  `~/.ploke-eval/runs/<instance>/multi-swe-bench-submission.jsonl`
- local Multi-SWE-bench checkout at
  `/home/brasides/code/github_clones/benches/multi-swe-bench`
- local Multi-SWE-bench work/output roots under `~/.ploke-eval/msb/`
- campaign closure snapshot for `rust-baseline-grok4-xai`

## Chosen Pilot Cohort

Use one coherent production-harness slice:

```text
ploke-eval inspect protocol-overview \
  --campaign rust-baseline-grok4-xai \
  --tool request_code_context \
  --issue search_thrash
```

Reason for choosing it:

- same likely owner: `ploke-tui` tool harness
- high frequency: `89` affected runs, `471` matching calls
- coherent trace story: repeated refinement/search churn around
  `request_code_context`
- enough non-empty current submissions exist to support a real downstream
  benchmark baseline

Fixed pilot cohort:

- `clap-rs__clap-4059`
- `clap-rs__clap-5527`
- `clap-rs__clap-5489`
- `clap-rs__clap-5520`
- `clap-rs__clap-3521`

These five were chosen because they:

- belong to the selected protocol slice
- share the same repo family (`clap-rs__clap`)
- already have non-empty submission artifacts on disk

## Environment Baseline

Frozen environment for the experiment:

- repo revision: `1d4833dbfb2b40d422ac77b75f271be6e131fdef`
- branch at experiment start: `experiment-protocol-diagnosis-validation`
- campaign: `rust-baseline-grok4-xai`
- campaign closure snapshot:
  `2026-04-18T03:15:47.411876474+00:00`
- eval totals:
  - `221` complete
  - `18` failed
  - `0` missing
- protocol totals:
  - `134` full
  - `1` partial
  - `9` failed
  - `0` missing
  - `77` ineligible

The workflow-validation experiment does **not** use those protocol totals as the
endpoint. They are the frozen adjudicated environment.

## Hypotheses

Primary hypothesis:

> A production-harness intervention selected through the structured protocol
> diagnosis workflow will increase resolved instances on the fixed pilot cohort,
> relative to the current baseline submissions for that same cohort.

Null hypothesis:

> The workflow-guided intervention will not increase resolved instances on the
> fixed pilot cohort relative to the current baseline submissions.

Secondary hypothesis:

> If the workflow-guided intervention is real rather than cosmetic, it should
> also reduce the targeted protocol slice signature on rerun traces:
> `request_code_context + search_thrash`.

## Endpoints

Primary endpoint:

- Multi-SWE-bench `final_report.json`
  - `resolved_instances`
  - on the fixed `5`-instance cohort

Secondary endpoints:

- `unresolved_instances`
- `incomplete_instances`
- `error_instances`
- protocol-derived rerun slice deltas on the same cohort:
  - matching `request_code_context + search_thrash` calls
  - affected runs
  - nearby `refine_search` / `mixed` pattern if materially changed

## Design

Use a bounded `cohort_replay` design.

### Control

Current production harness and current submission artifacts for the fixed cohort.

### Treatment

One bounded production-harness change chosen by the structured protocol
diagnosis workflow for the selected slice.

### Frozen Variables

- same fixed cohort of `5` instances
- same benchmark dataset file
- same Multi-SWE-bench harness runner
- same evaluation config shape
- same model/provider family for regenerated runs unless explicitly waived
- same runtime budgets and retry policy unless the intervention itself requires
  a documented change

### Single substantive variable

Only the chosen production-harness intervention should change between control
and treatment.

## Execution Plan

1. Establish the hard baseline.
   - Run local Multi-SWE-bench evaluation over the current submission artifacts
     for the fixed cohort.
   - Record the resulting `final_report.json`.
2. Run the structured diagnosis workflow on the selected slice.
   - Use the existing protocol triage CLI plus exemplar review.
   - Choose one bounded intervention with explicit `metric_to_move`.
3. Implement exactly one bounded production-harness change.
4. Regenerate submissions for the same `5` cohort instances.
5. Run the same Multi-SWE-bench evaluation again over the regenerated
   submission artifacts.
6. Compare:
   - hard benchmark outcome delta
   - protocol slice delta on rerun traces
7. Update `EDR-0003` with the result and decision.

## Validity Guards

- no widening beyond the fixed cohort during the pilot
- no multiple simultaneous production changes
- do not change the benchmark harness config except for:
  - `patch_files`
  - `output_dir`
  - `log_dir`
- if treatment runs fail due to provider or harness instability unrelated to the
  intervention, mark the result `inconclusive`
- if regenerated submissions are empty for most of the cohort, do not pretend
  that a downstream benchmark failure is meaningful evidence of workflow quality

## Acceptance Criteria

Primary:

- treatment `resolved_instances` is greater than baseline `resolved_instances`
  on the same `5`-instance cohort

Secondary:

- no increase in `error_instances` or `incomplete_instances`
- at least one targeted protocol-slice signal moves in the expected direction on
  the rerun cohort

Interpretation:

- if the hard benchmark result improves, the workflow has at least one concrete
  downstream success case
- if the benchmark result does not improve, the workflow remains unvalidated for
  downstream usefulness
- if the local protocol metric improves but benchmark outcomes do not, treat the
  result as mixed rather than successful

## Baseline Runner Surface

Current baseline config file:

- `/tmp/msb-clap-request-code-context-search-thrash-baseline-20260417.json`

Current baseline command:

```bash
/home/brasides/code/github_clones/benches/multi-swe-bench/.venv/bin/python \
  -m multi_swe_bench.harness.run_evaluation \
  --config /tmp/msb-clap-request-code-context-search-thrash-baseline-20260417.json
```

The expected benchmark artifact is:

- `~/.ploke-eval/msb/output/edr-0003-clap-request-code-context-search-thrash-baseline-20260417/final_report.json`

Current recorded baseline:

- see [2026-04-17_edr-0003-baseline-cohort-report.md](./2026-04-17_edr-0003-baseline-cohort-report.md)
- hard result:
  - `resolved_instances = 0/5`
  - `unresolved_instances = 5/5`
  - `3` benchmark invalidations where no fix-stage test results were captured
  - `2` benchmark invalidations where fix-stage tests ran but solved nothing

## What This Experiment Does Not Claim

- it does not prove the workflow is universally superior
- it does not compare against an ad hoc diagnosis control arm yet
- it does not validate every slice type

It is a first hard downstream test: can the workflow help choose a production
fix that improves real benchmark outcomes on a bounded cohort?
