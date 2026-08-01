# Protocol Selection And Stepping

## Readout

`protocol run` is already a single-step executor. In `crates/ploke-eval/src/cli.rs`, `ProtocolRunCommand::run` resolves one `record_path`, computes `protocol_state_for_run`, executes only `before.next_step`, then recomputes state. So a future `--single-step` flag would be a compatibility alias, not a new behavior.

If the default changes to “all steps,” the command needs a loop over the resolved frontier, not just `next_step`. `--range 0..4` would need to mean an ordered slice of the protocol frontier derived from current aggregate state. It cannot safely mean “artifact indices” unless the code first defines a stable step-plan abstraction.

## Artifact Identity Risk

Current artifact lookup is tied to the resolved `record_path`, but not to the artifact’s own identity fields. `list_protocol_artifacts()` and `load_latest_segmented_sequence()` only scope by `record_path.parent()/protocol-artifacts`, and `load_protocol_aggregate_from_artifacts()` trusts whatever is there. `StoredProtocolArtifact.run_id` and `subject_id` are persisted, but not validated against the resolved run.

So the likely failure mode is not “global artifact lookup ignored the run,” but “the wrong run/instance was resolved, and then run-local artifacts were trusted without content checks.”

## Missing Guards / Tests

- Validate `StoredProtocolArtifact.run_id` against the resolved run dir and `subject_id` against `record.metadata.benchmark.instance_id`.
- Add a regression test that mixed-run or mixed-instance artifacts under the same lookup path are rejected or skipped.
- Add a test around `ProtocolRunCommand::run` / `resolve_record_path_from_eval_home()` proving explicit `--record` or `--instance` resolution is the only input to protocol artifact selection.
- Add a test for `load_latest_segmented_sequence()` to confirm it never crosses run boundaries and that “latest” is only within the resolved run dir.

## Next Files To Inspect

- `crates/ploke-eval/src/cli.rs`
  - `ProtocolRunCommand::run`
  - `resolve_record_path_from_eval_home`
  - `protocol_state_for_run`
  - `execute_protocol_intent_segments_quiet`
  - `execute_protocol_tool_call_segment_review_quiet`
  - `load_latest_segmented_sequence`
- `crates/ploke-eval/src/protocol_artifacts.rs`
  - `StoredProtocolArtifact`
  - `write_protocol_artifact`
  - `list_protocol_artifacts`
  - `load_protocol_artifact`
- `crates/ploke-eval/src/protocol_aggregate.rs`
  - `load_protocol_aggregate_from_artifacts`
  - `normalize_anchor`
  - `normalize_call_review`
  - `normalize_segment_review`
- `crates/ploke-eval/src/run_registry.rs`
  - `sync_protocol_registration_status`
- `crates/ploke-eval/src/selection.rs`
  - `load_active_selection_at`
  - `render_selection_warnings`
