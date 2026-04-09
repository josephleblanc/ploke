---
name: postmortem-protocol
description: Use when a run is surprising or unclear and the team needs a consistent failure walk-through, primary classification, and follow-up decision.
---

# Postmortem Protocol

Use this skill for surprising runs and ambiguous failures.

## Walkthrough

1. Identify the failing case and stable artifact paths.
2. Check whether setup and data state were valid.
3. Ask whether the DB or index answered correctly.
4. Ask whether the tool contract and recovery path were adequate.
5. Ask whether the model then reasoned correctly.
6. Ask whether the harness graded and recorded the result correctly.
7. Assign one primary failure category and optional secondary categories.

## Output

- primary category
- confidence
- smallest credible follow-up action

Prefer the earliest blocking cause over downstream noise.
