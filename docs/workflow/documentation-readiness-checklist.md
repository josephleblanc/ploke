# Documentation Readiness Checklist

Complete this before implementation work starts depending on the new workflow.

## Design

- [ ] Programme charter reflects the current primary hypothesis, endpoints, and decision rule.
- [ ] Hypothesis registry lists active enabling and measurement hypotheses with statuses.
- [ ] Run manifest draft covers the provenance fields needed to explain a run without guesswork.
- [ ] Experiment config draft defines how controls, arms, and validity guards are frozen.
- [ ] Failure taxonomy is stable enough that two reviewers would likely classify the same run the same way.

## Tracking

- [ ] EDR template is available and linked from `AGENTS.md`.
- [ ] Active EDR directory exists and has naming guidance.
- [ ] Evidence ledger has an update policy and at least one real entry.
- [ ] Lab book has an entry format and an initial bootstrap entry.
- [ ] Every active artifact declares an `owning_branch`, `review_cadence`, and `update_trigger`.
- [ ] Handoff and recent-activity surfaces are discoverable from the workflow READMEs.

## Skills

- [ ] `§X.A` through `§X.E` are captured as repo-local skills with concrete triggers and outputs.
- [ ] Maintenance skills exist for the evidence ledger, failure taxonomy, and lab book.
- [ ] Skills point to the active files they are meant to update.
- [ ] Skill instructions are narrow enough to reduce drift instead of encouraging prose sprawl.

## Gate

Implementation can rely on the workflow once the unchecked items above are either completed or explicitly waived in an EDR.
