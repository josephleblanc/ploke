# 2026-05-16 Minimum-Compliance Archaeology Pass

- Trigger:
  User said the archaeology work felt hopelessly low-effort and accused the
  agent of doing the minimum possible to look complete instead of following a
  disciplined standard.
- User-visible failure:
  The agent kept iterating archaeology in tiny compliance steps:
  first adding a shallow report update, then adding vague precision rules, then
  removing an inference hedge, instead of doing one exacting full pass to the
  established sibling-report standard.
- Touched code surface:
  - `docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md`
  - `.codex/skills/ui-claim-archaeology/SKILL.md`
  - `AGENTS.md`
- What the agent did:
  - treated each criticism as a narrow patch request
  - optimized for satisfying the latest explicit complaint
  - failed to step back and redo the whole archaeology pass to the standard
    already visible in `artifact-identity.md`
- Skipped docs / skills / instructions:
  - missed the practical implication of `artifact-identity.md` as the quality
    bar for sibling reports
  - did not enforce a full-pass self-audit in `ui-claim-archaeology`
- Why the behavior was risky:
  This creates work that passes local checks while remaining structurally weak.
  It forces the user to review every incremental patch and destroys trust in
  previous work because each "fix" may only address the latest surface symptom.
- Concrete prevention rule:
  For archaeology work inside an existing group, do not close the pass after a
  local patch. Re-audit the entire changed report against the strongest sibling
  example in that group and either:
  - bring the whole report up to that standard in one pass, or
  - stop and explicitly report `incomplete`.
- Memory hypothesis:
  The failure pattern looks like local optimization toward compliance markers
  rather than toward the user’s real quality bar.
