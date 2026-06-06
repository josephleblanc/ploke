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

## 2026-06-04 Post-Commit Handoff

Committed current work:

- commit: `666477bf`
- subject: `Fix headless TUI admission replay`
- final status before this handoff update: clean worktree

Important verification already run:

- `cargo test -p ploke-eval historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop -- --nocapture`
- result: passed, `1 passed; 0 failed`
- `cargo test -p ploke-eval edit_surface -- --nocapture`
- result: passed, `129 passed; 0 failed; 10 ignored`

What the fix is supposed to prove:

- historical r10 reaches `ChatTurnFinished outcome=completed` after applied
  edits;
- the broad headless-TUI path runs request-declared validation after that stop
  boundary;
- if validation passes, a submitted result is written and verifies against the
  published request.

Next requested task after compaction:

- Set up another live Prototype 1 run in a new worktree.
- Start from the committed checkout at `666477bf` unless the user gives a newer
  base.
- Treat this changelog edit itself as uncommitted handoff state unless the user
  asks to commit it.

## 2026-06-04 New Live Run Setup

Superseded: this setup used the wrong eval/parent model for the intended next
live run. Leave it in place as evidence; do not advance it.

Created the next live-loop seed from the committed fix:

- campaign: `p1-admissionfix-live-20260604-190511`
- worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-admissionfix-live-20260604-190511`
- seed branch: `seed-p1-admissionfix-live-20260604-190511`
- setup-created parent branch:
  `prototype1-parent-p1-admissionfix-live-20260604-190511-gen0`
- setup parent/node id: `node-0053a8307de26534`
- setup parent commit: `aaa3a383`
- base commit: `666477bf`
- profile source:
  `/home/brasides/.ploke-eval/profiles/p1-admissionfix-live-20260604-190511.toml`
- admitted profile:
  `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-live-20260604-190511/prototype1/run-profile.toml`
- admitted profile sha256:
  `764eb95f53ed30325fa17f53b94e68d56312f5d2776df1a96e06fcee43c54ca7`

Profile policy cloned from `p1-memfix-0604a`, changing only the run/profile
identity:

- eval/parent model: `google/gemini-3.5-flash`
- eval/parent route: `direct-google`
- eval/parent provider sentinel: `google`
- protocol model: `google/gemini-2.5-flash`
- protocol route: `direct-google`
- protocol provider sentinel: `google`
- children: min `5`, max `5`, parallel targets `5`
- `execution.broad_tui.max_attempts = 2`
- `execution.broad_tui.fresh_slots_per_child = 2`

Setup verification:

- `cargo build -p ploke-eval`
- result: completed in the new worktree with existing warnings
- `./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json`
- result: `phase = "baseline_eval"`, `blockers = []`,
  `allowed_actions = ["doctor", "continue", "step"]`
- new worktree status after setup/build/doctor: clean on
  `prototype1-parent-p1-admissionfix-live-20260604-190511-gen0`

Provider preflight from this shell:

- `OPENROUTER_API_KEY`: missing
- `GOOGLE_API_KEY`: missing

Do not start the live step from this shell unless the intended direct-Google
credential path is confirmed for the launch environment.

## 2026-06-04 Handoff Evidence Checks

Existing source locations for the handoff evidence boundary:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6178`
  `select_artifact_for_handoff`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6196`
  requires selected candidate artifact surface before handoff
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6204`
  checks selected node id against `SuccessorDecision`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6212`
  requires `decision.selected_branch_id`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6217`
  checks node branch id against `decision.selected_branch_id`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6226`
  requires selected sealed payload evidence
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6235`
  checks sealed payload generation against node generation
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6245`
  checks sealed payload node id against node id
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6253`
  checks sealed payload branch id against selected branch id
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6261`
  requires runtime identity for historical rehydration

Existing tests covering the boundary:

- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4014`
  `historical_node_150_channel_treatment_reaches_current_generation_handoff`
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4247`
  historical current-generation child carries harness `artifact_surface`
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4287`
  historical current-generation path calls `select_artifact_for_handoff`
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4325`
  `current_generation_selector_trace_follows_child_channel_evidence_path`
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4629`
  `history_handoff_rejects_missing_artifact_surface_before_seal`
- `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs:4782`
  `history_handoff_selection_carries_resolved_artifact`

