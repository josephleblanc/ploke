# Rust Style Review: TUI Adapter Implementation Wave

## Findings

### High: retry prompts can overlap the still-running turn and consume unrelated events

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:86`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:131`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:151`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:242`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:279`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:304`

`run_headless` retries by immediately calling `harness.add_user_msg` after a tool failure, no-edit turn, empty proposal, or rejected proposal. It does not wait for the prior `ChatTurnFinished`, does not cancel the prior request, and does not filter subsequent `ToolCallCompleted` / `ToolCallFailed` / `ChatTurnFinished` events by the active request id. That means retry attempt N+1 can run while attempt N is still emitting events, and any proposal found in `state.proposals` may be approved or denied under the wrong attempt counter.

This is a correctness issue in the async/event boundary, not just noisy accounting. The adapter needs an explicit per-turn state machine keyed by request/session identifiers, or it needs to wait for a terminal event for the current turn before sending retry feedback.

### High: existing broad harness worktrees are destructively reset without branch/path safety checks

- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1628`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1641`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1643`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1648`

`prepare_broad_harness_workspace` treats any git-recognized worktree at the requested workspace path as reusable, then runs `git reset --hard HEAD` and `git clean -fd`. Unlike `ensure_reusable` for child workspaces, this path does not verify the expected branch/ref, does not check that the worktree is actually bound to the request identity, and does not inspect dirty paths before deleting them.

Given this adapter is driven from persisted request paths, this can silently discard local candidate evidence from a previous failed/in-flight attempt or a mismatched worktree that happens to occupy the same path. Reuse should first validate the managed identity and either reject dirty/mismatched state or refresh through an explicit, typed retry disposition.

### Medium: command-send failures are ignored, causing timeout-shaped failures and stale state races

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:77`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:140`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:161`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:166`

The adapter ignores failures from `SetEditingAutoConfirm`, `DenyEdits`, and `ApproveEdits`. If the state manager task has exited or the command channel is closed, the apply path drops into the status polling loop and waits until the outer timeout instead of returning the real channel failure. On the deny path, the code may retry even though the rejected proposal was never marked denied.

These sends should be awaited and mapped into an adapter error or terminal failure. The apply wait should also be event-driven or bounded by an apply-specific timeout so a lost command cannot consume the whole turn budget.

### Medium: path precheck is weaker than the backend admission boundary and is not tested directly

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:113`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:137`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:334`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:2558`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:2628`

`classify_paths` is the live pre-approval guard, but it only checks `..` and the `crates/ploke-eval` prefix after a best-effort `strip_prefix`. The stronger normal-repo-relpath validation lives later in backend admission, after the TUI proposal has already been approved and applied. There are no direct tests around `run_headless`/`classify_paths` behavior for absolute paths, `.` components, duplicate mixed absolute/relative paths, non-UTF-8 paths, or command-channel failure.

The later backend admission is useful, but pre-approval should share the same normal-relpath validation vocabulary or delegate to a common path classifier so the adapter does not apply a patch that it already had enough information to reject.

## Open Questions

- Is overlapping retry intended to be supported by `ploke-tui` chat sessions? If yes, the adapter still needs request-id filtering before it can safely attribute proposals and terminal events.
- Should broad harness workspace refresh ever be destructive, or should stale/dirty workspaces be surfaced as retryable evidence for the parent loop?

## Verification

- Ran `cargo check -p ploke-eval --tests`; it completed successfully with warnings only.

## Residual Risk

I did not run the live API tests. The current unit coverage exercises the typed surface boundary and backend admission, but I did not find focused tests for the new `run_headless` event loop, retry ordering, command-channel failure handling, or pre-approval path classifier.
