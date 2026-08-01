# S1A Report Back

**implemented**
- Audit only; no code changes were made.

**claims**
- `RunRecordBuilder::add_turn_from_artifact` is still a placeholder that drops captured turn data, so the emitted run record cannot faithfully replay real turns. See `crates/ploke-eval/src/record.rs:1346-1393`, `crates/ploke-eval/src/runner.rs:1514-1523`, and `crates/ploke-eval/src/runner.rs:2427-2460`.
- `replay_state_at_turn` does not reconstruct conversation history; `conversation_up_to_turn` is always empty. See `crates/ploke-eval/src/record.rs:408-437`.
- `DbState::lookup` and `replay_query` are leaky relative to `eval-design.md`: lookup is a fixed relation whitelist, and replay still expects callers to write raw Cozo with `@ 'NOW'` instead of hiding historical access. See `crates/ploke-eval/src/record.rs:958-1054` and `crates/ploke-eval/tests/introspection_integration.rs:266-336`.
- Test coverage is duplicated and weak: `setup_phase_integration.rs` reimplements production setup/lookup logic, and the introspection tests print on `Err`/`None` instead of failing, so missing behavior can still pass. See `crates/ploke-eval/tests/setup_phase_integration.rs:80-133`, `crates/ploke-eval/tests/setup_phase_integration.rs:177-220`, `crates/ploke-eval/tests/setup_phase_integration.rs:225-474`, `crates/ploke-eval/tests/introspection_integration.rs:160-180`, `crates/ploke-eval/tests/introspection_integration.rs:202-220`, `crates/ploke-eval/tests/introspection_integration.rs:272-302`, `crates/ploke-eval/tests/introspection_integration.rs:316-336`, and `crates/ploke-eval/tests/test_introspection.rs:7-67`.

**evidence**
- Inspected `crates/ploke-eval/src/lib.rs`, `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/src/runner.rs`, `crates/ploke-eval/src/run_history.rs`, `crates/ploke-eval/src/tests/replay.rs`, `crates/ploke-eval/src/tests/llm_deserialization.rs`, `crates/ploke-eval/tests/test_introspection.rs`, `crates/ploke-eval/tests/introspection_integration.rs`, and `crates/ploke-eval/tests/setup_phase_integration.rs`.
- `record.rs` advertises a broad replay/introspection API, but the builder still writes empty `tool_calls`, `None` responses, and `TurnOutcome::Content`; `replay_state_at_turn` also leaves conversation reconstruction unimplemented.
- `runner.rs` captures richer events at turn time, but passes them through `add_turn_from_artifact` without filling the record fields.
- `setup_phase_integration.rs` uses local helper copies instead of the shipped API, which makes it easy for the test logic to drift from production behavior.

**unsupported_claims**
- The claim that Phase 1 replay/introspection is complete is not supported by this code snapshot.
- The claim that the current tests validate replay/lookup behavior is not supported; several paths only log failures instead of asserting them.

**not_checked**
- I did not do a full line-by-line audit of `crates/ploke-eval/src/{main.rs,cli.rs,layout.rs,registry.rs,model_registry.rs,msb.rs,provider_prefs.rs,spec.rs}` beyond surface references.
- I did not run `cargo test` or re-open the live `~/.ploke-eval` artifacts.
- I did not inspect `ploke_db`, `ploke_tui`, or `ploke_llm` internals beyond call-site context.

**risks**
- Broken replay can be masked by tests that accept `Err` or `None`.
- Duplicated setup logic in tests can drift from production and report false confidence.
- The record schema can look complete while still omitting the actual turn payload needed for replay and attribution.

**next_step**
- Open a follow-up packet to wire `RunRecordBuilder` to persist real turn fields and replace the print-on-error introspection tests with fail-fast assertions against the production API.
