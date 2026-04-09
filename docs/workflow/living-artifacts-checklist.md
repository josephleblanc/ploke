# Six Living Artifacts Checklist

Use this checklist to keep `§XI` concrete and small.

## Required Artifacts

- [x] Programme Charter
  Active file: [docs/active/workflow/programme_charter.md](/home/brasides/code/ploke/docs/active/workflow/programme_charter.md)
- [x] Hypothesis Registry
  Active file: [docs/active/workflow/hypothesis-registry.md](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md)
- [x] Experiment Decision Records
  Durable template/example: [docs/workflow/edr](/home/brasides/code/ploke/docs/workflow/edr)
  Active records: [docs/active/workflow/edr](/home/brasides/code/ploke/docs/active/workflow/edr)
- [x] Evidence Ledger
  Active file: [docs/active/workflow/evidence-ledger.md](/home/brasides/code/ploke/docs/active/workflow/evidence-ledger.md)
  Maintenance skill: [docs/workflow/skills/evidence-ledger/SKILL.md](/home/brasides/code/ploke/docs/workflow/skills/evidence-ledger/SKILL.md)
- [x] Failure Taxonomy
  Active file: [docs/active/workflow/failure-taxonomy.md](/home/brasides/code/ploke/docs/active/workflow/failure-taxonomy.md)
  Maintenance skill: [docs/workflow/skills/failure-taxonomy/SKILL.md](/home/brasides/code/ploke/docs/workflow/skills/failure-taxonomy/SKILL.md)
- [x] Lab Book
  Active directory: [docs/active/workflow/lab-book](/home/brasides/code/ploke/docs/active/workflow/lab-book)
  Maintenance skill: [docs/workflow/skills/lab-book/SKILL.md](/home/brasides/code/ploke/docs/workflow/skills/lab-book/SKILL.md)
  Book summary: [docs/active/workflow/lab-book/src/SUMMARY.md](/home/brasides/code/ploke/docs/active/workflow/lab-book/src/SUMMARY.md)

## Tightness Rules

- Keep artifact scopes distinct. Do not let the evidence ledger become the lab book or the failure taxonomy become a postmortem archive.
- Prefer a new entry in an existing artifact over a new top-level artifact.
- Add a new failure category only when repeated cases do not fit the current taxonomy.
- Use `owning_branch` metadata on living artifacts to indicate update responsibility.
