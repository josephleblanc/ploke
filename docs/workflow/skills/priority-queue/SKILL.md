---
name: priority-queue
description: Use when ranking eval work so lower-layer blockers, evidence value, cost, and coupling are applied consistently instead of by intuition.
---

# Priority Queue

Use this skill when choosing what to do next across multiple eval tasks.

## Ranking Order

1. Layer violation:
   fix lower-layer blockers before higher-layer optimization.
2. Evidence value:
   prefer work that will clearly confirm or refute something important.
3. Cost:
   prefer the cheaper item when evidence value is similar.
4. Coupling:
   prefer decoupled work over bundled changes.

## Output

Produce a short ranked list with one sentence per item explaining why it landed there.
