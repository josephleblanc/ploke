# Self-Improvement Evaluation Underread Existing Metrics

Date: 2026-05-16

## Trigger

The user stated that the prior self-improvement architecture evaluation was
founded on incomplete codebase evaluation because the repo already had several
of the data types and metrics the response described as future work.

## User-Visible Failure

The response treated MBE/evaluation/metric carriers too generically and framed
some already-implemented typed records as missing, so the strategic critique was
not grounded enough for the user's current implementation state.

## Touched Surface

- `crates/ploke-eval/src/operational_metrics.rs`
- `crates/ploke-eval/src/branch_evaluation.rs`
- `crates/ploke-eval/src/mbe/mod.rs`
- `crates/ploke-records/src/evaluation.rs`
- `crates/ploke-records/src/selection.rs`
- `crates/ploke-records/src/history/payload.rs`
- Prototype 1 self-improvement planning and History/selection evidence review.

## What The Agent Did

The agent read some controlling docs and code paths but stopped before
enumerating the existing typed evaluation, selection, oracle, and metric
records. It then gave an architectural answer whose broad direction was useful
but whose implementation-gap claims were overbroad.

## Skipped Docs / Skills / Instructions

- Did not fully follow the typed-persistence-spine audit habit before making
  claims about missing persisted carriers.
- Did not sufficiently inspect the existing `ploke-records` passive DTOs before
  proposing new evidence objects.
- Did not distinguish "carrier exists" from "carrier is used as policy/admission
  authority" clearly enough.

## Why This Was Risky

Prototype 1 already has a dense typed record surface. Treating existing carriers
as absent can push the next slice toward duplicate types, semantic drift, and
unnecessary architecture churn instead of connecting current records into the
selection, History, and validator paths.

## Prevention Rule

Before giving a strategic implementation-gap answer about Prototype 1 metrics or
persistence, first produce a grounded inventory of existing Rust carriers and
then classify each gap as one of: missing type, missing persisted field, missing
writer, missing reader, missing History admission, missing selector authority,
or missing validator/consensus role.

## Memory Hypothesis

The memory rule that digest and History correctness are non-negotiable was
useful but insufficient. Future memory should also bias toward checking current
typed carriers before naming new ones.
