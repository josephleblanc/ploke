# S3A - Workflow Adherence Audit Report

- implemented: Reviewed the packet, orchestration protocol, `docs/active/workflow/README.md`, `docs/active/workflow/readiness-status.md`, `docs/active/workflow/handoffs/recent-activity.md`, `AGENTS.md`, the active control-plane doc, `docs/workflow/handoff-template.md`, two recent handoffs, and the workflow skills index plus `micro-sprint-eval-loop`.

- claims:
  - The biggest agent failure mode is still over-claiming in prose, because the packet/report path is not yet rigid enough to force claim-to-evidence-to-criterion mapping.
  - Verifier passes are better bounded than before, but the protocol still leaves enough room for a mini-investigation unless the verifier budget is made more explicit.
  - Orchestrator context load is reduced by the current-state table in the control plane, but there is still no reusable template for that doc, so each sprint can drift in structure.
  - The workflow docs are mostly aligned now, but the operational surfaces still rely on several separate entry points, which makes recovery and handoff discipline fragile.
  - The skills layer does not yet provide a dedicated orchestration/reporting skill, so agents must infer how to apply the protocol from several partial skills.

- evidence:
  - `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-12_S3A_workflow-adherence-audit.md` defines the packet scope and required report shape.
  - `docs/active/agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md` requires evidence-backed claims, claim-to-criterion mapping, and a bounded verifier budget.
  - `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md` adds the current-state table and packet links, but no reusable template.
  - `docs/active/workflow/README.md` still frames startup around workflow docs and handoffs, not a control-plane template.
  - `docs/workflow/handoff-template.md` is minimal and does not force claims, evidence, unsupported claims, or verifier budget.
  - `docs/active/workflow/handoffs/2026-04-09_run-record-design-handoff.md` and `docs/active/workflow/handoffs/2026-04-10_conversation-capture-design.md` are narrative handoffs, not structured claim/evidence reports.
  - `docs/workflow/skills/README.md` lists operational skills, but none is explicitly an orchestration-control-plane or report-adherence skill.

- unsupported_claims:
  - I did not verify the actual packet docs `P0A`-`P0E` or `S1A`-`S3A` beyond the control-plane index.
  - I did not inspect any handoffs beyond the two sampled notes.
  - I did not audit whether any runtime or chat wrapper enforces these docs automatically.
  - I did not read the full `docs/active/workflow/edr/` or lab-book surfaces.

- not_checked:
  - Whether the new control-plane doc is the only active sprint surface in practice.
  - Whether the packet/report templates are already being used consistently by all current agents.
  - Whether a narrower verifier budget would be sufficient for all Layer 0-1 work.
  - Whether a short skill update would be enough instead of a new dedicated orchestration skill.

- risks:
  - Workers can still over-promise if they write a plausible summary without an explicit criterion link.
  - Verifiers can still spend too long if they treat the bounded budget as advisory instead of mandatory.
  - Orchestrator recovery can become expensive again if control-plane docs and handoffs diverge in structure.
  - Agents can keep defaulting to old narrative handoff habits because the template does not force the newer report schema.

- next_step:
  1. Add one reusable control-plane template or checklist with a mandatory current-state table, packet links, and report links.
  2. Update `docs/workflow/handoff-template.md` and the packet/report templates to require numbered acceptance criteria, `unsupported_claims`, and a hard evidence budget.
  3. Add a short workflow skill or skill note that tells agents when to use the orchestration protocol and which living artifacts to update after each packet disposition.