Current happy-path evidence production locations:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:884`
  extracts the applied edit only after `ChatTurnFinished outcome="completed"`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:887`
  runs request-declared validations after that stop boundary
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:949`
  classifies the terminal as `Applied` only when requested validation is not
  failed or missing
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1600`
  passes request evidence roots, edit policy, and declared validation commands
  into the headless TUI adapter
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1682`
  handles the adapter terminal; only `HeadlessTerminal::Applied` reaches
  submitted-result publication
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1827`
  builds `SubmittedHarnessReturnEvidence` from changed files, evidence roots,
  and declared check commands
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1885`
  binds the submitted result to the published request identity
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1897`
  writes the submitted-result JSON at the request-declared path
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1448`
  admits the submitted result only after request binding and workspace
  validation
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1495`
  measures the candidate `ArtifactSurface`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:260`
  projects admitted transaction evidence into child harness evidence, including
  changed paths and artifact surface
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2212`
  attaches admitted harness evidence to the broad child plan
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5042`
  carries child harness artifact surface into `PlannedChildOutcome`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6149`
  carries the planned outcome artifact surface into the candidate artifact used
  by handoff selection

Verification already run:

- `cargo test -p ploke-eval handoff -- --nocapture`
- result: `6 passed; 0 failed`
- `cargo test -p ploke-eval current_generation_selector_trace_follows_child_channel_evidence_path -- --nocapture`
- result: `1 passed; 0 failed`

## 2026-06-04 Corrected Live Run Setup

Created the corrected next live-loop seed from the committed fix:

- campaign: `p1-admissionfix-g31pro-p25flash-20260604-191249`
- worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249`
- seed branch: `seed-p1-admissionfix-g31pro-p25flash-20260604-191249`
- setup-created parent branch:
  `prototype1-parent-p1-admissionfix-g31pro-p25flash-20260604-191249-gen0`
- setup parent/node id: `node-9795f351873f2c49`
- setup parent commit: `719f6b5d`
- base commit: `666477bf`
- profile source:
  `/home/brasides/.ploke-eval/profiles/p1-admissionfix-g31pro-p25flash-20260604-191249.toml`
- admitted profile:
  `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g31pro-p25flash-20260604-191249/prototype1/run-profile.toml`
- admitted profile sha256:
  `1bd62ad6dea95601f9cbe92c543aa07dfc4f13d24ae850db587e047a1b28353e`

Profile policy cloned from
`p1-g31pro-p25flash-10g5c-a2-patch5-eval3-explore-fix-20260604-091439`,
changing only the run/profile identity:

- eval/parent model: `google/gemini-3.1-pro-preview`
- eval/parent route: `direct-google`
- eval/parent provider sentinel: `google`
- protocol model: `google/gemini-2.5-flash`
- protocol route: `direct-google`
- protocol provider sentinel: `google`
- children: min `5`, max `5`, parallel targets `5`
- `execution.broad_tui.max_attempts = 2`
- `execution.broad_tui.fresh_slots_per_child = 2`
- `control.parallel_cap = 3`

Setup verification:

- `cargo build -p ploke-eval`
- result: completed in the corrected worktree with existing warnings
- `./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json`
- result: `phase = "baseline_eval"`, `blockers = []`,
  `allowed_actions = ["doctor", "continue", "step"]`
- doctor effective control:
  `parallel_cap = 3`, `patch_generation_parallel_cap = 5`
- corrected worktree status after setup/build/doctor: clean on
  `prototype1-parent-p1-admissionfix-g31pro-p25flash-20260604-191249-gen0`

Provider preflight from this shell:

- `OPENROUTER_API_KEY`: missing
- `GOOGLE_API_KEY`: missing

Do not start the live step from this shell unless the intended direct-Google
credential path is confirmed for the launch environment.

## 2026-06-04 Historical R10 Fixture Lockfile Repair

Fixed a test-only fixture setup failure in
`historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop`.
The historical r10 synthetic workspace ran `cargo generate-lockfile`, then
unconditionally staged `Cargo.lock`; in this workspace, Cargo did not create a
lockfile for that path-only fixture, so `git add Cargo.lock` failed before the
adapter replay path was exercised.

Patch:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- `install_historical_r10_selection_score_workspace` now guarantees the
  synthetic workspace has a tracked `Cargo.lock` baseline before candidate
  validation runs.

Verification:

- `cargo test -p ploke-eval historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop -- --nocapture`
- result: passed, `1 passed; 0 failed`
- observed changed paths stayed limited to:
  `crates/ploke-selection-score/src/common/ranking.rs` and
  `crates/ploke-selection-score/src/ploke/frontier.rs`
- `cargo test -q -p ploke-eval edit_surface`
- result: passed, `129 passed; 0 failed; 10 ignored`

Follow-up full-lib failure and repair:

- `cargo test -q -p ploke-eval --lib` reproduced an order-sensitive failure in
  the same historical r10 test.
- The adapter and backend were behaving correctly: admission rejected
  `Cargo.lock` as an out-of-policy authority file under
  `WorkspaceExceptPlokeEval`.
- The fixture was unstable because the synthetic baseline sometimes lacked a
  tracked `Cargo.lock`; later validation could create it in the candidate
  worktree, making the candidate appear to edit an authority file.
- The test fixture now seeds and commits the minimal lockfile for its closed
  two-crate path-only workspace before the candidate worktree is created.
- `cargo test -q -p ploke-eval --lib`
- result: passed, `767 passed; 0 failed; 27 ignored`

## 2026-06-04 Fresh Live Run Setup

Created a new profile-backed Prototype 1 seed using the same policy as the last
corrected setup, changing only the run/profile identity:

- campaign: `p1-admissionfix-g31pro-p25flash-20260604-195208`
- worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-195208`
- seed branch: `seed-p1-admissionfix-g31pro-p25flash-20260604-195208`
- setup-created parent branch:
  `prototype1-parent-p1-admissionfix-g31pro-p25flash-20260604-195208-gen0`
