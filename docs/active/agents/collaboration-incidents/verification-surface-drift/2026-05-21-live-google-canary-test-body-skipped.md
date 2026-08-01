# 2026-05-21 Live Google Canary Test Body Skipped

## Trigger

The user said, "it sounds like you didn't actually read the test," after the agent
reported the result of `live_google_chat_session_executes_list_dir_tool_call_success_or_quota`.

## User-visible failure

The agent first ran the wrong Cargo invocation with `--ignored`, then reran the
test correctly and summarized it as proving the live `ploke-tui` session-loop
canary path before reading the test body. The summary was directionally related
but not grounded in the actual construction and assertion chain.

## Touched code surface

- `crates/ploke-tui/src/llm/manager/session.rs`
- `docs/active/plans/self-improvement-loop/google-api.md`

## What the agent did

The agent used the test name, feature gate, and pass/fail output as enough
evidence to describe the verification surface. It had not yet checked that the
test constructs `ChatSession<Google>` directly, uses `ChatStepSource::live()`,
dispatches `ToolCallRequested` through `process_tool`, and asserts one requested
tool, one completed tool, two chat steps, no errors, and `SessionOutcome::Completed`.

## Skipped docs / skills / instructions

- Skipped the practical part of Verification Surface Honesty: read the test body
  and assertions before saying what a passed test proves.
- Skipped the repo preference to answer from exact file-backed surfaces when the
  user is concerned about guessing.

## Why this was risky

The Google routing work is specifically about which surface is proven: direct
`ploke-llm`, direct `ChatSession<Google>`, normal `ploke-tui` runtime routing,
native interactive TUI, or `ploke-eval` headless adapter. Treating the test name
and pass output as sufficient evidence could blur those boundaries and make a
future Google/Vertex/OpenRouter route decision look more verified than it is.

## Prevention rule

Before reporting what any targeted test proves, read the test body or the helper
it delegates to and name the concrete assertion chain. If the test name suggests
a broader surface than the assertions cover, report the narrower assertion-backed
surface first.
