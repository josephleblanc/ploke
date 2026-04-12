# P0D - Turn DB State Lookup

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires a turn-bound DB snapshot API that can answer symbol-existence questions programmatically
- Design intent: Implement the minimum `turn.db_state().lookup(name)` surface from `eval-design.md` without overcommitting to a broader query abstraction too early
- Scope: Add a turn-level DB-state wrapper and minimal lookup method inside `crates/ploke-eval/`
- Non-goals: Do not add arbitrary replay queries in this packet, do not redesign run-record storage, do not touch production crates unless separately approved
- Owned files: `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/tests/` as needed
- Dependencies: `P0C`
- Acceptance criteria:
  1. A turn-level API exposes a DB-state wrapper or equivalent snapshot handle.
  2. `lookup(name)` answers whether a requested item exists at the turn snapshot.
  3. Evidence demonstrates present and absent lookup behavior against a recorded turn timestamp.
- Required evidence:
  - targeted diff summary for record/introspection code
  - named test command(s)
  - explicit note on what `lookup(name)` matches and what it intentionally does not match yet
  - explicit note on error behavior for missing timestamps or missing setup data
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: proposed

## Permission Gate

No additional permission if implementation stays inside `crates/ploke-eval/`, but this packet depends on historical query support from `P0C`.
