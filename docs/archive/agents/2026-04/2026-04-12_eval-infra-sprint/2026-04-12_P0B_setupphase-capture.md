# P0B - SetupPhase Capture Wiring

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires setup artifacts to be populated in `RunRecord`, not only written as sidecar files
- Design intent: Populate `run_record.phases.setup` during real eval setup so a run artifact can answer basic setup questions without external file hunting
- Scope: Wire `SetupPhase` construction and assignment in `crates/ploke-eval/src/runner.rs` and any directly related tests
- Non-goals: Do not redesign artifact emission, do not implement historical DB queries, do not change production crates outside `ploke-eval`
- Owned files: `crates/ploke-eval/src/runner.rs`, `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/tests/` as needed
- Dependencies: `P0A`
- Acceptance criteria:
  1. Successful runs populate `run_record.phases.setup` instead of leaving it `null`.
  2. The populated setup phase includes indexed-crate data from the accepted `P0A` schema.
  3. Evidence includes at least one targeted test and one inspected emitted record artifact or equivalent artifact-level assertion.
- Required evidence:
  - targeted diff summary for `runner.rs` and any test files
  - named test command(s)
  - artifact evidence showing `phases.setup` is non-null and shaped as expected
  - explicit note on whether failure-path setup capture was checked
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required if work stays inside `crates/ploke-eval/`.
