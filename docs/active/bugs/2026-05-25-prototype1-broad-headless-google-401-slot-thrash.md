# Prototype 1 Broad Headless TUI Retries Direct-Google 401 As Candidate Failure

## Status

Open for the live campaign until direct-Google credentials are valid again.
Source mitigation has been added so provider-unavailable terminals are typed
and stop child planning instead of cycling fresh slots.

## Summary

During the `child_plan` phase for
`p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`, direct-Google calls
failed with HTTP 401. The broad headless TUI path treated the turn as an
ordinary aborted/no-edit attempt and kept retrying fresh slots. That is the
wrong contract: provider authentication failure is an environment/provider
blocker, not evidence that a candidate failed to produce an admissible edit.

## Evidence

- Campaign:
  `p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824`
- Phase before the failing step: `child_plan`
- Diagnostic artifact:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1/messages/edit-harness-result/node-57e8487f70ce4abc.headless-tui.json`
- The diagnostic terminal was:
  `terminal=exhausted`, `attempts=4`,
  `last_feedback="Previous attempt aborted before staging an edit."`
- The same artifact's debug relay contains provider errors with
  `API error (status 401)`, `UNAUTHENTICATED`, and
  `ACCESS_TOKEN_TYPE_UNSUPPORTED`.
- `prototype1-doctor --live-protocol-preflight` for the same worktree reported
  `outcome=failed`, `route_source=direct_google`, `reasoning=disabled`, and
  `detail=provider_request: Failed while sending request to LLM provider.`

There was also an evidence-root mismatch in the first attempt:
`list_dir` on the enclosing `prototype1` directory was rejected as outside
configured roots. That is tracked separately in
`2026-05-19-rf-08-headless-tui-evidence-read-roots.md`; it was not the final
blocker here because the provider 401 repeated afterward.

## Broken Contract

The broad headless TUI runner must surface provider authentication, quota, or
availability failures as a terminal provider/environment blocker. It must not
convert them into retry prompts or slot-level no-edit failures.

## Root Cause

The provider error was persisted as a `System` message with
`MessageStatus::Error`. `tui_adapter::run_attempt` only classified provider
failures from errored `Assistant` messages. When the turn finished as aborted,
the headless runner only saw the generic aborted summary and generated retry
feedback.

## Expected Behavior

- HTTP 401/403/429 provider errors recorded on system or assistant messages
  produce `HeadlessTerminal::ProviderUnavailable`.
- `finish_broad_headless_tui_attempt` returns a blocker-shaped error for that
  terminal.
- Child planning stops rather than spending additional fresh slots on the same
  provider/environment failure.
- The persisted headless diagnostics retain enough bounded evidence to explain
  the provider class without exposing credentials.

## Disposition

Repair-and-resume only after credentials are valid and the source path no longer
misclassifies provider errors. If the live campaign admitted misleading child
evidence before the stop, abandon it and start a fresh campaign.
