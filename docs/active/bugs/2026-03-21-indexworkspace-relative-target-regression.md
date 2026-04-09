# Bug Report: `IndexWorkspace` Relative Target Re-Resolution Broke `test_update_embed`

Date: 2026-03-21
Status: active
Related phase: Phase 3 `C2` in
[2026-03-20_workspaces_progress_tracker.md](../agents/2026-03-workspaces/2026-03-20_workspaces_progress_tracker.md)

## Summary

The `C2` manifest-driven indexing refactor changed
`ploke-tui::app_state::handlers::indexing::index_workspace(...)` so that a
relative `workspace` string was always passed back through generic path
resolution from process cwd.

That broke a previously implicit but real contract in `ploke-tui`: once the app
already knows the loaded crate or workspace root as an absolute path, command
handling should not silently discard that authority and reinterpret the same
target relative to the shell cwd.

In practice, this surfaced in
`app_state::database::test::test_update_embed`, where the test sets focus to:

- `/home/brasides/code/ploke/tests/fixture_crates/fixture_update_embed`

and then dispatches:

- `StateCommand::IndexWorkspace { workspace: "tests/fixture_crates/fixture_update_embed", ... }`

When the process cwd is `/home/brasides/code/ploke/crates/ploke-tui`, the
refactored handler mis-resolved that relative string to:

- `/home/brasides/code/ploke/crates/ploke-tui/tests/fixture_crates/fixture_update_embed`

which does not exist.

## Observed symptoms

- `test_update_embed` appeared to stall because it originally subscribed to the
  non-replaying indexing broadcast after sending the command.
- After the test was hardened to subscribe before send and to fail fast on
  `AppEvent::Error`, the actual runtime error became visible:
  `Failed to normalize target path ... crates/ploke-tui/tests/fixture_crates/fixture_update_embed`
- The broader issue is not in `ploke-embed`; it is in `ploke-tui` command
  orchestration and target resolution.

## Root cause

The regression came from replacing focused/app-state anchored target selection
with:

- `resolve_index_target(Some(PathBuf::from(&workspace)))`

inside
[indexing.rs](../../../crates/ploke-tui/src/app_state/handlers/indexing.rs).

That delegated relative-path interpretation to cwd-based resolution even when
`SystemStatus` already carried authoritative loaded state.

## Why this matters

This is a `G1` / `G2` class problem, not just a test nuisance:

- membership and loaded-target authority drifted away from parsed/restored app
  state
- command execution could fail or act on a different target than the loaded
  workspace/crate state implied
- the failure first manifested as an apparent hang because the test waited on a
  broadcast event that was never sent on the early-error path

## Reproduction

The narrow regression witnesses are in:

- [index_workspace_targets.rs](../../../crates/ploke-tui/tests/index_workspace_targets.rs)
- [indexing.rs](../../../crates/ploke-tui/src/app_state/handlers/indexing.rs)

Relevant tests:

- `resolve_index_target_relative_fixture_path_fails_from_ploke_tui_crate_dir`
  proves generic parser resolution fails from the crate cwd for this
  repo-relative target string
- `anchor_relative_target_matches_loaded_focus_suffix`
  proves the handler can recover the authoritative absolute path from loaded
  state before generic resolution
- `index_workspace_anchors_repo_relative_target_to_loaded_state_when_cwd_differs`
  proves the handler uses that authority instead of re-resolving from cwd

## Preventive guidance

- Do not treat process cwd as authoritative once `ploke-tui` has explicit
  loaded crate/workspace state.
- Subscribe to non-replaying broadcast channels before sending commands in
  tests that wait on those events.
- When command input is ambiguous, prefer structured target types or state-aware
  resolution over reinterpreting raw strings deep in handlers.
