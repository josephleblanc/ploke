# P0A/P0B Initial Verification Note

- date: 2026-04-12
- author: orchestrator
- scope: current in-worktree `ploke-eval` changes relevant to `P0A` and `P0B`

## implemented

- Reviewed the current diffs in `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/src/runner.rs`, and `crates/ploke-eval/tests/**`.
- Verified that the in-flight patch adds `SetupPhase.indexed_crates` and related setup-schema types in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:632).
- Verified that the in-flight patch populates `run_record.phases.setup` during runner setup in [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1486).
- Collected a prior independent verification summary from a worker test pass:
  - `cargo test -p ploke-eval setup_phase -- --nocapture`
  - `cargo test -p ploke-eval test_introspection -- --nocapture`

## claims

- `P0A` appears implemented in the current worktree:
  - `SetupPhase` now has an explicit serialized `indexed_crates` field and related summary types.
  - Unit tests in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1926) exercise serialization and round-trip of the new schema shape.
- `P0B` appears implemented in the current worktree:
  - successful runs assign `run_record.phases.setup = Some(setup_phase)` in [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1496)
  - artifact-backed tests expect non-null setup data in [test_introspection.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/test_introspection.rs:17) and [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:57)
- The current patch exceeds `P0A/P0B` scope:
  - it also adds replay/query surfaces and DB snapshot helpers in [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:438), [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:767), and corresponding tests under `crates/ploke-eval/tests/`

## evidence

- Schema evidence:
  - [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:666) defines `SetupPhase`
  - [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:632) defines `IndexedCrateSummary`
  - [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1926) tests serialization/round-trip
- Capture wiring evidence:
  - [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:228) defines `build_setup_phase(...)`
  - [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1486) assigns `run_record.phases.setup`
- Artifact-level evidence:
  - [test_introspection.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/test_introspection.rs:17) expects `SetupPhase` to be populated from an emitted run record
  - [introspection_integration.rs](/home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:57) asserts indexed crates from the existing ripgrep run artifact
- Independent worker test evidence previously reported:
  - `cargo test -p ploke-eval setup_phase -- --nocapture` exited `0`
  - `cargo test -p ploke-eval test_introspection -- --nocapture` exited `0`

## unsupported_claims

- This note does not prove full `cargo test -p ploke-eval` success.
- This note does not prove failure-path setup capture behavior.
- This note does not accept the replay/query additions that were bundled into the same worktree patch.
- This note does not verify backward compatibility of old `record.json.gz` artifacts beyond serde defaults on the new fields.

## not_checked

- Whether cached-DB setup capture semantics are correct beyond the current tests.
- Whether `tool_schema_version` should be populated now or deferred.
- Whether the `version` and `file_count` placeholders in `IndexedCrateSummary` are sufficient for the intended inspection UX.
- Whether any fixture regeneration or compatibility work is needed for older run records.

## risks

- The current patch mixes `P0A/P0B` with unaccepted replay/query work, which makes packet acceptance ambiguous.
- Some replay/query tests are weak or non-failing-on-error, so a green test subset can overstate progress outside setup capture.
- `test_introspection.rs` currently logs replay-query parser errors without failing, so it should not be treated as evidence that replay surfaces are ready.

## next_step

1. Treat `P0A` and `P0B` as provisionally independently checked, but do not accept the broader replay/query additions under the same patch.
2. Keep `P0C` blocked pending explicit permission for `crates/ploke-db/`.
3. Create or adopt a follow-up packet to separate/clean up unaccepted `P0D/P0E` work and strengthen weak replay tests.
