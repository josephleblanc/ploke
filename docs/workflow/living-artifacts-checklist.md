# Six Living Artifacts Checklist

Use this checklist to keep `§XI` concrete and small.

## Required Artifacts

- [x] Programme Charter
  Active file: [programme_charter.md](../active/workflow/programme_charter.md)
- [x] Hypothesis Registry
  Active file: [hypothesis-registry.md](../active/workflow/hypothesis-registry.md)
- [x] Experiment Decision Records
  Durable template/example: [edr](edr)
  Active records: [edr](../active/workflow/edr)
- [x] Evidence Ledger
  Active file: [evidence-ledger.md](../active/workflow/evidence-ledger.md)
  Maintenance skill: [evidence-ledger/SKILL.md](skills/evidence-ledger/SKILL.md)
- [x] Failure Taxonomy
  Active file: [failure-taxonomy.md](../active/workflow/failure-taxonomy.md)
  Maintenance skill: [failure-taxonomy/SKILL.md](skills/failure-taxonomy/SKILL.md)
- [x] Evalnomicon
  Durable directory: [evalnomicon](evalnomicon)
  Book summary: [SUMMARY.md](evalnomicon/src/SUMMARY.md)

## Tightness Rules

- Keep artifact scopes distinct. Do not let the evidence ledger become the evalnomicon or the failure taxonomy become a postmortem archive.
- Prefer a new entry in an existing artifact over a new top-level artifact.
- Add a new failure category only when repeated cases do not fit the current taxonomy.
- Use `owning_branch` metadata on living artifacts to indicate update responsibility.
