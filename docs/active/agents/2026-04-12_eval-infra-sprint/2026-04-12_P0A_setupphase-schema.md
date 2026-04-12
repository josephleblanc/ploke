# P0A - SetupPhase Schema Extension

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires setup data to be queryable from the run record itself
- Design intent: Extend the serialized setup schema just enough to capture which crates were indexed, without weakening schema clarity or conflating setup capture with replay/query behavior
- Scope: Add the minimal `SetupPhase` fields and supporting types needed for indexed-crate capture inside `crates/ploke-eval/`
- Non-goals: Do not populate the field at runtime, do not add historical query support, do not broaden into manifest redesign
- Owned files: `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/tests/` as needed
- Dependencies: none
- Acceptance criteria:
  1. `SetupPhase` includes an explicit serialized field for indexed-crate summaries.
  2. The new schema shape round-trips through the existing run-record serialization path.
  3. The change does not weaken existing invariants or silently tolerate missing setup data.
- Required evidence:
  - targeted diff summary for `record.rs`
  - named test command(s) covering serialization shape or round-trip
  - explicit note on whether backward-compat or fixture impact was checked
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required if work stays inside `crates/ploke-eval/`.
