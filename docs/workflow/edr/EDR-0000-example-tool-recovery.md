# EDR-0000: Follow-Up Lookup Recovery

- status: complete
- date: 2026-04-09
- owners: eval workflow bootstrap
- hypothesis ids: A1
- related issues/prs: none
- linked manifest ids: run-2026-04-09-example-control, run-2026-04-09-example-treatment
- linked artifacts:
  - [run-manifest.v0.draft.json](../run-manifest.v0.draft.json)
  - [experiment-config.v0.draft.json](../experiment-config.v0.draft.json)

## Decision

Test whether a recovery-oriented lookup miss response outperforms a hard failure on a fixed failure cohort.

## Why Now

Recent eval postmortems show multiple runs ending without a useful patch after lookup misses, while the rest of the run state remained healthy enough to continue.

## Control And Treatment

- control:
  hard failure on `code_item_lookup` miss
- treatment:
  return up to three follow-up candidates with an explicit recovery hint
- frozen variables:
  same model, provider, prompt, budget, subset, and retry policy

## Acceptance Criteria

- primary:
  productive recovery rate improves by at least 15%
- secondary:
  no more than 5% wall-clock or token regression
- validity guards:
  provider failure rate and setup failure rate both remain under 5%

## Plan

1. Replay the known miss cohort.
2. If replay looks promising, run the paired live subset.
3. Classify failures and update the evidence ledger.

## Result

- outcome:
  example only; no real run executed
- key metrics:
  placeholder
- failure breakdown:
  placeholder
- surprises:
  placeholder

## Decision And Follow-Up

- adopt / reject / inconclusive:
  inconclusive
- next action:
  replace placeholders with real results before using this pattern as precedent
