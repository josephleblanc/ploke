# Prototype 1 Admission Fix Changelog

Temporary root-level handoff log. Keep this terse until the historical replay
case proves the fix or disproves it.

## Current Goal

Prove that the broad headless-TUI path now produces an admissible submitted
result for a historical case that previously showed the zero-admission bug.

The historical replay should run the recorded tool-call sequence through the
current code path and reach the same admission boundary the broad harness uses:

1. model emits edit tool calls;
2. TUI tool loop applies one or more edits;
3. model/tool loop reaches its stop/end boundary;
4. harness-owned validation runs against the edited target workspace;
5. validation result decides whether a submitted result is written/admitted.

## Contract To Preserve

- The model is responsible for edit/tool calls.
- The model is not responsible for validating the target codebase.
- The harness validates after the tool loop, using the request-declared checks.
- More than one edit must be allowed before validation.
- Admission must stay strict: failed, missing, or unsupported validation is not
  an admissible successor.
- Do not make timeout-after-apply children admissible by weakening the submitted
  result gate.

## Current Correction From User

The validation check for the target codebase is not part of the model loop. It
should happen after the model returns a stop token / the tool loop finishes, then
that validation decides whether the candidate successor is admitted.

Current risk: the in-progress `tui_adapter.rs` change appears to run validation
immediately after a clean applied edit and then cancel the chat turn. That may
conflict with the requirement to allow multiple edits in the tool loop before
post-loop validation.

## Test We Need

Use a historical replay/tape case from the last run to exercise the real fixed
functions, not synthetic tool-result injection.

The test should prove:

- the historical model response enters the normal TUI tool-calling loop;
- the edit(s) are applied in the target workspace;
- the loop reaches the intended stop/end boundary;
- harness-owned validation runs after the loop;
- the resulting candidate reaches the submitted-result/admission path that was
  previously failing.

Passing only at `HeadlessTerminal::Applied` is not enough unless the test also
shows that the submitted-result/admission boundary accepts the replayed case.

## Verified Evidence: 2026-06-04

Bug report:

- `docs/active/bugs/2026-06-04-prototype1-headless-timeout-after-apply.md`
- It says the downstream submitted-result guard was intentionally strict and
  should not be relaxed.
- It says the old adapter success gate was too weak because it accepted latest
  model-run cargo success rather than request-declared validation.
- It says `p1-memfix-0604a` failed with zero admitted children: 8
  `applied_timed_out`, 1 `applied_validation_missing`, 1 `timed_out`.

Most recent worktree by directory mtime/birth time:

- `/home/brasides/.ploke-eval/worktrees/p1-memfix-0604a`
- `stat` birth time: `2026-06-04 12:36:47.247106726 -0700`
- mtime: `2026-06-04 12:37:04.347273072 -0700`

Useful historical case:

- request:
  `/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a/prototype1/messages/edit-harness-request/node-a212c1db6c2774de-r10.json`
- headless result:
  `/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a/prototype1/messages/edit-harness-result/node-a212c1db6c2774de-r10.headless-tui.json`
- trace:
  `/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a/prototype1/messages/edit-harness-result/node-a212c1db6c2774de-r10.turn-live/agent-turn-trace.json`

r10 trace facts:

- event 83: `non_semantic_patch` stages two files;
- event 85: those two patches apply;
- event 86/87: model runs `cargo test -p ploke-selection-score`, which fails;
- event 88: model requests another `non_semantic_patch`;
- event 90: second patch applies;
- event 91/92: model runs `cargo test -p ploke-selection-score`, which passes;
- event 93/94: model runs `cargo check -p ploke-selection-score`, which passes;
- event 95: `TurnFinished` with `outcome = "completed"`.

r10 request-declared validation facts:

- `cargo check -p ploke-eval`
- `cargo test -p ploke-eval edit_surface`

r10 failure fact:

- The model produced edits and reached a completed turn, but the result was
  `applied_validation_missing` because the requested validation commands were
  absent.
- No submitted result file was written for r10.
- Child plan rejected r10 with:
  `missing requested validation after applying proposal ... cargo check -p
  ploke-eval, cargo test -p ploke-eval edit_surface`.

Loop boundaries:

- Outer adapter loop: `run_attempt` in
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`.
  It observes `ToolCallRequested`, `ToolCallCompleted`, staged/applied batches,
  and `ChatTurnFinished`.
- Inner model/tool loop: `run_chat_session` in
  `crates/ploke-tui/src/llm/manager/session.rs`. It may execute multiple
  provider/tool-call steps before returning `SessionOutcome::Completed`.
- Tool dispatch loop: `handle_event` in
  `crates/ploke-tui/src/llm/manager/mod.rs` dispatches
  `SystemEvent::ToolCallRequested` to `tools::process_tool`.
- Test runtime harness: `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
  provides the spawned app/state/event/LLM actor harness and relays state
  commands for assertions.

Correct property to test:

- Given the historical r10 replay reaches `ChatTurnFinished outcome=completed`
  after applied edits, the fixed broad headless-TUI path must then run
  request-declared validation as harness work and produce an admissible
  submitted result if those validations pass.
- The validation must happen after the inner model/tool loop reaches the stop
  boundary, not immediately after the first clean edit batch.

## Active Suspicions

- The original failure was not stale persisted files being checked.
- The submitted-result guard was correctly refusing non-admissible terminals.
- The bug is in how/when the adapter produces the terminal evidence needed for
  admission after edits and request-declared validation.
- Historical replay is the repro mechanism for the admission fix, not a separate
  feature test.

## Touched So Far

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `docs/active/bugs/2026-06-04-prototype1-headless-timeout-after-apply.md`

Other dirty files were already present or from adjacent work and should not be
treated as part of this handoff unless inspected directly.

## 2026-06-04 Update

Changed
`historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop`
to test the admission property directly:

- resolves historical r10 through `agent-turn-trace.json` event 95, where the
  turn reached `ChatTurnFinished outcome=completed`;
- replays the ending r10 tool-call tape through the broad headless-TUI request
  path, not a synthetic submitted-result reader;
- preserves the historical ordering sensitivity: the first r10 patch introduces
  `+ nth`, the repair patch changes it to `+ *nth`, so request-declared
  `ploke-eval` validation only passes if validation waits until the completed
  stop boundary;
- asserts the request declares exactly `cargo check -p ploke-eval` and
  `cargo test -p ploke-eval edit_surface`;
- asserts a submitted result is written and `verify_request` accepts it;
- asserts diagnostics record terminal `Applied`, the completed turn event, and
  successful `declared_validation_1_0` / `declared_validation_1_1`.

Focused verification:

- `cargo test -p ploke-eval historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop -- --nocapture`
- result: passed, `1 passed; 0 failed`.

Broader adjacent verification:

- `cargo test -p ploke-eval edit_surface -- --nocapture`
- result: passed, `129 passed; 0 failed; 10 ignored`.

Remaining useful follow-up:

- Run any repository-wide checks required before committing. The focused
  historical replay and the adjacent `edit_surface` filter are green.
