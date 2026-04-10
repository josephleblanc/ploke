# Recent Activity

- last_updated: 2026-04-09
- owning_branch: `refactor/tool-calls`
- review_cadence: update after meaningful workflow-doc changes or handoffs
- update_trigger: update after touching workflow structure, review rules, or active artifact layout

## 2026-04-09

- formalized the split between [docs/workflow](../../../workflow) and [docs/active/workflow](..)
- created durable workflow docs for manifests, experiment config, EDRs, checklists, and skills
- populated the living workflow artifacts for the programme charter, registry, evidence ledger, taxonomy, and active EDR area
- converted the lab book into an `mdbook` and added an explicit archive-boundary chapter
- added `owning_branch`, `review_cadence`, and `update_trigger` metadata to the active workflow artifacts
- ran five independent doc-review passes and folded the highest-signal issues into the workflow docs; see [2026-04-09-doc-review-followups.md](2026-04-09-doc-review-followups.md)
- **AGENTS.md** now references eval workflow documentation
- **A5** marked as hard gate for H0 interpretation in hypothesis registry
- **Diagnostic hypotheses** added to registry with `D-{DOMAIN}-{NNN}` format (Option C)
- **Cozo time travel** clarified for DB snapshot strategy — see [2026-04-09_run-manifest-design-note.md](../../agents/2026-04-09_run-manifest-design-note.md)
- **Run manifest vs run record** design converged — manifest is lightweight/differentiating, record is comprehensive with Cozo timestamps
- **Type inventory** created — complete catalog of serializable types for run record implementation — see [2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md)
- **Handoff doc** created — [2026-04-09_run-record-design-handoff.md](./2026-04-09_run-record-design-handoff.md)
- **Phase 1 tracking** created — [phase-1-runrecord-tracking.md](../../plans/evals/phase-1-runrecord-tracking.md) — implementation plan validated, ready to begin
