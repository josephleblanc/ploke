---
name: failure-taxonomy
description: Use when classifying runs or refining the failure taxonomy so categories stay stable, meaningful, and resistant to subtype sprawl.
---

# Failure Taxonomy

Update [docs/active/workflow/failure-taxonomy.md](/home/brasides/code/ploke/docs/active/workflow/failure-taxonomy.md).

## Rules

- Choose one primary category and at most two secondary categories.
- Prefer existing categories over inventing a new one.
- Add a new top-level category only after repeated failures cannot be represented cleanly with notes or examples.
- Put recurring detail in canonical examples or postmortem notes, not in new category names.
