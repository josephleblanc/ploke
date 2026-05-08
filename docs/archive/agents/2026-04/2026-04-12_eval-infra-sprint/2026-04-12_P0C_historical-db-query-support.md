# P0C - Historical DB Query Support

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires recorded turn timestamps to be usable for historical DB queries
- Design intent: Add a timestamp-aware DB query path that preserves existing `@ 'NOW'` behavior while enabling replay against recorded snapshots
- Scope: Implement the minimal timestamped query support required for `ploke-eval` replay and lookup work
- Non-goals: Do not broaden into parser fixes, tool redesign, or unrelated DB API cleanup
- Owned files: `crates/ploke-db/src/database.rs`, `crates/ploke-eval/src/record.rs`, targeted tests as needed
- Dependencies: none
- Acceptance criteria:
  1. There is an explicit database query path that accepts a historical timestamp without weakening current query semantics.
  2. `ploke-eval` can invoke that path with a turn timestamp from run-record data.
  3. Evidence covers both current-time and historical-time query behavior or clearly states any remaining asymmetry.
- Required evidence:
  - targeted diff summary for DB and eval call sites
  - named test command(s)
  - explicit statement of query syntax or API contract used for historical access
  - explicit note on any correctness or migration risks
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: blocked

## Permission Gate

Explicit user permission is required before implementation because this packet touches production code outside `crates/ploke-eval/`, specifically `crates/ploke-db/`.