- setup parent/node id: `node-90cab936910719cb`
- setup parent commit: `7a2dbc78`
- base commit: `666477bf`
- profile source:
  `/home/brasides/.ploke-eval/profiles/p1-admissionfix-g31pro-p25flash-20260604-195208.toml`
- admitted profile:
  `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g31pro-p25flash-20260604-195208/prototype1/run-profile.toml`
- admitted profile sha256:
  `8b6135f87ecbe2c588ebba64df1cf40f25fb08a96beba458e9c10c55dc96c8ba`

Profile policy:

- eval/parent model: `google/gemini-3.1-pro-preview`
- eval/parent route: `direct-google`
- eval/parent provider sentinel: `google`
- protocol model: `google/gemini-2.5-flash`
- protocol route: `direct-google`
- protocol provider sentinel: `google`
- children: min `5`, max `5`, parallel targets `5`
- `search.max_generations = 10`
- `search.max_total_nodes = 64`
- `search.require_keep_for_continuation = true`
- `search.explore_from_rejected = true`
- `selection.strategy = "history-score-child-prop"`
- `selection.evidence = "operational-and-protocol"`
- `selection.metrics.score_profile = "operational-quality-v1"`
- `selection.oracle.mode = "record-only"`
- `selection.oracle.require_evidence = true`
- `protocol.max_tokens = 8000`
- `protocol.tool_review_parallelism = 8`
- `execution.broad_tui.max_attempts = 2`
- `execution.broad_tui.fresh_slots_per_child = 2`
- `control.parallel_cap = 3`

Setup verification:

- `cargo build -p ploke-eval`
- result: completed in the new worktree with existing warnings
- `./target/debug/ploke-eval loop prototype1-doctor --repo-root . --format json`
- result: `phase = "baseline_eval"`, `blockers = []`,
  `allowed_actions = ["doctor", "continue", "step"]`
- doctor effective control:
  `parallel_cap = 3`, `patch_generation_parallel_cap = 5`
- new worktree status after setup/build/doctor: clean on
  `prototype1-parent-p1-admissionfix-g31pro-p25flash-20260604-195208-gen0`

Provider preflight from this shell:

- `OPENROUTER_API_KEY`: missing
- `GOOGLE_API_KEY`: missing

Do not start the live step from this shell unless the intended direct-Google
credential path is confirmed for the launch environment.

## 2026-06-06 Admissionfix 195048 Run-Review Update

Reviewed the run-review artifacts for
`p1-admissionfix-g35flash-p25flash-20260605-195048`. They were not present as
unstaged edits in the main checkout; they were unstaged in the failed run
worktree:

`/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260605-195048/docs/active/agents/run-reviews/`

The review set says this was not a repeat of the earlier "otherwise valid edit
never writes submitted result" bug. The per-slot submitted-result layer worked
for two slots:

- `node-28d0482d4bc80fcb-r3.json` exists and binds to a committed candidate
  diff in `crates/ingest/ploke-mbe/src/structural.rs`.
- `node-28d0482d4bc80fcb-r4.json` exists and binds to a committed candidate
  diff in `crates/ploke-ty-mcp/src/manager.rs`.

The persistence failure is at the parent/batch handoff layer, not at those two
slot files:

- child budget required `min = 5`, `max = 5`;
- only `r3` and `r4` reached submitted/applied candidate state;
- the other trace-bearing slots timed out, hit provider-unavailable, or never
  reached a submitted-result artifact;
- the parent `node.json` still says `running`;
- `runner-result.json` is absent;
- `scheduler.json` still has the parent as frontier/planned with no completed
  or failed node ids;
- no completed child plan / successor handoff evidence exists for this run.

Important quality caveat: `r3` is mechanically admissible but benchmark benefit
is unproven. `r4` is mechanically persisted but likely invalid/benchmark-useless:
the model intended a six-edit change, while the candidate commit contains only a
one-line `start_locks` field addition and no matching constructor/method
changes.

Saved the detailed source/artifact walkthrough at:

`docs/active/agents/2026-06-02_prototype1-state-loop-walkthrough/edit-surface-persistence-walkthrough-2026-06-06.md`
