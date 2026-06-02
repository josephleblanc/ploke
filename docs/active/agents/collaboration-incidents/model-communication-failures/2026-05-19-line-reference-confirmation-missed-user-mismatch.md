# 2026-05-19 Line Reference Confirmation Missed User Mismatch

## Trigger

The user showed a snippet of the newly added RF-05 regression test without the
`regr:` marker and asked whether it was the test marked in the tracker. After
the agent responded with a file/line confirmation, the user reported that the
line reference landed on a different neighboring replay test.

## User-Visible Failure

The answer confirmed the marker/tracker relationship but did not address the
actual mismatch the user was surfacing: their displayed snippet lacked the
marker, and the line navigation they used did not point at the same code the
agent was citing.

## Touched Surface

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `docs/active/agents/expected-failing-regression-tests.md`

## What The Agent Did

- Checked the current file and found the marker above the RF-05 test.
- Answered with line links as though current local line numbers alone resolved
  the user's concern.
- Failed to say that the user's snippet appeared to be from a stale or
  different view of the file and failed to provide a nearby function-line
  cross-check showing both the RF-05 test and the adjacent gated replay test.

## Skipped Or Underweighted Instructions

- Verification surface honesty: the response named a local line but did not
  distinguish current local `nl`/`rg` output from the user's editor/navigation
  surface.
- Collaboration incident handling: the user was pointing at a trust-breaking
  mismatch, and the first response should have treated that as the issue rather
  than just restating the current file content.

## Why This Was Risky

Regression tracker markers are used to decide whether a known-red test is
intentional. If the agent cannot clearly identify which test owns the marker,
the team can end up ignoring the wrong failing test, leaving the actual replay
regression untracked or accidentally excluding a green fixed-contract test.

## Prevention Rule

When the user reports that a file/line reference does not match what they see,
do not answer with only another line link. Provide a three-point cross-check:

1. Current `rg -n` output for marker and function names.
2. Current `nl -ba` excerpt including the marker, attributes, and function
   signature.
3. The neighboring symbol line numbers that explain possible stale-line or
   shifted-link confusion.

Then explicitly say whether the user's snippet is stale, incomplete, or points
at a different symbol.

## Memory Hypothesis

Prior instruction and memory emphasized concise verification statements. That
was misapplied here: the user needed reconciliation of two conflicting views,
not a compact restatement of the local file result.
