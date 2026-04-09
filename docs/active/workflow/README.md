# Active Workflow Documentation

`docs/active/workflow` is the live programme record: put the current charter, registry, evidence, taxonomy, active EDRs, handoffs, and lab-book entries here when future work should update them in place. Use `docs/workflow` for the durable workflow layer that defines reusable templates, schema drafts, framework docs, and skills.

This directory holds the living artifacts for eval-driven development.

Key live entry points:

- [readiness-status.md](/home/brasides/code/ploke/docs/active/workflow/readiness-status.md)
- [programme_charter.md](/home/brasides/code/ploke/docs/active/workflow/programme_charter.md)
- [hypothesis-registry.md](/home/brasides/code/ploke/docs/active/workflow/hypothesis-registry.md)
- [evidence-ledger.md](/home/brasides/code/ploke/docs/active/workflow/evidence-ledger.md)
- [failure-taxonomy.md](/home/brasides/code/ploke/docs/active/workflow/failure-taxonomy.md)
- [priority-queue.md](/home/brasides/code/ploke/docs/active/workflow/priority-queue.md)
- [edr/README.md](/home/brasides/code/ploke/docs/active/workflow/edr/README.md)
- [postmortems/README.md](/home/brasides/code/ploke/docs/active/workflow/postmortems/README.md)
- [handoffs/recent-activity.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/recent-activity.md)
- [handoffs/README.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/README.md)
- [lab-book/README.md](/home/brasides/code/ploke/docs/active/workflow/lab-book/README.md)
- [docs/active/plans/evals/phased-exec-plan.md](/home/brasides/code/ploke/docs/active/plans/evals/phased-exec-plan.md)

Use [handoffs/recent-activity.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/recent-activity.md) for the rolling current-state board and [docs/active/workflow/handoffs](/home/brasides/code/ploke/docs/active/workflow/handoffs) for short-lived task-specific handoff notes.

Authority rule:

- use `docs/active/workflow/*` for current operational truth
- use [docs/active/plans/evals/phased-exec-plan.md](/home/brasides/code/ploke/docs/active/plans/evals/phased-exec-plan.md) as the canonical phase and exit-criteria guide
- use [docs/active/plans/evals/eval-design.md](/home/brasides/code/ploke/docs/active/plans/evals/eval-design.md) as the central design and rationale document; if it diverges from a living artifact, the living artifact wins unless explicitly noted otherwise

Metadata rule for living artifacts:

- use `owning_branch` instead of an owning team
- add `review_cadence` and `update_trigger` near the top of each living artifact

Start-here rule for active work:

- if you are about to start real work, begin here, then read [readiness-status.md](/home/brasides/code/ploke/docs/active/workflow/readiness-status.md), [handoffs/recent-activity.md](/home/brasides/code/ploke/docs/active/workflow/handoffs/recent-activity.md), and [docs/active/plans/evals/phased-exec-plan.md](/home/brasides/code/ploke/docs/active/plans/evals/phased-exec-plan.md), then the relevant handoff note if one exists, then the specific living artifact or plan your task depends on

Minimum record by work type:

- routine implementation work:
  keep chronology in the lab book and update handoffs or recent activity if context would otherwise be lost
- formal run entered into the record:
  follow the run protocol, preserve artifacts, and update the evidence ledger
- planned A/B test, ablation, or materially diagnostic workflow change:
  create or update an EDR as well as the run record
