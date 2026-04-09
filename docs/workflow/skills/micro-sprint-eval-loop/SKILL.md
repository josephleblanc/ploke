---
name: micro-sprint-eval-loop
description: Use when working a small daily eval loop on a narrow failure cohort so the team can diagnose, replay, widen, and log changes without drifting into ad hoc iteration.
---

# Micro-Sprint Eval Loop

Use this skill for the daily operational unit.

## Steps

1. Select a tight failure cohort, usually 5 to 10 related failures.
2. State the diagnostic hypothesis.
3. Open or update the EDR if the change is materially diagnostic.
4. Implement the change.
5. Replay the cohort first.
6. Run a wider subset only after replay is promising.
7. Update the EDR, evidence ledger, and lab book.

## Keep Tight

- Cohorts should share an error pattern.
- Do not widen to a benchmark slice until the narrow replay says the change is directionally sound.
