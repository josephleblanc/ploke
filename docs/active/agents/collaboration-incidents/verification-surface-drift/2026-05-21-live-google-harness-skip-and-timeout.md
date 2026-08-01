# 2026-05-21 Live Google Harness Skip And Timeout

## Trigger

The user asked for one specific live surface: a real Google API call through
`llm_manager`, `list_dir` tool execution through the `ploke-tui` command test
harness, and a final assistant response assertion after Google returned.

## User-Visible Failure

The agent added adjacent helper behavior and reported non-authoritative progress:
cheap command-harness tests, environment skip logic, and a final-response wait
that timed out with only `Elapsed(())`.

## Touched Code Surface

- `crates/ploke-tui/src/app/commands/unit_tests/mod.rs`
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
- Live Google routing through `llm_manager`
- EventBus-backed `list_dir` tool execution

## What The Agent Did

The agent treated nearby wiring checks as useful progress before proving the
exact live path the user had named. The first live harness test also waited for
the `ChatTurnFinished` placeholder assistant id instead of accounting for the
current chat-loop behavior where tool-call placeholder output and final
assistant output can be distinct assistant messages.

## Skipped Docs / Skills / Instructions

- Verification Surface Honesty in `AGENTS.md`
- The user's narrowed test acceptance surface
- The existing direct live session test body before adapting it to the harness

## Why This Was Risky

The substituted checks made it look like the new feature was closer to wired
than it was. The opaque timeout also hid the important distinction between the
live provider/tool loop succeeding and the harness assertion watching the wrong
chat-state message.

## Prevention Rule

When the user names a single live verification surface, do not add skip helpers,
cheap substitute tests, or adjacent passing surfaces as progress. Run that live
surface directly, and if it fails, include the state/event evidence needed to
distinguish provider failure, tool failure, state-manager failure, and assertion
failure.

## Follow-Up Guardrail

The live harness test now carries comments at the test and final-response wait
sites spelling out the exact surface, the no-skip expectation, the EventBus
requirement, and the placeholder-vs-final-assistant-message distinction.

## Memory Hypothesis

Prior memory and earlier test-body discussion helped identify the intended
Google/tool-loop surface, but the agent still over-scoped the implementation
instead of preserving the user's narrowed acceptance contract.
