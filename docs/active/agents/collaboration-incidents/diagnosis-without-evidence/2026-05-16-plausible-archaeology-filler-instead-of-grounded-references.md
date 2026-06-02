# 2026-05-16 Plausible Archaeology Filler Instead Of Grounded References

- Trigger:
  User pointed out that the archaeology update used vague `where it appears`
  entries and asked, sarcastically but correctly, whether the skill needed to
  say things like "read the files you talk about" and "don't lie".
- User-visible failure:
  The agent produced archaeology content that looked complete on the surface
  while using imprecise carrier-location text instead of concrete type/field
  references already modeled by the better sibling report.
- Touched code surface:
  - `docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md`
  - `.codex/skills/ui-claim-archaeology/SKILL.md`
  - `AGENTS.md`
- What the agent did:
  - used loose phrases like `scheduler / node records` and `child-plan record`
  - did not force itself to confirm every cited carrier location by re-reading
    the exact type/field definitions in the current pass
  - therefore produced a report that was plausible but under-grounded
- Skipped docs / skills / instructions:
  - failed to treat `artifact-identity.md` as the local precision standard for
    `where it appears`
  - `ui-claim-archaeology` did not yet explicitly require concrete
    type/path-oriented `where it appears` entries or current-pass reads for
    cited references
- Why the behavior was risky:
  It creates archaeology that looks authoritative while quietly smuggling in
  unverified location claims. That makes later implementation and review trust
  the report more than it deserves.
- Concrete prevention rule:
  Archaeology carrier tables must:
  - include `where it appears` with concrete type/path references
  - only cite files/types/helpers actually read in the current pass
  - mark anything else `unverified` instead of filling with plausible prose
- Memory hypothesis:
  The issue was not forgetting facts. It was allowing "good enough" prose to
  stand in for grounded references because the workflow did not state the
  precision rule explicitly enough.
