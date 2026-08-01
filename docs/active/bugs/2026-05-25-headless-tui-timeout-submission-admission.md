# Headless TUI Timeout Submits Invalid Child Result

## Status

Fixed in source for future attempts. The existing campaign
`p1-gemini35-flash-direct-fresh-20260525-035030` still contains the invalid r3
artifact described below; continuing that campaign is an explicit operator
override, not clean loop evidence.

## Broken Contract

A Prototype 1 broad headless-TUI child attempt must not publish a submitted
result for materialization when the terminal state is `timed_out` or the latest
recorded cargo validation failed.

## Evidence

- Campaign: `p1-gemini35-flash-direct-fresh-20260525-035030`
- Child slot: `node-dfbca03c896b03ae-r3`
- Workspace:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030/prototype1/workspaces/edit-harness/node-dfbca03c896b03ae-r3`
- Commit: `303eb599`
- Headless diagnostics:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030/prototype1/messages/edit-harness-result/node-dfbca03c896b03ae-r3.headless-tui.json`
- Submitted result:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030/prototype1/messages/edit-harness-result/node-dfbca03c896b03ae-r3.json`

The diagnostics artifact reports:

```json
{
  "terminal": { "terminal": "timed_out", "secs": 900 },
  "last_validation": {
    "display_command": "cargo check",
    "ok": false,
    "status_reason": "compile_failed",
    "exit_code": 101,
    "errors": 2
  }
}
```

The sibling submitted result still records changed files for admission:

- `crates/ploke-db/src/get_by_id/mod.rs`
- `crates/ploke-db/src/helpers.rs`

The live step then reported doctor phase `materialize` with no blockers, so the
next transition could treat this invalid child artifact as materialization
input.

## Suspected Source

`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::finish_broad_headless_tui_attempt`
currently handles `HeadlessTerminal::TimedOut` by publishing a submitted result
when `run.applied_edit()` exists.

Existing test coverage encodes that old behavior:

`timed_out_headless_tui_applied_attempt_writes_submitted_result_for_admission`

## Disposition

Repair completed for future attempts. The current campaign has persisted
invalid child transition evidence, so review notes should treat it as tainted
materialization input even if the operator advances it for diagnostic purposes.

## Required Fix

- Timed-out headless-TUI attempts must not write `submitted_result_path`, even
  when an edit was applied before timeout.
- The old regression test should be replaced with one asserting timed-out
  applied attempts are rejected before submitted-result publication.
- A focused `ploke-eval` test should prove the invariant before any fresh
  campaign is advanced past child planning.

## Verification

Implemented in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs::finish_broad_headless_tui_attempt`.

Focused tests:

```text
RUSTFLAGS=-Awarnings cargo test -p ploke-eval timed_out_headless_tui -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval validation_audit -- --nocapture
```

Both passed on 2026-05-25.
