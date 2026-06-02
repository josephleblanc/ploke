Not tested: this docs-only follow-up used code inspection only.

Change summary:

- Added `../20260518-nine-phase-selection-run-questions.md`, a source-grounded
  question-and-answer note for the nine-phase selection run findings.
- Updated `../README.md` so the profiling note is discoverable.

Verification surface:

- Code inspection only.
- No allocation tests were run.
- No allocation logging files, heap artifacts, or benchmark `report.json` files
  were inspected for this pass.

Baseline comparison:

- No baseline comparison was valid because this was a docs-only analysis note,
  not a benchmark run.

Allocation churn summary:

- No measured scenario was run.
- Callsite attribution was not captured.

Measured improvements:

- None measured.

Measured regressions:

- None measured.

Remaining allocation debt:

- The open questions still require measured confirmation. The code inspection
  points to Patch Debug diff construction/layout, Run Records row volume, and
  app-lifetime render caches as likely next inspection targets.

Unmeasured risk and next action:

- The best guesses may be wrong where egui internals or allocator behavior
  dominate. A focused callsite-attributed pass should verify Patch Debug, Run
  Records, and one edge section before implementation work is prioritized.
