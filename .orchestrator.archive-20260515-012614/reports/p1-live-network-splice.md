# p1-live-network-splice

Changed files:
- None.

Request path:
- `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r9.json`

Credential preflight:
- `OPENROUTER_API_KEY` was present in the process environment. The key was not printed.

Command:
- `PLOKE_EVAL_HEADLESS_TUI_LIVE=1 PLOKE_EVAL_BROAD_HARNESS_REQUEST_PATH=/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-request/node-e8202a069a00d7f5-r9.json cargo test -q -p ploke-eval live_broad_headless_tui_attempt_from_published_request_env -- --ignored --nocapture 2>&1 | tail -n 180`

Outcome:
- The live headless-TUI path ran and wrote result artifacts:
  - `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r9.json`
  - `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/messages/edit-harness-result/node-e8202a069a00d7f5-r9.headless-tui.json`
- Diagnostics recorded terminal `applied` with one changed workspace path:
  - `/home/brasides/.ploke-eval/campaigns/p1-sparse-context-broad-tui-20260514-4/prototype1/workspaces/edit-harness/node-e8202a069a00d7f5-r9/crates/ploke-tui/src/test_harness.rs`
- Final validation rejected the attempt because the source repository path in the request is `.` and this checkout is dirty from the current patch wave plus an unrelated untracked script.

Blocker:
- A fully admitted live splice needs a clean source checkout for requests whose `source_repository_path` is `.`.
