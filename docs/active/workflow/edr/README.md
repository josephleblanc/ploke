# Active EDRs

Store active Experiment Decision Records here.

- owning_branch: `refactor/tool-calls`
- review_cadence: review alongside daily experiment planning at 3:00 p.m. America/Los_Angeles local time
- update_trigger: update before implementation for planned formal experiments and again after results are available
- id_conventions: [id-conventions.md](../../../workflow/id-conventions.md)

## Naming

Use `EDR-XXXX-short-title.md`.

## Status Index

### Active

- [EDR-0001-ripgrep-1294-phase2-entry.md](EDR-0001-ripgrep-1294-phase2-entry.md)

### Complete

- [EDR-0002-cli-trace-review-skill-experiment.md](EDR-0002-cli-trace-review-skill-experiment.md)

### Superseded

- none yet

## Process

1. Start from [EDR_TEMPLATE.md](../../../workflow/edr/EDR_TEMPLATE.md).
2. Create the EDR before implementation when the work is a planned A/B test, ablation, or other materially diagnostic eval change.
3. Update the same file after the run with manifest IDs, outcome, and decision.

Routine implementation work does not require an EDR by default; use the lab book, handoffs, and evidence ledger unless the work is materially diagnostic.

## Archive Rule

- Keep completed EDRs in place and move them from `Active` to `Complete`.
- If an EDR is replaced by a better-scoped or corrected successor, keep the original file and list it under `Superseded` with the replacement ID instead of deleting it.
