# P2F - Ripgrep First Formal Phase 2 Packet

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A3` / `A4` / `H0`
- Related hypothesis: A first formal Phase 2 baseline/control packet can now be scoped fairly if it stays narrow, adopts explicit validity guards in a concrete experiment config, and records waivers for draft-only reproducibility fields
- Design intent: Turn accepted `P2E` planning into the first real formal-run packet for the ripgrep-first entry path without broadening into multi-target scheduling or silent provenance overclaims
- Scope: Author the first formal experiment-config and paired EDR surfaces for `BurntSushi__ripgrep-1294`, define the control/treatment shape, list adopted validity guards, list explicit waivers, and recommend whether execution can proceed immediately or still needs one smaller prerequisite
- Non-goals: Do not launch the live formal run yet, do not modify production code, do not broaden into non-ripgrep targets, do not silently treat waived fields as runtime-frozen
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`
- Dependencies: accepted `P2B`, accepted `P2C`, accepted `P2D`, accepted `P2E`, `docs/workflow/experiment-config.v0.draft.json`, `docs/workflow/edr/EDR_TEMPLATE.md`, `docs/active/workflow/target-capability-registry.md`, current `ploke-eval` run artifact surface
- Acceptance criteria:
  1. The packet authors one bounded formal experiment-config/EDR pair for `BurntSushi__ripgrep-1294` and ties it to the accepted ripgrep-first run policy.
  2. The packet explicitly distinguishes adopted guards from waived or deferred draft fields.
  3. The packet leaves behind one clear next action: execute, or complete one smaller prerequisite before execution.
- Required evidence:
  - direct citations to `P2B`, `P2C`, `P2D`, `P2E`, the target capability registry, and the current harness artifact/config surfaces
  - the concrete adopted guard list and waiver list
  - one clear execution recommendation after authoring the formal packet
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready
