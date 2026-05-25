# P0A/P0B Scope Separation Review

## implemented

- `P0A`-safe setup schema slice is present in `crates/ploke-eval/src/record.rs`: `IndexedCrateSummary`, `CrateIndexStatus`, `ParseErrorSummary`, and the added `SetupPhase` fields at [record.rs:662](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:662>) and [record.rs:693](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:693>).
- `P0B`-safe setup capture wiring is present in `crates/ploke-eval/src/runner.rs`: `build_setup_phase(...)` and `run_record.phases.setup = Some(setup_phase)` at [runner.rs:231](</home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:231>) and [runner.rs:1490](</home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1490>).
- Setup-only test coverage exists in [setup_phase_integration.rs](</home/brasides/code/ploke/crates/ploke-eval/tests/setup_phase_integration.rs:80>) and the setup sections of [record.rs](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:2119>).
- The same patch also contains replay/query surface area in `record.rs` and mixed-scope tests; those are not part of the setup acceptance slice.
- `runner.rs` also has unrelated turn-event capture plumbing; it should be judged separately from the setup wire-up.

## claims

- `P0A` can be accepted independently of replay/query behavior because the setup schema additions are self-contained and only widen `SetupPhase` serialization shape.
- `P0B` can be accepted independently of replay/query behavior because the runner change only materializes `SetupPhase` into `run_record.phases.setup`.
- `DbState`, `lookup`, `query`, `replay_query`, and their tests remain unaccepted `P0D/P0E` work and must not be bundled into `P0A/P0B` acceptance.
- `test_introspection.rs` and `introspection_integration.rs` are mixed-scope; only their setup assertions support `P0A/P0B`.

## evidence

- Setup schema evidence: [record.rs:662](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:662>), [record.rs:693](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:693>), [record.rs:2119](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:2119>), [record.rs:2229](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:2229>).
- Setup capture evidence: [runner.rs:231](</home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:231>), [runner.rs:1490](</home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1490>).
- Setup-only test evidence: [setup_phase_integration.rs:80](</home/brasides/code/ploke/crates/ploke-eval/tests/setup_phase_integration.rs:80>), setup sections of [introspection_integration.rs:56](</home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:56>) and [test_introspection.rs:18](</home/brasides/code/ploke/crates/ploke-eval/tests/test_introspection.rs:18>).
- Replay/query evidence that must stay outside `P0A/P0B`: [record.rs:795](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:795>), [record.rs:923](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:923>), [record.rs:985](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:985>), [record.rs:1089](</home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1089>), [introspection_integration.rs:136](</home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:136>), [introspection_integration.rs:227](</home/brasides/code/ploke/crates/ploke-eval/tests/introspection_integration.rs:227>), [test_introspection.rs:45](</home/brasides/code/ploke/crates/ploke-eval/tests/test_introspection.rs:45>).

## unsupported_claims

- I did not execute `cargo test`, so no runtime pass/fail claim is supported here.
- I did not inspect a freshly emitted `record.json.gz` artifact in this review.
- I did not verify live DB behavior for `lookup` or `replay_query`; those remain separate acceptance questions.

## not_checked

- Backward compatibility of older `record.json.gz` files against the new setup schema.
- Whether placeholder setup fields like `version`, `file_count`, and `tool_schema_version: None` are good enough for downstream UX.
- Whether the replay/query API shape in `record.rs` matches the eventual `P0C` design.
- Whether failure-path setup capture is correct for cached DB runs and parse failures.

## risks

- The current patch mixes setup acceptance evidence with replay/query code, so a casual review could over-accept `P0D/P0E`.
- `test_introspection.rs` proves setup population and replay/query in one file, which makes it a poor acceptance artifact unless the sections are separated mentally.
- Accepting `DbState` or `replay_query` now would implicitly bless historical-query behavior before `P0C` is settled.

## next_step

- Accept only the setup slice: `record.rs` schema additions plus setup-only serialization tests, `runner.rs` setup capture wiring, and the setup assertions in the test files.
- Leave `DbState`, `lookup`, `query`, and `replay_query` unaccepted until the `P0C` path is explicit and independently checked.
- If this patch is to be accepted cleanly, split the replay/query tests out of the setup evidence path first.
