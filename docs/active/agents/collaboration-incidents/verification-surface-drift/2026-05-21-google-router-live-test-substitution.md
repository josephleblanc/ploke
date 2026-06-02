# 2026-05-21 Google Router Live Test Substitution

## Trigger

The user explicitly asked for live Google API tests in `ploke-eval`, then had to
clarify: "WRITE LIVE API TESTS. YOU HAVE PERMISSION TO RUN LIVE API TESTS."

## User-Visible Failure

The agent reported the passing
`broad_tui_attempt_google_provider_selects_google_router` selector test too close
to the requested live Google `Router` verification surface. That test only
asserts model-selection wiring and does not make a live API call.

## Touched Code Surface

- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- `crates/ploke-eval/src/runner.rs`
- `ploke-eval` Google direct-router and headless-TUI live test surfaces

## What The Agent Did

The agent did name that the selector test was non-live, but did not immediately
add and run live `ploke-eval` API tests when the user's earlier request was for
live verification.

## Skipped Instructions

- User request: add live API tests for Google routing in `ploke-eval`.
- `AGENTS.md` verification-surface rule: do not let one surface stand in for
  another.
- `AGENTS.md` narrowed-surface rule: when the user narrows acceptance to one live
  surface, run that surface directly.

## Why This Was Risky

Google direct auth and endpoint behavior can fail only at the live HTTP boundary.
A selector-only test can pass while `ploke-eval` still builds the wrong request,
uses OpenRouter-specific provider data, or fails Google auth at execution time.

## Prevention Rule

When the user asks for live provider verification, add or run a test that crosses
the provider HTTP boundary from the named crate. Report selector tests only as
selector tests, never as progress toward live API confirmation.

## Memory Hypothesis

The repeated failure mode is verification-surface drift around live Google tests:
the agent overweights nearby green tests and underweights the exact live path the
user named.
