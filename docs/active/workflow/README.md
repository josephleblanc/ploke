# Active Workflow Documentation

`docs/active/workflow` is the live programme record: put the current charter, registry, evidence, taxonomy, active EDRs, handoffs, and other workflow artifacts here when future work should update them in place. Use `docs/workflow` for the durable workflow layer that defines reusable templates, schema drafts, framework docs, skills, and synthesis surfaces such as the `evalnomicon`.

This directory holds the living artifacts for eval-driven development.

Key live entry points:

- [readiness-status.md](readiness-status.md)
- [programme_charter.md](programme_charter.md)
- [hypothesis-registry.md](hypothesis-registry.md)
- [evidence-ledger.md](evidence-ledger.md)
- [failure-taxonomy.md](failure-taxonomy.md)
- [priority-queue.md](priority-queue.md)
- [longitudinal-metrics.md](longitudinal-metrics.md)
- [target-capability-registry.md](target-capability-registry.md)
- [edr/README.md](edr/README.md)
- [postmortems/README.md](postmortems/README.md)
- [handoffs/recent-activity.md](handoffs/recent-activity.md)
- [handoffs/README.md](handoffs/README.md)
- [phased-exec-plan.md](../plans/evals/phased-exec-plan.md)

Use [handoffs/recent-activity.md](handoffs/recent-activity.md) for the rolling current-state board and [handoffs](handoffs) for short-lived task-specific handoff notes.

Authority rule:

- use `docs/active/workflow/*` for current operational truth
- use [phased-exec-plan.md](../plans/evals/phased-exec-plan.md) as the canonical phase and exit-criteria guide
- use [eval-design.md](../plans/evals/eval-design.md) as the central design and rationale document; if it diverges from a living artifact, the living artifact wins unless explicitly noted otherwise

Metadata rule for living artifacts:

- use `owning_branch` instead of an owning team
- add `review_cadence` and `update_trigger` near the top of each living artifact

Start-here rule for active work:

- if you are about to start real work, begin here, then read [readiness-status.md](readiness-status.md), [handoffs/recent-activity.md](handoffs/recent-activity.md), and [phased-exec-plan.md](../plans/evals/phased-exec-plan.md), then the relevant handoff note if one exists, then the specific living artifact or plan your task depends on
- if the work involves target selection, benchmark scheduling, fairness interpretation, parser/modeling readiness, or deciding whether a run should happen by default, read [target-capability-registry.md](target-capability-registry.md) before choosing targets or interpreting results

Minimum record by work type:

- routine implementation work:
  keep chronology in the lab book and update handoffs or recent activity if context would otherwise be lost
- active eval execution under time pressure:
  prefer the smallest restart surface that preserves the next move; keep `CURRENT_FOCUS.md` and `handoffs/recent-activity.md` current, and defer new task-specific handoff notes unless a real decision boundary was crossed or a compaction/restart is about to happen
- formal run entered into the record:
  follow the run protocol, preserve artifacts, and update the evidence ledger
- planned A/B test, ablation, or materially diagnostic workflow change:
  create or update an EDR as well as the run record
