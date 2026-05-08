# P2E - Phase 2 Formal Entry Planning

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A3` / `A4` / `H0`
- Related hypothesis: The programme can start a fair first formal Phase 2 baseline/control packet once one bounded experiment-config/EDR pair adopts explicit validity guards and cites a concrete provenance surface the harness actually emits
- Design intent: Convert accepted `P2D` convergence guidance into the first real formal execution-planning packet without pretending the long-term manifest/config story is already complete
- Scope: Choose the first bounded formal Phase 2 candidate slice, define the concrete experiment-config and EDR surfaces it should use, state the adopted validity guards and waivers, and leave behind one clear execution-planning recommendation
- Non-goals: Do not launch the formal batch yet, do not modify production code, do not broaden into a full benchmark schedule, do not silently treat draft-only provenance fields as runtime-frozen
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`
- Dependencies: accepted `P2B`, accepted `P2C`, accepted `P2D`, `docs/workflow/experiment-config.v0.draft.json`, `docs/workflow/edr/EDR_TEMPLATE.md`, `docs/active/workflow/target-capability-registry.md`, current `ploke-eval` run artifact surface
- Acceptance criteria:
  1. The packet identifies one bounded formal Phase 2 candidate slice and ties it to the accepted target/run-policy state.
  2. The packet specifies the exact experiment-config/EDR adoption surface for validity guards and any explicit waivers for draft-only fields.
  3. The packet leaves behind one clear next packet recommendation: execution, narrower prerequisite, or scoped instrumentation follow-up.
- Required evidence:
  - direct citations to `P2B`, `P2C`, `P2D`, the target capability registry, and the current artifact/config surfaces
  - an explicit list of adopted guards versus waived or deferred draft fields
  - one clear next-packet recommendation after formal-entry planning
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready
