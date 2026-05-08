# P2D - Manifest And Config Convergence

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: `A3` / `A4` / `H0`
- Related hypothesis: Formal baseline/control runs remain hard to interpret if validity-guard adoption and provenance freezing depend on multiple draft artifacts that do not yet map cleanly onto the real harness output path
- Design intent: Turn the accepted `P2C` policy into one bounded convergence step so the first formal Phase 2 packet can point at a concrete manifest/config surface instead of parallel drafts
- Scope: Compare the draft run-manifest and experiment-config schemas against current harness output artifacts, identify the minimum convergence needed for a first formal baseline/control packet, and leave behind one explicit recommendation for the formal-run entry surface
- Non-goals: Do not launch new eval runs, do not modify production code, do not silently declare the full long-term manifest design complete, do not broaden into target-selection work beyond what is needed to define the formal-run entry surface
- Owned files: `docs/active/workflow/**`, `docs/workflow/**`, `docs/active/agents/2026-04-12_eval-infra-sprint/**`
- Dependencies: accepted `P2C`, `docs/workflow/run-manifest.v0.draft.json`, `docs/workflow/experiment-config.v0.draft.json`, `docs/active/workflow/evidence-ledger.md`, `docs/active/plans/evals/eval-design.md`, current `ploke-eval` run artifact layout
- Acceptance criteria:
  1. The packet identifies which manifest/config fields already map cleanly onto current harness artifacts versus which remain draft-only or split across files.
  2. The packet recommends one bounded formal-run entry surface for the next Phase 2 packet, including where validity guards should be adopted and where provenance should be read from.
  3. The packet updates or recommends updates to the live workflow artifacts so the next formal-run packet does not need to rediscover the manifest/config blocker.
- Required evidence:
  - direct citations to the draft manifest/config files and the current harness artifact layout
  - an explicit statement about what a first formal-run packet should freeze versus what can remain draft
  - one clear next-packet recommendation after the convergence decision
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready
