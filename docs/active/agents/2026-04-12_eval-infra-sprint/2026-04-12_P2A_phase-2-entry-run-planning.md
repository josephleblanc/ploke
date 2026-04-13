# P2A - Phase 2 Entry Run Planning

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A2` / `A3` / `H0`
- Related hypothesis: The accepted Phase 1 replay/inspection substrate and the new target capability registry are sufficient to support a bounded decision about the first Phase 2 baseline/control run-planning slice
- Design intent: Turn the now-accepted Phase 1 substrate and active run-policy artifacts into a concrete, fairness-aware next-step packet for Phase 2 rather than leaving phase advancement as an implicit chat decision
- Scope: Review the phased plan, hypothesis/priority/evidence workflow artifacts, and target capability registry to propose the first bounded Phase 2 run-planning slice, including candidate targets or subsets, explicit run-policy constraints, and any remaining blockers that must be cleared before a formal baseline batch
- Non-goals: Do not launch live benchmark runs, do not modify production code, do not redesign the target capability registry schema, do not silently adopt new validity-guard thresholds without documenting the basis
- Owned files: `docs/active/workflow/**`, `docs/active/plans/evals/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`
- Dependencies: accepted `P0A`-`P0F`, accepted `S2B`, integrated `target-capability-registry.md`, current `priority-queue.md`, `hypothesis-registry.md`, and `evidence-ledger.md`
- Acceptance criteria:
  1. The packet produces a bounded recommendation for the first Phase 2 run-planning slice, with explicit target or subset candidates and linked run-policy annotations from the target capability registry.
  2. The packet identifies the concrete blockers, if any, that still prevent a fair formal baseline/control batch from starting, with specific reference to the current `A2`, `A3`, and manifest/validity workflow state.
  3. The packet leaves behind one clear next packet recommendation, such as `A2` validation, validity-guard policy adoption, manifest convergence, or baseline-run execution planning.
- Required evidence:
  - explicit review of `phased-exec-plan.md`, `hypothesis-registry.md`, `priority-queue.md`, `evidence-ledger.md`, and `target-capability-registry.md`
  - candidate target/subset list with per-target run-policy notes
  - blocker table or equivalent summary tying the recommendation to current workflow state
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional permission required for this planning packet as long as the work stays in documentation, workflow artifacts, and read-only inspection of existing run metadata or code references.
