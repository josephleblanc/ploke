# Protocol Artifact Persistence Handoff

- date: 2026-04-15
- task title: protocol artifact persistence and inspection surface
- task description: persist run-local protocol artifacts for the existing `ploke-protocol` procedures and add a compact `ploke-eval` retrieval surface for listing and drilling into those artifacts
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/README.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-15_protocol-cold-start-reference.md`

## Summary

The current protocol commands now persist durable run-local artifacts instead of
only printing procedure results to stdout.

Added:

- run-local protocol artifact persistence under
  `runs/<instance>/protocol-artifacts/*.json`
- a stored artifact envelope with:
  - `procedure_name`
  - `subject_id`
  - `run_id`
  - `created_at_ms`
  - `model_id`
  - `provider_slug`
  - typed `input`
  - typed `output`
  - full nested procedure `artifact`
- a new retrieval surface:
  - `ploke-eval inspect protocol-artifacts`
  - `ploke-eval inspect protocol-artifacts <INDEX>`

## Files Changed

- [crates/ploke-eval/src/protocol_artifacts.rs](/home/brasides/code/ploke/crates/ploke-eval/src/protocol_artifacts.rs)
- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
- [crates/ploke-eval/src/spec.rs](/home/brasides/code/ploke/crates/ploke-eval/src/spec.rs)
- [crates/ploke-eval/src/layout.rs](/home/brasides/code/ploke/crates/ploke-eval/src/layout.rs)
- [crates/ploke-eval/src/lib.rs](/home/brasides/code/ploke/crates/ploke-eval/src/lib.rs)

## Verification

Completed:

- `cargo check -p ploke-eval -p ploke-protocol`
- `cargo build -p ploke-eval`
- `./target/debug/ploke-eval inspect --help`
- `./target/debug/ploke-eval protocol tool-call-review 0`
- `./target/debug/ploke-eval protocol tool-call-intent-segments`
- `./target/debug/ploke-eval inspect protocol-artifacts`
- `./target/debug/ploke-eval inspect protocol-artifacts 0`

Observed:

- persisted artifacts are listed under the run-local `protocol-artifacts/`
  directory
- the new `inspect` subcommand can list and drill into saved artifacts
- the detailed view is intentionally verbose because it shows full typed
  `input`, `output`, and nested step artifacts

## Current Usefulness

The protocol layer is now materially more useful for downstream composition
because procedure outputs are durable, inspectable, and available outside the
immediate CLI call.

The old `inspect tool-calls` workflow still remains the better raw audit
surface, but the protocol workflow now has enough persistence substrate to act
as a real derivation layer rather than a transient interpretation command.

## Next Pressure Points

The next priorities remain:

1. refine segmentation semantics
   - explicit uncertainty or ambiguity status
   - better label discipline
   - clearer distinction between weak labeling and uncovered regions
2. build one downstream procedure that composes over segmentation as an atom
3. keep the persisted artifact model step-granular and inspectable so later
   calibration and aggregation work has stable substrate

## Risks And Limitations

- `inspect protocol-artifacts <INDEX>` currently prints the full nested JSON for
  `input`, `output`, and `artifact`, which is high-signal but large
- the persistence envelope does not yet distinguish terminal metric states from
  evidential or supporting states
- no calibration or agreement metadata is recorded yet
