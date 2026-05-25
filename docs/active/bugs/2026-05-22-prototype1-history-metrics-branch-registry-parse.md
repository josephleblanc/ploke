# Prototype 1 History Metrics Branch Registry Parse Failure

- date: 2026-05-22 local / 2026-05-23 UTC
- status: open

## Summary

The read-only Prototype 1 metrics projection can fail before producing metrics for an otherwise highly relevant live campaign. This blocks the feedback loop we need for long-running self-improvement validation, because `history metrics` is one of the main ways to confirm whether the loop is stable and whether selected descendants improve over generations.

## Observed Failure

Command, run from `/home/brasides/code/ploke` after rebuilding `ploke-eval`:

```text
./target/debug/ploke-eval history metrics \
  --campaign p1-google-live-run-20260521-4 \
  --format json --rows 20
```

Observed error:

```text
database setup failed during 'build prototype1 metrics projection': typed evidence boundary error: failed to parse Prototype1BranchRegistry for class BranchRegistry at '/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-4/prototype1/branches.json'
```

The referenced file is JSONL with one branch record per line, for example:

```text
{"schema_version":"prototype1-branch-record.v1","recorded_at":"2026-05-22T17:51:08.434793724+00:00","body":{"kind":"parent_comparison",...}}
```

## Why This Matters

The long-running loop stabilization plan depends on being able to ask read-only questions after every run:

- which nodes completed, failed, or were selected;
- whether applied patches and valid submissions increased;
- whether failed tool calls, repair loops, missing submissions, and aborts decreased;
- whether selected successors are improving on operational/protocol/oracle metrics.

If one parse incompatibility in `branches.json` prevents the whole metrics projection, the operator loses a key progress signal exactly when diagnosing a failed or partially successful live run.

## Initial Hypothesis

This may be a schema/back-compat boundary: the campaign contains `prototype1-branch-record.v1` JSONL branch records, while the current metrics projection path appears to expect a different `Prototype1BranchRegistry` shape or fails hard instead of degrading/skipping unknown branch-registry evidence.

Do not assume this is the same root cause as the successor History sealed-block failure. It is a feedback/observability blocker found while inspecting that campaign.

## Expected Behavior

One of these should hold:

1. `history metrics` parses the JSONL branch-record format and includes the branch comparison records in the projection; or
2. if a historical branch-registry schema is unsupported, `history metrics` emits a bounded diagnostic and still produces whatever node/evaluation/history rows it can derive from other evidence.

It should not fail the whole metrics projection without a partial output artifact or actionable degraded-state report.

## Fix Direction

1. Add a focused regression fixture using the `p1-google-live-run-20260521-4` `branches.json` JSONL shape.
2. Confirm whether the parser should accept `prototype1-branch-record.v1` JSONL directly or route it through a compatibility/degraded-evidence adapter.
3. Make metrics projection fail soft for unsupported read-only branch evidence unless the missing branch parse would make a specific requested view unsound.
4. Re-run:

```text
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval <new-regression-test> -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval --lib
./target/debug/ploke-eval history metrics --campaign p1-google-live-run-20260521-4 --format json --rows 20
```

## Related Evidence

The same campaign remains important because it got past the earlier Google/headless-TUI problems and failed later during successor startup:

```text
docs/active/bugs/2026-05-22-prototype1-successor-history-sealed-block-verification.md
```
