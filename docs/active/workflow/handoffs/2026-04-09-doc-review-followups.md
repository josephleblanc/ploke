# 2026-04-09 Doc Review Follow-Ups

- owning_branch: `refactor/tool-calls`
- source: five independent read-only doc review passes

## Resolved In This Pass

- clarified the startup path for real work
- aligned the central taxonomy with the live taxonomy on `SETUP_ENVIRONMENT`
- made the standalone phased plan canonical for phase status and exit criteria
- narrowed EDR scope to planned A/B tests, ablations, and materially diagnostic changes
- added durable locations for handoffs, postmortems, and a live priority queue
- stopped pointing raw experiment outputs at doc directories
- expanded the live hypothesis registry toward the schema promised in the design doc

## Still Intentionally Open

- exact numeric validity-guard thresholds for formal runs
- whether `A5` is a hard interpretation gate for `H0` or a strong enabling tool
- how fine-grained diagnostic hypotheses should be represented beyond the current `H0` and `A*` layer

## Suggested Next Follow-Up

1. Decide the threshold policy for formal runs and record it in the workflow.
2. Decide how fine-grained diagnostic hypotheses should appear in the registry.
3. If needed, add a small mapping note from failure categories to dependency layers for Phase 1 triage.
