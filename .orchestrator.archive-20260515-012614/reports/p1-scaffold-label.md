# p1-scaffold-label

Changed files:
- crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs

Summary:
- Changed deterministic EOF candidate text to identify itself as a deterministic scaffold/no-op.
- Added explicit wording that the proposal is not semantic improvement evidence.
- Added a focused test checking the emitted proposed content.

Verification:
- `cargo test -p ploke-eval deterministic_tui_surface_producer_labels_scaffold_noop_candidates 2>&1 | tail -n 80`
- Passed: 1 passed, 0 failed.
- `cargo test -p ploke-eval tui_edit_surface_parent_selection_publishes_child_plan 2>&1 | tail -n 80`
- Passed: 1 passed, 0 failed.

Blockers:
- None.
