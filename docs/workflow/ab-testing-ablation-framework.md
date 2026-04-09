# A/B Testing And Ablation Framework

This file is the durable operating contract for `§IX`.

## Purpose

Run controlled comparisons by changing configuration, not code paths. The experiment description should be precise enough that another person can reproduce the comparison from the config, manifest, and linked EDR.

## Rules

1. Freeze all control variables before the run:
   - benchmark subset
   - model and provider policy
   - prompt version
   - runtime budgets and retry policy
2. Change one substantive variable per experiment unless the EDR explicitly documents coupling.
3. Pair runs by benchmark instance whenever possible.
4. Predeclare acceptance criteria and validity guards in the EDR before implementation.
5. Abort the comparison if setup, provider, or telemetry failures exceed the allowed guardrails.

## Supported Designs

- `ab_test`
  Compare one control arm against one or more treatment arms.
- `ablation`
  Remove or disable one component at a time from an otherwise fixed treatment.
- `cohort_replay`
  Replay a known failure cohort against a narrow change before paying for a wider live run.

## Required Outputs

Every formal comparison should produce:

- one committed experiment config
- one EDR in [edr](../active/workflow/edr)
- one run manifest per executed arm/run
- an evidence-ledger update summarizing what changed in belief

## Current Accuracy Note

The repo does not yet have a single converged manifest or fully implemented comparison runner. The draft config in [experiment-config.v0.draft.json](experiment-config.v0.draft.json) is the target contract to converge on, not a claim that the harness already supports every field.
