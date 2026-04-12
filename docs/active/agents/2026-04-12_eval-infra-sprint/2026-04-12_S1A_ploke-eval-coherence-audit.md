# S1A - Ploke-Eval Coherence Audit

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection work is more reliable when `ploke-eval` API boundaries and test expectations are coherent and aligned with `eval-design.md`
- Design intent: Produce a compact, high-signal audit of `ploke-eval` API shape, duplication, drift, and trivially passing test risk without broadening into implementation
- Scope: Audit `crates/ploke-eval/` and its immediate eval-facing boundaries for coherence issues relevant to the active replay/inspection sprint
- Non-goals: Do not implement fixes, do not redesign the whole crate, do not touch production crates outside `ploke-eval`
- Owned files: `crates/ploke-eval/src/**`, `crates/ploke-eval/tests/**`, `docs/active/agents/phase-1-audit/AUDIT_SYNTHESIS.md`, `docs/active/plans/evals/eval-design.md`
- Dependencies: none
- Acceptance criteria:
  1. The audit identifies the highest-signal API/coherence/drift findings in `ploke-eval`.
  2. Findings distinguish between blocking primary-lane issues and non-blocking cleanup opportunities.
  3. The output calls out any test patterns that look trivially passing or insufficiently discriminating.
- Required evidence:
  - file/area inventory of what was inspected
  - concise findings with concrete file references
  - explicit note on what was not audited
  - prioritized next-action suggestions suitable for future packets
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required if work stays read-only or inside `crates/ploke-eval/`.
