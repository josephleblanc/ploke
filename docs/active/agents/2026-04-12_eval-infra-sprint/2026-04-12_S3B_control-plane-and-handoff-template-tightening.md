# S3B - Control-Plane And Handoff Template Tightening

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A4
- Related hypothesis: Agents are less likely to drift if the reusable templates force claim/evidence discipline and a uniform sprint-control shape
- Design intent: Convert the accepted S3A findings into concrete template-level guardrails rather than relying on narrative compliance
- Scope: Add or update the reusable control-plane and handoff/report templates so they require current-state tables, acceptance-criterion linkage, `unsupported_claims`, and bounded evidence framing
- Non-goals: Do not rewrite all historical handoffs, do not create a full new skill unless clearly needed, do not broaden into non-eval process docs
- Owned files: `docs/workflow/handoff-template.md`, `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-templates.md`, and nearby template docs as needed
- Dependencies: `S3A` report
- Acceptance criteria:
  1. The reusable templates now encode the structured claim/evidence protocol rather than leaving it optional.
  2. There is a reusable control-plane shape or checklist that future sprints can copy.
  3. The changes are small enough to be realistically followed by agents in practice.
- Required evidence:
  - targeted diff summary
  - explicit template paths changed
  - note on what was intentionally left out to avoid template sprawl
  - recommended next step if a skill update is still warranted
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required for doc-only work.
