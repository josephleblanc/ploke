# 2026-06-06 Broad Batch Finalization Fix

Status: implemented locally; focused regression tests passing; live run pending.

## Fixed Contract

Broad headless-TUI batch admission must persist a parent-readable batch outcome
after slot outcomes exist. A later provider/database blocker must not erase
already observed slot evidence, including submitted results that were valid but
not materialized because the batch stayed below `search.children.min`.

The fix keeps the strict candidate boundary intact:

- only `HeadlessTerminal::Applied` can write a submitted result;
- request-declared validation remains a harness responsibility after an
  applied or stop boundary;
- timed-out, validation-missing, validation-failed, turn-aborted, and
  provider-unavailable terminals are not admitted as children.

## Source Changes

Code path:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs
```

Implemented pieces:

- `BatchLedger` records completed slot indexes and explicit rejection reasons.
- `admit_broad_harness_batch` records each completed slot before admission.
- The provider/database fatal branch now stops the batch, drains running slot
  tasks, and finalizes/persists the batch outcome before returning the blocker.
- `publish_broad_harness_child_plan_from_attempts` handles both successful
  child-plan publication and failed-plan persistence from the same ledger.
- `batch_attempt_evidence` persists failed-attempt evidence for:
  - non-admitted slots;
  - fatal provider/database slots;
  - admitted submitted results that were not materialized because the batch was
    below minimum.
- Mixed child plans can now carry rejected surface-attempt evidence for
  attempted slots that did not become children.
- The headless summary fixture hook maps `provider_unavailable` terminals to
  `PrepareError::ProviderUnavailable`, so non-live tests can exercise the
  fatal slot path without provider calls.

## Regression Test

Test:

```text
provider_unavailable_after_partial_admissions_persists_failed_child_plan
```

Property:

```text
Given two valid submitted broad-harness results and a later
ProviderUnavailable slot with child_budget.min = 5, the controller must write a
failed child-plan before returning the provider blocker.
```

Assertions:

- the call returns `PrepareError::ProviderUnavailable`;
- `messages/child-plan/<parent>.json` is written;
- the persisted plan has zero children;
- rejected surface-attempt evidence contains the provider-unavailable slot;
- rejected surface-attempt evidence also contains both admitted-but-not-planned
  submitted results;
- a later child-plan resolution reuses the persisted failed plan instead of
  minting a new broad-harness batch.

## Verification

Commands run with warning noise disabled:

```bash
RUSTFLAGS=-Awarnings cargo test -p ploke-eval provider_unavailable_after_partial_admissions_persists_failed_child_plan -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval zero_admission_batch_is_persisted -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval broad_slots_run_in_parallel -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval broad_harness_batch_rejects_below_minimum_admitted_transactions -- --nocapture
```

Result: all four focused tests passed.

## Remaining Live Check

Run a fresh `prototype1-state` campaign from a committed fix with the previous
settings, except lower the child budget to:

```text
search.children.min = 2
search.children.max = 5
```

Expected result:

- if two or more slots pass validation, the batch publishes child nodes;
- if fewer than two slots pass validation, the batch still writes a failed
  child-plan with complete surface-attempt evidence;
- provider/database fatal slots do not erase already observed slot evidence.
