# p1-clean-live-splice

Changed files:
- None.

Clean source worktree:
- `/home/brasides/.ploke-eval/worktrees/p1-live-splice-clean-20260515-1`
- Base commit: `8de50435`

Request:
- `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r7.json`

Command:
- `RUSTFLAGS=-Awarnings PLOKE_EVAL_HEADLESS_TUI_LIVE=1 PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH=/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r7.json CARGO_TARGET_DIR=/home/brasides/code/ploke/target cargo test -q -p ploke-eval live_broad_headless_tui_attempt_from_published_request_env -- --ignored --nocapture`

Outcome:
- Passed: 1 test passed, 0 failed.
- Executor applied proposal `185056f9-fa0c-54df-9e6a-94ce0b09f21c`.
- Base head: `8de50435dab2a230dfa417c4f18e9b255f819a02`.
- Changed path:
  - `crates/ploke-tui/src/test_harness.rs`

Artifacts:
- `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r7.json`
- `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r7.headless-tui.json`

Notes:
- This proves the clean-source admission gate passes when the request is run from a clean checkout.
- The active `/home/brasides/code/ploke` checkout still has unrelated `ploke-egui` edits and was not modified by this task.
