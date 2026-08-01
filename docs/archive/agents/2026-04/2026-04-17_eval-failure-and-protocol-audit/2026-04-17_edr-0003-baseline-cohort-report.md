# 2026-04-17 EDR-0003 Baseline Cohort Report

- date: 2026-04-17
- task title: hard downstream baseline for the EDR-0003 pilot cohort
- task description: record the actual Multi-SWE-bench outcome for the fixed
  `request_code_context + search_thrash` pilot cohort before any
  workflow-guided production intervention
- related planning files:
  - [2026-04-17_edr-0003-downstream-validation-plan.md](./2026-04-17_edr-0003-downstream-validation-plan.md)
  - [EDR-0003-protocol-diagnosis-workflow-experiment.md](../../workflow/edr/EDR-0003-protocol-diagnosis-workflow-experiment.md)

## Scope

Fixed cohort:

- `clap-rs__clap-4059`
- `clap-rs__clap-5527`
- `clap-rs__clap-5489`
- `clap-rs__clap-5520`
- `clap-rs__clap-3521`

All five belong to the protocol slice:

```text
ploke-eval inspect protocol-overview \
  --campaign rust-baseline-grok4-xai \
  --tool request_code_context \
  --issue search_thrash
```

## Claims

1. A hard downstream baseline now exists for the fixed pilot cohort.
2. The current submission set resolves `0/5` cohort instances under the local
   Multi-SWE-bench harness.
3. The cohort baseline failure mode splits into two concrete benchmark-side
   families:
   - `3/5` invalid because no fix-stage test results were captured
   - `2/5` invalid because no failed tests transitioned to passing

## Evidence

- Baseline config:
  - `/tmp/msb-clap-request-code-context-search-thrash-baseline-20260417.json`
- Benchmark runner:
  - `/home/brasides/code/github_clones/benches/multi-swe-bench/.venv/bin/python -m multi_swe_bench.harness.run_evaluation --config /tmp/msb-clap-request-code-context-search-thrash-baseline-20260417.json`
- Final report:
  - [`final_report.json`](/home/brasides/.ploke-eval/msb/output/edr-0003-clap-request-code-context-search-thrash-baseline-20260417/final_report.json)
- Benchmark logs:
  - [`run_evaluation.log`](/home/brasides/.ploke-eval/msb/logs/edr-0003-clap-request-code-context-search-thrash-baseline-20260417/run_evaluation.log)
  - [`gen_report.log`](/home/brasides/.ploke-eval/msb/logs/edr-0003-clap-request-code-context-search-thrash-baseline-20260417/gen_report.log)

### Final report summary

- `total_instances = 5`
- `submitted_instances = 5`
- `completed_instances = 5`
- `incomplete_instances = 0`
- `resolved_instances = 0`
- `unresolved_instances = 5`
- `empty_patch_instances = 0`
- `error_instances = 0`

### Benchmark-side invalidation pattern

From `gen_report.log`:

- `clap-rs/clap:pr-5527`
  - no test results were captured when executing the fix-stage test command
- `clap-rs/clap:pr-5520`
  - no test results were captured when executing the fix-stage test command
- `clap-rs/clap:pr-5489`
  - no test results were captured when executing the fix-stage test command
- `clap-rs/clap:pr-4059`
  - no failed tests transitioned from failed to passed
- `clap-rs/clap:pr-3521`
  - no failed tests transitioned from failed to passed

Collapsed count:

- `3` instances: `fix = (0, 0, 0)` / no fix-stage test results captured
- `2` instances: fix stage executed but solved nothing

## Unsupported Claims

- This baseline does not yet prove that `request_code_context` churn is the only
  or dominant cause of the benchmark failures.
- It does not yet prove that the next workflow-guided intervention will improve
  the cohort.
- It does not compare the structured workflow to an ad hoc diagnosis control
  arm.

## Not Checked

- I did not yet rerun `ploke-eval` on the cohort after any treatment change;
  there is no post-treatment protocol delta yet.
- I did not yet inspect the per-instance Multi-SWE-bench workdir reports beyond
  the generated summary/log surfaces.
- I did not yet test whether a smaller sub-cohort would give a cleaner signal
  than the current `5`-instance pilot.

## Risks

- Three current cohort failures terminate at the benchmark as
  "no fix-stage test results captured", which may reflect malformed or
  over-large patches rather than the exact trace-level root cause suggested by
  the protocol slice alone.
- Because the cohort is only `5` instances, a `+1` improvement is meaningful
  but still noisy.
- The first run paid one-time image-build and clone cost; follow-up treatment
  evaluation should be cheaper, but the pilot remains sensitive to Docker/harness
  stability.
