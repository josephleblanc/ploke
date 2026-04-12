# P0F - Turn Record Fidelity And Replay-State Reconstruction

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: A5 replay/introspection readiness requires emitted run records to persist actual turn data rather than placeholder shells
- Design intent: Make `RunRecord` persist the real turn payload needed for trustworthy replay and inspection, and remove tests that pass when that payload is missing
- Scope: Fix `RunRecordBuilder::add_turn_from_artifact`, tighten replay-state reconstruction for persisted turn data, and replace weak print-on-error tests with fail-fast assertions inside `crates/ploke-eval/`
- Non-goals: Do not change production crates outside `ploke-eval`, do not implement new `ploke-db` historical-query primitives, do not redesign the full replay API surface
- Owned files: `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/src/runner.rs`, `crates/ploke-eval/tests/**` as needed
- Dependencies: none, but must respect the `P0C` permission gate and avoid requiring `crates/ploke-db` edits
- Acceptance criteria:
  1. `add_turn_from_artifact` persists the real turn fields it already has enough information to capture, instead of placeholder `None`/empty/default values.
  2. `replay_state_at_turn` no longer hardcodes an empty conversation when the required data is present in the record.
  3. Tests covering replay/lookup behavior fail when the expected behavior is missing; they do not pass by logging `Err` or `None`.
  4. Evidence distinguishes behavior proven entirely inside `ploke-eval` from anything still blocked on `P0C`.
- Required evidence:
  - targeted diff summary for `record.rs`, `runner.rs`, and changed tests
  - named test command(s)
  - explicit note on what replay behavior remains blocked pending `P0C`
  - artifact or assertion evidence that persisted turn data is no longer placeholder-only
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional user permission required if work stays inside `crates/ploke-eval/`.
