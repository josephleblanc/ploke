# P0C0 - Query Builder And Historical Query Surface Survey

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: The safest way to add historical replay queries is to first determine whether the existing query-builder surface is salvageable enough to carry timestamped query construction without copy-pasted Cozo scripts proliferating
- Design intent: Assess the current `ploke-db` query-construction surface before committing to a `P0C` implementation path, with special attention to timestamp support, existing ergonomics, and whether the builder can become a serviceable substrate for eval replay and future LLM-facing DB tools
- Scope: Survey the existing query builder and adjacent raw-query helpers; identify how much of the current `@ 'NOW'` behavior is builder-driven versus scattered raw scripts; recommend the smallest viable path for historical query support without breaking existing callers
- Non-goals: Do not implement `P0C` in this packet, do not redesign all DB query APIs, do not require a full type-state rewrite now, do not weaken current query correctness requirements
- Owned files: `crates/ploke-db/src/query/**`, `crates/ploke-db/src/database.rs`, relevant `ploke-eval` and `ploke-tui` query call sites, sprint docs as needed
- Dependencies: none
- Acceptance criteria:
  1. The survey identifies the current query-builder surface, its actual consumers, and the main places where historical-query support would have to attach.
  2. The survey distinguishes builder limitations from broader raw-query sprawl so we do not blame the wrong layer.
  3. The survey recommends a concrete next step before `P0C`: extend the builder, add a narrower timestamped helper path, or accept a targeted fallback with explicit tradeoffs.
  4. The survey explicitly notes whether a future type-state builder direction looks incremental, awkward-but-possible, or likely to require a separate redesign packet.
- Required evidence:
  - sampled file list covering builder, DB query entrypoints, and at least a few active call sites
  - concise findings with concrete file references
  - explicit recommendation with tradeoffs and impact on `P0C/P0D/P0E`
  - note on what was not sampled
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required for survey/documentation work. Implementation that changes `crates/ploke-db/` still requires explicit approval under `P0C`.
