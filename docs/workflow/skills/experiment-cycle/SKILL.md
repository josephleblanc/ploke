---
name: experiment-cycle
description: Use when planning or recording a concrete eval change through the Observe, Orient, Hypothesize, Implement, Measure, Record, Decide loop so the work stays tied to a hypothesis, an EDR, and an explicit expected outcome.
---

# Experiment Cycle

Use this skill when a task is an eval experiment rather than a generic code change.

## Required Outputs

- hypothesis ID
- one-sentence prediction
- linked EDR path or reason no EDR is needed
- expected metric movement
- next decision after measurement

## Loop

1. Observe the anomaly or failure cohort.
2. Orient it against the hypothesis registry.
3. Write a testable prediction.
4. Implement the narrowest meaningful change.
5. Measure on the right cohort or subset.
6. Record the result in the EDR, ledger, or lab book.
7. Decide whether to adopt, reject, or continue.

## Guardrails

- Prefer one meaningful variable change per experiment.
- If measurement quality is still in doubt, escalate to a lower-layer task instead of pretending the experiment is interpretable.
