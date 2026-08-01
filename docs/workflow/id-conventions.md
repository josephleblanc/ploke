# Workflow ID Conventions

Use stable, human-scannable IDs across the workflow artifacts.

## IDs

- `EDR-XXXX`
  Experiment Decision Records. Four digits, zero-padded.
- `BEL-XXX`
  Evidence-ledger beliefs. Three digits, zero-padded.
- `exp-XXX-short-tag`
  Experiment IDs for configs and summaries.
- `run-YYYY-MM-DD-XXX`
  Run or manifest IDs when the harness does not already provide a canonical one.

## Rules

- Never reuse an ID for a different artifact.
- If an experiment is superseded, keep the old ID and mark the status instead of renumbering.
- If a draft becomes formal, preserve the ID and change the status or path, not the identifier.
