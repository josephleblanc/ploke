# P2G - Runner Arm Surface

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A3` / `A4` / `H0`
- Related hypothesis: The first formal Phase 2 packet is executable once `ploke-eval` can express the intended control and treatment arms through an explicit runner surface instead of hardcoded benchmark policy only
- Design intent: Turn the authored `P2F` config/EDR pair into something the runner can execute honestly without relying on undocumented prompt-only conventions
- Scope: Inspect and, if appropriate, implement the minimal `crates/ploke-eval/` surface needed to express the first formal control/treatment arms for `BurntSushi__ripgrep-1294`, then leave behind one execution recommendation
- Non-goals: Do not broaden into multi-target scheduling, do not redesign the whole long-term experiment runner, do not weaken provenance or validity requirements, do not change production code outside `crates/ploke-eval/` without explicit permission
- Owned files: `crates/ploke-eval/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`, `docs/active/workflow/**`
- Dependencies: accepted `P2F`, [EDR-0001](../../workflow/edr/EDR-0001-ripgrep-1294-phase2-entry.md), [exp-001 config](./2026-04-12_exp-001-ripgrep-1294-phase2-entry.config.json), current `ploke-eval` runner/CLI surfaces
- Acceptance criteria:
  1. The packet identifies and, if feasible within `crates/ploke-eval/`, implements the minimal runner surface needed to distinguish the planned control and treatment arms honestly.
  2. The packet provides targeted evidence for whether the authored `P2F` packet is now executable or still needs one smaller prerequisite.
  3. The packet keeps scope narrow to the first formal ripgrep packet rather than designing the full long-term experiment platform.
- Required evidence:
  - direct citations to the current runner/CLI surface and the authored `P2F` config/EDR artifacts
  - targeted test or verification evidence if code changes are made
  - one clear next action: execute, dry-run verify, or one smaller follow-up
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready
