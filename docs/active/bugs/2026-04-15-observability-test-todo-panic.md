# Active Observability Test Contains `todo!()` Fields

- date: 2026-04-15
- task title: active observability test contains `todo!()` fields
- task description: track the incomplete active `ploke-db` observability test that appears to panic if executed because required fields are still filled with `todo!()`
- related planning files: `docs/testing/BACKUP_DB_FIXTURES.md`, `docs/active/agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md`

## Summary

`crates/ploke-db/tests/observability_tests.rs` contains an active, non-ignored
test case that still constructs values using `todo!()` for `model` and
`provider_slug`.

This is not just weak coverage; it is an incomplete test body in an active test
surface.

## Evidence

- [crates/ploke-db/tests/observability_tests.rs](/home/brasides/code/ploke/crates/ploke-db/tests/observability_tests.rs:108)
  The test setup includes `todo!()` placeholders for `model` and
  `provider_slug`.
- [crates/ploke-db/tests/observability_tests.rs](/home/brasides/code/ploke/crates/ploke-db/tests/observability_tests.rs:147)
  A sibling idempotency test is explicitly ignored, which reinforces that this
  file is still an unstable test surface rather than a fully settled one.

## Risk

- If the active test runs, it will panic rather than checking a real invariant.
- This can hide whether the underlying observability path is correct, because
  the failure mode is test incompleteness rather than product behavior.

## Suggested Follow-Up

- Confirm whether the test is currently executed in normal workspace test runs.
- Replace the placeholder values with real fixture/setup inputs before treating
  the test as meaningful coverage.
- Review the rest of the file for similar placeholder or partially implemented
  cases.
