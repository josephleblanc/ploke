# P2C - Validity-Guard Policy

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A3` / `H0`
- Related hypothesis: Formal baseline/control runs are not interpretable unless the current validity-guard thresholds for provider/setup/runtime failures are explicit rather than draft
- Design intent: Turn the currently acknowledged validity-guard ambiguity into one bounded policy decision before any formal Phase 2 baseline/control packet is allowed to proceed
- Scope: Review the live workflow artifacts that mention validity guards, identify the current draft thresholds or threshold gaps, and leave behind one explicit recommendation for how formal runs should be gated right now
- Non-goals: Do not launch new eval runs, do not modify production code, do not silently adopt a broad experiment-config redesign, do not reinterpret `A2` readiness beyond accepted `P2B`
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`
- Dependencies: accepted `P2B`, `readiness-status.md`, `hypothesis-registry.md`, `priority-queue.md`, `eval-design.md`, `docs/workflow/experiment-config.v0.draft.json`
- Acceptance criteria:
  1. The packet identifies which validity guards are already draft-defined versus missing for formal Phase 2 runs.
  2. The packet states one explicit recommendation for the current operational policy: adopt thresholds now, require an EDR first, or keep formal runs blocked pending manifest/config convergence.
  3. The packet updates or recommends updates to the live workflow artifacts without overstating what has been formally adopted.
- Required evidence:
  - direct citations to the workflow/readiness/design/config artifacts that currently define or defer validity guards
  - an explicit statement about whether numeric thresholds are already operationally binding
  - one clear next-packet recommendation after the policy decision
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready
