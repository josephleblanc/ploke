# S2D - Metrics Backfill And Regeneration Prototype

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: H0
- Related hypothesis: The longitudinal metrics layer only becomes operational when a tiny real backfill/regeneration loop proves the proposed JSONL companion and markdown ledger flow against actual formal runs
- Design intent: Turn the accepted `S2C` design into a minimal prototype or proof path over a small real run sample
- Scope: Prototype the smallest practical backfill/regeneration path for the longitudinal metrics layer using a small sample of formal runs and the storage/update model proposed in `S2C`
- Non-goals: Do not build a full CI/CD pipeline, do not redesign run-manifest or run-record schema, do not broaden into unrelated workflow automation
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, implementation helpers narrowly related to the prototype if needed, related sprint docs
- Dependencies: accepted `S2B`, accepted `S2C`
- Acceptance criteria:
  1. The packet validates or narrows the proposed JSONL-companion plus regenerated-markdown flow against a small real sample.
  2. The output makes clear what part of the flow is now proven versus still hypothetical.
  3. The packet leaves behind a bounded next step for operationalizing the prototype.
- Required evidence:
  - sampled formal-run input set
  - concrete output artifact or prototype summary
  - explicit note on what still remains manual or blocked
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional permission required for doc/workflow-local prototype work that stays inside the repo and eval artifact roots.
