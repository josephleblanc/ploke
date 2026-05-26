# Prototype 1 OODA Log

Started: 2026-05-25 22:12:59 PDT

Purpose: keep current operator state explicit so decisions are not made from
stale notes, stale memory, or the last local symptom.

Rules:

- Use only current-session facts unless an older artifact is explicitly
  revalidated.
- Record uncertainty as uncertainty.
- Before changing code or running a live command, add an OODA entry with the
  intended action and the reason it helps the Prototype 1 credibility goal.
- Do not treat old orchestration notes as current state.

## Mission

Make Prototype 1 into a credible self-improvement loop by proving the loop as a
sequence of focused, repeatable, authority-preserving steps before trusting a
long live run.

The loop exists to hill-climb a target benchmark while preserving enough typed
state, digests, persisted records, and replay evidence that the result is
believable and resistant to self-editing reward hacks.

Do not keep running the whole loop when sub-steps keep failing. Instead, build a
proof ladder:

1. Prove setup/admission.
2. Prove baseline eval.
3. Prove baseline protocol.
4. Prove child-plan request publication.
5. Prove parent patch generation, including live parallel slot execution under
   `search.children.parallel_targets`.
6. Prove child-plan admission and rejected-attempt persistence.
7. Prove child materialization, build, and cleanup.
8. Prove child self-eval and runner result production.
9. Prove parent observation through channel-authoritative records.
10. Prove compare/selection.
11. Prove Crown/History handoff and successor restart.
12. Only then run a long live loop with confidence.

For each step, use focused local or live tests as appropriate. Live API tests
are acceptable and preferred when they expose real provider/tool-loop behavior
earlier than a full run would. Gate Rust live tests behind `live_api_tests` and
run them deliberately.

Use `observe.rs` and persisted data as the proof surface. Each step should show
the phase, typestate transition, authority records read or written,
runtime/channel state when applicable, durable artifact paths, and retry
behavior.

Current immediate target: replace the single-slot live child-plan canary with,
or supplement it by, a focused live test that proves parent patch-generation
slots actually execute concurrently up to `search.children.parallel_targets`.

## Entry 1: Stop And Reorient

Observe:

- User interrupted because I was reacting to symptoms and reaching for stale
  orchestration notes.
- Current git status was checked immediately before this log was created:
  only `crates/ploke-eval/src/cli/prototype1_state/run/core.rs` and
  `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  were dirty.
- The process check did not show a real active `cargo test` or `ploke-eval`
  child; it matched the sandbox wrapper around the command itself.
- The previously found `docs/active/agents/2026-05-24_prototype1-orchestrator-loop-notes.md`
  is stale for current decision-making unless revalidated.

Orient:

- The immediate problem is operator drift, not a missing code edit.
- The active technical thread is still Prototype 1 credibility: historical
  blockers must become focused regression evidence before live loop work is
  trusted.
- The live Google child-plan test has already been run more than once in this
  session. Re-running it reflexively would be another drift error.

Decide:

- Start this fresh OODA log before more code or live commands.
- Treat the dirty diff as pending instrumentation/test work, not as proven
  accepted design.
- Next technical move must be chosen from current evidence, not old notes.

Act:

- Created `docs/active/agents/operator-logs/`.
- Created this fresh OODA log and index.

## Entry 2: Rerun Bounded Live Child-Plan Test

Observe:

- User explicitly asked to run the test again.
- The test in scope is the already-run bounded live Google child-plan test:
  `live_google_step_child_plan`.
- The live test was previously run more than once, so this rerun should be
  treated as a deliberate verification pass, not discovery by repetition.

Orient:

- This is not a full Prototype 1 loop advance.
- The useful evidence is whether the current test instrumentation now gives
  bounded timing/resource output while exercising the real `prototype1-step`
  child-plan path through live Google.
- The run should remain limited to one ignored test target.

Decide:

- Run exactly:
  `cargo test -q -p ploke-eval live_google_step_child_plan -- --ignored --nocapture`
- Do not start any other loop command from this result.

Act:

- Ran `cargo test -q -p ploke-eval live_google_step_child_plan -- --ignored --nocapture`.
- Result: passed.
- Runtime: 60.02s for the live test body.
- Terminal headless-TUI state: `TimedOut { secs: 60 }`.
- The live path produced model-driven tool activity:
  33 tool requests, 28 completed tool calls, 5 failed tool calls, 1 cargo
  validation, 0 proposals.
- Resource snapshot:
  - RSS moved from about 35 MB at `google_auth_checked` to about 119 MB after
    diagnostics loaded.
  - Candidate workspace at completion: 39,358 bytes across 40 entries.
  - Candidate target directory at completion: 38,477 bytes across 24 entries.
- Important behavior observed:
  - The model used live tools quickly: first tool request at about 1.8s.
  - The first `cargo check` succeeded in about 43ms.
  - A later package-scoped `cargo check -p ploke-eval` failed because the
    synthetic fixture workspace does not contain a `ploke-eval` package.
  - Several absolute-path `list_dir` calls outside configured roots failed,
    which is useful evidence about model/tool-root confusion in this fixture.

## Entry 3: Audit Live-Test Evidence Before More Loop Work

Observe:

- The bounded live test passed, but it timed out with zero proposals.
- The current dirty diff includes test diagnostics in `run/core.rs`, live
  observer output in `tui_adapter.rs`, and this operator log.
- The passed live test proves provider/tool-loop reachability, not edit
  admission or successor handoff.

Orient:

- The next useful step is to make sure the test says exactly what it proves:
  live Google reaches the real child-plan path, zero-admission/rejected-attempt
  evidence is persisted, and timeout is not confused with admission success.
- Runtime instrumentation should not make normal Prototype 1 output noisier
  unless it is explicitly gated.

Decide:

- Audit the dirty diff before more edits.
- Keep only instrumentation that is gated or narrowly diagnostic.
- Tighten assertions if the live test is overclaiming.
- Run focused non-live child-plan authority tests after changes.

Act:

- Audited the dirty diff against the child-plan authority and edit-pipeline
  docs.
- Kept the resource probes gated behind `PLOKE_EVAL_HEADLESS_TUI_LIVE` plus
  `PLOKE_EVAL_HEADLESS_TUI_LIVE_RESOURCES`; normal headless output is not
  changed unless those diagnostics are explicitly enabled.
- Tightened `live_google_step_child_plan` so a test-limited one-slot live step
  must write exactly one broad request and exactly one resulting admitted child
  or rejected surface attempt.
- Preserved the distinction between live provider/tool-loop reachability and
  successful edit admission.
- Ran focused non-live checks:
  - `cargo fmt --all`
  - `cargo test -q -p ploke-eval step_persists_zero_admission_plan -- --nocapture`
  - `cargo test -q -p ploke-eval zero_admission_batch_is_persisted -- --nocapture`
  - `cargo test -q -p ploke-eval broad_harness_batch_admits_three_transactions_into_three_children -- --nocapture`
  - `cargo test -q -p ploke-eval below_min_rejected_attempts_are_persisted_and_recoverable_from_existing_child_plan -- --nocapture`
  - `cargo test -q -p ploke-eval child_plan_replay_rejects_wrong_parent -- --nocapture`
- Result: all focused checks passed.
- Remaining caveat: the tightened live assertion itself has not been rerun after
  this assertion change. The previous live run strongly suggests it will pass
  because it produced one request, zero children, and one rejected timed-out
  attempt, but that exact stricter assertion has not yet been live-verified.

## Entry 4: Correct Test Intent

Observe:

- User corrected my framing of `live_google_step_child_plan`.
- I described it as a live child-plan authority canary.
- The actual intent of the work was to test whether the child patch-generation
  step can run in parallel under the run-profile child fanout policy.

Orient:

- My tightened assertions are useful for the zero-admission authority claim,
  but they do not prove parallel slot execution.
- The current `PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT=1` setting actively prevents
  the live test from exercising parallel child-plan slot execution.
- This means the current live test shape is misaligned with the stated
  functionality we were trying to verify.

Decide:

- Stop treating the current live test as sufficient for the parallelism change.
- Re-inspect the child-plan fanout code and tests around
  `[search.children].parallel_targets`.
- Either rename/scope the current test as an authority/live-route canary or
  change/add a test that actually exercises parallel slot execution.

Act:

- Pending code inspection.

## Entry 5: Prove Parent Patch-Generation Fanout

Observe:

- The primary child-plan authority docs say the parent patch-generation unit is
  the broad-harness request slot.
- The current fanout code uses `JoinSet` in `admit_broad_harness_batch` and a
  `patch_generation_parallel_cap` copied from
  `[search.children].parallel_targets`.
- Existing tests prove publication, cap storage, admission ordering, and
  zero-admission persistence. They do not yet prove that more than one
  unfinished patch-generation slot can be active at the same time.
- The current live Google test still forces `PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT=1`,
  so it cannot prove parallel patch generation.

Orient:

- The immediate proof should be deterministic first: make two spawned slot
  tasks block at a test-only barrier until both have started, then verify the
  parent still writes the expected rejected-attempt child plan.
- That isolates the scheduler contract without spending live API time or
  conflating provider behavior with fanout behavior.
- After that, a live Google test can be reshaped to exercise the same step with
  a slot cap greater than one.

Decide:

- Add a test-only slot-start probe inside `run_broad_slot_for_admission`, only
  for slots that would actually run the headless-TUI attempt.
- Drive it with a local fixture summary so the proof does not depend on model
  output.
- Keep the proof narrow: two slots, `parallel_targets = 2`, slot limit two,
  expected zero admitted children, and two rejected attempts persisted in the
  `ChildPlan` message.

Act:

- Implementing the deterministic fanout probe and test next.

## Entry 6: Local Fanout Proof Passed

Observe:

- Added a test-only probe at the broad-harness slot execution point. It writes a
  start marker for each slot, waits until the configured number of slots have
  started, then writes a release marker.
- The probe is only active when `PLOKE_EVAL_BROAD_TUI_SLOT_PROBE_DIR` is set.
- The deterministic test publishes exactly two request slots, sets
  `parallel_targets = 2`, and uses a historical timeout diagnostic fixture so
  no live provider behavior can affect the scheduler proof.

Orient:

- If fanout were accidentally serial, the first slot would time out waiting for
  the second start marker and the test would fail.
- Passing means `admit_broad_harness_batch` starts two unfinished
  patch-generation slots concurrently up to the configured cap.
- The test also rechecks the authority boundary: even with zero admitted
  children, the parent writes a child-plan message with two rejected attempts.

Decide:

- Keep the deterministic test as the fast proof of scheduler concurrency.
- Add a separate ignored live Google test that uses the same two-slot shape and
  records timing/resource evidence, rather than modifying the existing one-slot
  canary into a confused mixed-purpose test.

Act:

- Ran `cargo fmt --all`.
- Ran `cargo test -q -p ploke-eval broad_slots_run_in_parallel -- --nocapture`.
- Ran `cargo test -q -p ploke-eval broad_batch_ -- --nocapture`.
- Ran `cargo test -q -p ploke-eval zero_admission_batch_is_persisted -- --nocapture`.
- Ran `cargo test -q -p ploke-eval broad_harness_batch_admits_three_transactions_into_three_children -- --nocapture`.
- Ran `cargo test -q -p ploke-eval step_persists_zero_admission_plan -- --nocapture`.
- Result: all passed.
- Caveat: cargo emits a large amount of existing warnings in `ploke-eval`; these
  are noisy but not introduced by this fanout proof.

## Entry 7: Run Live Parallel Slot Proof

Observe:

- The new ignored live test compiles with `--features live_api_tests`.
- It uses a two-child profile shape (`min = 1`, `max = 2`) and a test-scoped
  two-slot limit.
- It uses the same slot-start probe as the deterministic test, but after the
  barrier releases each slot continues into the real headless-TUI Google path.

Orient:

- This should prove two things at once: the scheduler starts two live
  patch-generation slots concurrently, and each slot reaches a real
  model/tool-loop attempt that writes headless-TUI diagnostics.
- The test may still end with zero admitted children. That is acceptable if the
  failed batch persists exactly two rejected attempts in the child-plan message.
- The live provider call is worth doing now because it catches fanout/resource
  or route problems before a long Prototype 1 run.

Decide:

- Run only the focused ignored live test:
  `cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
- Do not start a full loop run from this evidence.

Act:

- Running the live test next.
- Result: failed quickly.
- Both slots started, both reached prompt construction, and both wrote
  headless-TUI diagnostics, so the live fanout barrier itself worked.
- Both live attempts then ended as `aborted` in about 200 ms with zero tool
  requests:
  `Previous attempt aborted before staging an edit.`

## Entry 8: Diagnose Parallel Live Abort

Observe:

- The failure is not "second slot never started"; both `start-*` and
  `release-*` markers were present before diagnostics were checked.
- The failure is also not a long timeout; both attempts aborted almost
  immediately after prompt construction.
- The current headless summary only preserves the high-level chat-turn summary,
  not the underlying `LoopError` or provider-attempt details needed to know
  whether this is provider status, response parsing, cancellation, or a local
  state-machine issue.

Orient:

- This is exactly why the proof ladder is useful: the focused live step found a
  narrower blocker before a long loop run.
- The next action should expose the HTTP/provider phase without logging
  credentials.
- `PLOKE_PROTOCOL_DEBUG=1` emits bounded provider-attempt observations to
  stderr without bearer tokens.

Decide:

- Rerun only the same focused live test with `PLOKE_PROTOCOL_DEBUG=1`.
- Use the result to classify whether the abort is a live Google/provider issue
  or a local concurrent headless-TUI/session issue.

Act:

- Running the focused debug rerun next.
- Result: the rerun failed the same way, but with a better boundary.
- Both slots reached prompt construction and wrote diagnostics.
- No `chat_http_*` provider trace appeared with `PLOKE_PROTOCOL_DEBUG=1`.
- That means the attempts most likely aborted locally before the provider HTTP
  call began, or the debug surface is not attached at the point where the abort
  happens.

## Entry 9: Compare Parallel Abort Against Single-Slot Live Step

Observe:

- The two-slot live proof currently fails with two fast `aborted` turns and zero
  tool requests.
- The runtime keeps the cancel sender in `TestRuntimeInner`, so a dropped
  one-off receiver is not enough to explain the abort.
- The direct-Google route smoke tests passed outside this path, so the next
  useful split is same execution path with one slot vs two slots.

Orient:

- If the one-slot `prototype1-step` live test aborts before HTTP too, the bug is
  in the direct-Google headless step path itself.
- If one slot reaches a model/tool-loop but two slots abort, the bug is in
  concurrent headless runtimes, shared session state, or shared env/config.
- This is a bounded proof-ladder step; it is cheaper and safer than starting a
  full Prototype 1 loop.

Decide:

- Run the single-slot live step with provider debug enabled.
- Preserve the output boundary in this log before editing runtime behavior.

Act:

- Running `live_google_step_child_plan` next with `PLOKE_PROTOCOL_DEBUG=1`.
- Result: the single-slot test failed the same way as the two-slot test.
- Evidence:
  - prompt construction completed in the headless TUI runtime;
  - `ChatTurnFinished` reported `outcome=aborted`;
  - no provider HTTP debug events were printed;
  - no tool requests were observed;
  - elapsed time was about 230 ms, far below the 60 second attempt timeout.
- Revised classification: this is not primarily a parallel fanout failure. The
  direct-Google Prototype 1 headless step path aborts locally before the model
  request begins.

## Entry 10: Trace Local Abort Source

Observe:

- The abort summary is generated by the TUI chat session as `[aborted]`.
- In `run_chat_session`, that shape is produced by `abort_for_user_cancel`.
- `abort_for_user_cancel` is reached either if the cancel receiver already
  contains `Close`, or if `wait_for_cancel_signal` wins the `tokio::select!`
  against the provider step.

Orient:

- Because no provider debug events appear, the cancel branch is still the most
  likely local source.
- The sender is held by the `App`, so a simple dropped sender is not the whole
  story.
- The next useful proof is to instrument or test the cancel path without
  changing provider behavior.

Decide:

- Read `submit_prompt` and the app cancel-token paths before changing logic.
- If the prompt path sends `Close` or leaves the token in `Close`, fix the
  headless setup to reset it to `KeepOpen` at the correct boundary.
- If not, add a tiny focused diagnostic around cancel observation in the chat
  session and rerun the focused live test.

Act:

- Inspecting prompt submission and cancel-token lifecycle next.
- Direct `chat_step` canary isolated the source:
  `failed to resolve bearer token: ... failed to resolve Google application default credentials`.
- `gcloud auth application-default print-access-token` also fails non-
  interactively with `Reauthentication failed`; copying the gcloud config to a
  writable temp dir does not fix the stale ADC token.
- This environment cannot currently make the intended direct-Google API call
  without a fresh interactive ADC login, but the code should not report that as
  generic headless edit exhaustion.
- Updated the code path so:
  - live Google test preflight resolves a bearer token, not just ADC config;
  - `ChatTurnFinished` summaries include last loop-error code/kind/summary;
  - the headless adapter recognizes Google ADC/bearer-token failure as
    provider-unavailable instead of retrying as an edit failure.

## Entry 11: Verify Auth Failure Classification

Observe:

- The proof ladder still needs a real live Google run once credentials are
  fresh in this process.
- Before that, the local classification path should be tested so future stale
  auth failures stop early and legibly.

Orient:

- This is not the final parallel-slot proof. It is a repair to make the proof
  fail at the right boundary when the provider token cannot be resolved.
- The deterministic fanout test remains the local concurrency proof.

Decide:

- Run formatting and focused tests for the new summary/classification behavior.
- Then rerun the focused live tests; if ADC is still stale, expect an explicit
  skip/provider-unavailable boundary rather than a misleading edit exhaustion.

Act:

- Running focused local verification next.
- `cargo fmt --all` passed.
- `cargo test -q -p ploke-tui chat_session_summary_preserves_last_error_detail -- --nocapture`
  passed.
- `cargo test -q -p ploke-eval aborted_summary_google_adc_failure_is_provider_unavailable -- --nocapture`
  passed.
- `cargo test -q -p ploke-eval --features live_api_tests live_google_step_child_plan -- --ignored --nocapture`
  now exits cleanly before stepping because ADC token resolution fails.
- `cargo test -q -p ploke-eval --features live_api_tests live_google_resolved_route_forces_list_dir_tool_call_success_or_quota -- --ignored --nocapture`
  also exits cleanly before HTTP for the same stale ADC token.

Current boundary:

- Local deterministic parent slot fanout is proven.
- Direct Google provider auth is not currently live in this sandbox process.
- The next real API proof requires fresh ADC token availability inside this
  process; until then, the live tests correctly refuse to produce misleading
  Prototype 1 evidence.

## Entry 12: Revalidate Parallel Slot Boundary After Compaction

Observe:

- The active goal is still the proof ladder, not a full live loop.
- The immediate target is parent patch-generation slot concurrency bounded by
  `search.children.parallel_targets`.
- The working tree still contains the parallel-slot proof, live auth
  classification, and operator-log changes.

Orient:

- The deterministic test is the strongest current proof of scheduler behavior:
  it uses the real broad harness request publication/admission path and a
  two-slot barrier. A serial implementation would timeout waiting for the
  second slot.
- The live Google parallel-slot test is still valuable, but only if the direct
  Google route can resolve an ADC bearer token in this process.

Decide:

- Re-run the focused deterministic proof first.
- Then run the ignored `live_api_tests` Google parallel-slot path with warnings
  suppressed so the result is legible.
- Treat an auth skip as an environment boundary, not as loop evidence.

Act:

- `cargo test -q -p ploke-eval broad_slots_run_in_parallel -- --nocapture`
  passed: 1 test passed, no failures.
- `RUSTFLAGS=-Awarnings PLOKE_PROTOCOL_DEBUG=1 cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
  passed by explicit skip:
  `direct Google route is configured for this live test, but missing Google ADC auth`.

Current boundary:

- Parent patch-generation slot concurrency is locally proven through the real
  request publication/admission path.
- The live API proof is ready to run, but this process cannot currently resolve
  the Google ADC bearer token.
- Do not start a long live loop from this state. The next useful step is either
  fresh ADC availability followed by the live parallel-slot proof, or the next
  proof-ladder step that does not require provider auth.

## Entry 13: Clean Up Test Support And Re-run The Local Net

Observe:

- Diff review showed a duplicate test `EnvGuard` still present in
  `run/core.rs` after adding shared `crate::test_support::EnvGuard`.
- That duplicated an existing carrier and violated the type-reuse discipline.

Orient:

- This is a cleanup issue in the proof harness, not a loop behavior change.
- It should be fixed before expanding the proof ladder so the test surface does
  not accumulate local duplicate scaffolding.

Decide:

- Remove the local `EnvGuard` type and use `crate::test_support::env_guard_os`
  from the run tests.
- Re-run the focused parallel-slot proof and related broad-batch admission
  tests.
- Re-run the live parallel-slot test to ensure the auth boundary still reports
  cleanly.

Act:

- Removed the duplicate local test env guard from `run/core.rs`.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_slots_run_in_parallel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_batch_ -- --nocapture`
  passed: 3 tests passed.
- `RUSTFLAGS=-Awarnings PLOKE_PROTOCOL_DEBUG=1 cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
  passed by explicit ADC-auth skip.
- `cargo fmt --all` passed.

Current boundary:

- The parent patch-generation concurrency proof is clean enough to move forward
  locally.
- The live proof remains pending fresh direct-Google ADC availability.

## Entry 14: Prove Child Fanout Cap

Observe:

- The next resource-risk stage after parent patch generation is parent-side
  child execution fanout.
- A new focused test using real admitted `ChildFiles` initially failed:
  `run_child_fanout` started all 3 full-batch children even when
  `parallel_targets = 2`.

Orient:

- This was not a test expectation issue. `Prototype1ChildScheduleMode::FullBatch`
  treated fanout width as all planned children.
- That contradicted the newer policy decision that `[search.children].parallel_targets`
  is the resource cap for concurrent work, defaulting to `min(3, max)`.
- The correct interpretation is: full-batch means the whole admitted batch
  should eventually run, but in capped concurrent chunks.

Decide:

- Change `Prototype1ChildScheduleMode::fanout_width` so both full-batch and
  adaptive-batch respect `child_budget.parallel_targets()`.
- Derive the profile default `control.parallel_cap` from the same scheduler
  fanout policy.
- Keep the child fanout proof local and deterministic by stopping at
  materialization and using a test-only two-child barrier.

Act:

- Updated `fanout_width` and profile default control cap.
- Updated `zero-admission-flow.md` to document that child execution fanout is
  also capped by `parallel_targets`.
- Added `child_fanout_is_parallel`, which:
  - publishes/admitted broad-harness children through the existing path;
  - calls `run_child_fanout` with `StopAfter::Materialize`;
  - proves only the first two children start in the first capped batch;
  - verifies both materialized children reached `WorkspaceStaged`.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_fanout_is_parallel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval parallel_cap -- --nocapture`
  passed: 4 tests passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval fanout_width -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_slots_run_in_parallel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_batch_ -- --nocapture`
  passed: 3 tests passed.

Current boundary:

- Parent patch generation is locally proven to start slots concurrently up to
  `parallel_targets`.
- Parent-side child materialization fanout is locally proven to start children
  concurrently in capped chunks.
- The direct-Google live parallel-slot proof is still pending working ADC in
  this process.

Follow-up:

- Added `run_child_fanout` and `run_planned_child` to
  `docs/workflow/pipeline-registry.jsonl` under
  `prototype1.parent_child_channel`, with links to the parent/child channel
  docs and this zero-admission flow note.
- `target/debug/xtask pipeline check` passed.
- `RUSTFLAGS=-Awarnings PLOKE_PROTOCOL_DEBUG=1 cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
  still passes by explicit ADC-auth skip.

## Entry 15: Prove Child Build Promotion And Scratch Cleanup

Observe:

- After materialization, the next proof-ladder stage is the parent-side child
  build transition.
- Disk pressure comes from temporary target directories, so the proof should
  specifically check that build scratch output is not treated as durable state.

Orient:

- A full workspace build is unnecessary for this proof and would spend disk.
- The useful boundary is the Prototype 1 transition path, not Cargo itself.

Decide:

- Exercise `run_planned_child(... StopAfter::Build ...)` through real admitted
  `ChildFiles`.
- Replace only the external `cargo` binary with a test-local fake that honors
  `CARGO_TARGET_DIR` and creates the expected `debug/ploke-eval` artifact.
- Verify the promoted child binary exists and the node scratch `target/` dir is
  removed.

Act:

- Added `child_build_promotes_binary_and_cleans_scratch`.
- Added that test to the `run_planned_child` pipeline-registry entry.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_build_promotes_binary_and_cleans_scratch -- --nocapture`
  passed.
- `target/debug/xtask pipeline check` passed.

Current boundary:

- Parent patch generation, child materialization fanout, and child build
  promotion/cleanup now each have focused local proofs.
- Spawn/observe still need focused proof before trusting a long live loop.

Verification refresh:

- Re-ran `child_fanout_is_parallel`, `child_build_promotes_binary_and_cleans_scratch`,
  `parallel_cap`, `fanout_width`, `broad_slots_run_in_parallel`, and
  `broad_batch_`; all passed.
- Re-ran the gated live Google parallel-slot test; it still passes by explicit
  ADC-auth skip.

## Entry 16: Existing Channel/Observe Proofs

Observe:

- Spawn/observe is the next major proof-ladder area, but this area already has
  a registered parent/child channel pipeline with tests.

Orient:

- Before adding new tests, run the existing channel tests to avoid duplicating
  proof coverage.
- These tests do not prove a full live spawn, but they do prove the channel
  authority contract around terminal result observation and projection
  non-authority.

Decide:

- Run the registered observe/channel tests as the current baseline for this
  rung.
- Treat any remaining gap as "spawn through real child process" rather than
  "observe semantics from channel artifacts".

Act:

- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval observe_child_ -- --nocapture`
  passed: 9 tests passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval parent_and_child_have_opposite_directions_from_existing_role_states -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval file_transport_reads_only_new_complete_records -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval current_generation_selector_trace_follows_child_channel_evidence_path -- --nocapture`
  passed.

Current boundary:

- Observe/channel semantics have focused existing coverage.
- The remaining unproven local rung is a real spawn path from promoted child
  binary into channel ready/evaluating/result behavior, without relying on a
  full live loop.

## Entry 17: Live Google Parallel Slot Proof

Observe:

- Google auth was refreshed and live calls are available again.
- The immediate target is not "run the whole loop"; it is to prove that parent
  patch-generation slots can run concurrently up to `parallel_targets` on the
  real Google route.

Orient:

- The local barrier test proves concurrency in the same execution path, but a
  live provider test is needed because provider latency and the headless TUI
  loop are the actual pressure point.
- A timeout can still be useful evidence if the test proves concurrent starts,
  provider calls, resource profile, persisted summaries, and bounded terminal
  behavior.

Decide:

- Run the gated live Google test directly with `live_api_tests`.
- Treat the proof target as concurrency and model/tool-loop visibility, not
  successful patch production.

Act:

- `RUSTFLAGS=-Awarnings PLOKE_PROTOCOL_DEBUG=1 cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
  passed.
- The test made real direct-Google chat-completion calls and started two
  headless-TUI slots together.
- Both live slots reached the configured 60-second timeout. That is acceptable
  for this rung because the test proves the parallel execution path and bounded
  terminal behavior.
- Diagnostics showed both slots performed tool work but produced no edit
  proposals:
  - slot 0: 64 events, 32 tool requests, 31 completed tools, 1 failed tool,
    0 proposals;
  - slot 1: 60 events, 30 tool requests, 28 completed tools, 2 failed tools,
    0 proposals.
- The strongest live observation is that direct-Google routing works and parent
  patch-generation slots overlap in the real headless TUI path, but the model
  spent the budget exploring and did not stage edits.

Current boundary:

- Parent patch generation is now proven in both local and live-Google forms to
  start concurrent slots up to `parallel_targets`.
- This does not prove good patch quality or eventual child handoff.
- The next missing proof rung remains real child process spawn from a promoted
  child binary into authoritative channel records.

## Entry 18: Focused Child Spawn Proof

Observe:

- The next unproven rung after parent patch generation and child build was C3:
  spawning a promoted child runtime and observing readiness.
- The pipeline registry had `SpawnChild` registered, but the only listed test
  was indirect.

Orient:

- A long live loop is the wrong proof surface for this rung.
- The useful local proof is the real parent path through
  `run_planned_child(... StopAfter::Spawn ...)`, with the child executable
  reduced to the minimum behavior needed for the C3 handshake.
- The authority being tested is the per-runtime child-to-parent channel,
  not the transition journal or a result-file projection.

Decide:

- Add a focused Unix test that:
  - admits one real broad-harness child;
  - uses fake Cargo only to promote a small child executable;
  - runs the real parent-side C1 -> C2 -> C3 -> C4 path;
  - has the fake child write a valid `ToParent::Ready` channel envelope and
    remain alive briefly so the parent observes readiness before process exit.

Act:

- Added `child_spawn_observes_ready`.
- Added the test to the `prototype1.parent_child_channel` registry entries for
  `SpawnChild` and `run_planned_child`.
- Updated `docs/workflow/evalnomicon/drafts/runtime/child.md` to say the
  current C3 ready authority is the per-runtime channel, while spawn journal
  entries are projection/audit records.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_spawn_observes_ready -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_ -- --nocapture`
  passed: 68 passed, 2 ignored.
- `target/debug/xtask pipeline check` passed.

Current boundary:

- The proof ladder now covers parent patch-generation concurrency, parent-side
  child materialization fanout, child build promotion/cleanup, and C3 spawn
  readiness through the channel.
- The next missing rung is either a focused child self-evaluation terminal
  result proof or a bounded C4 observation proof that consumes a terminal
  `ToParent::Result` from a spawned child process rather than a test thread.

## Entry 19: Spawned-Process Terminal Result Proof

Observe:

- Existing C4 tests prove terminal channel authority with a sender thread.
- That is useful, but it does not prove the parent path can spawn a child
  process and then observe a terminal channel `Result` from that process.

Orient:

- A full child self-evaluation is still too large for this rung.
- A failed terminal result is enough to prove C4 consumes the authoritative
  `ToParent::Result` payload through the per-runtime channel without requiring
  treatment evidence.
- The test should avoid relying on result sidecars or compatibility
  `ResultWritten` projections.

Decide:

- Add a spawned-process terminal proof using the real
  `run_planned_child(... StopAfter::Complete ...)` path.
- Keep fake Cargo only as the child executable producer.
- Make the promoted child executable write valid `Ready`, `Evaluating`, and
  failed terminal `Result` channel envelopes.

Act:

- Added `child_spawn_observes_failed_result`.
- Refactored the local fake-Cargo setup into a shared Unix test helper.
- Registered the new proof under `run_planned_child`, `ObserveChild`,
  `child_result_from_channel`, `ToParent`, and `runner_result` in
  `docs/workflow/pipeline-registry.jsonl`.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_spawn_observes -- --nocapture`
  passed: 2 passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_harness_batch_rejects_below_minimum_admitted_transactions -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_ -- --nocapture`
  passed: 69 passed, 2 ignored.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval observe_child_ -- --nocapture`
  passed: 9 passed.
- `target/debug/xtask pipeline check` passed.
- `git diff --check` passed.

Current boundary:

- C3 spawn readiness and C4 failed terminal result observation are now proven
  through a spawned process.
- The remaining missing high-value proof is a successful child terminal result
  with treatment evidence flowing into parent comparison, or a live/focused
  child self-evaluation that produces real treatment evidence.

## Entry 20: Existing Success/Comparison Proofs

Observe:

- Before adding another synthetic success test, check whether success and
  parent comparison are already covered.

Orient:

- A spawned failed terminal result proves the process/channel boundary.
- Separate existing tests can still prove that successful treatment evidence
  reaches parent comparison and historical handoff logic.

Decide:

- Run the focused success/comparison tests instead of adding another local
  synthetic process test immediately.

Act:

- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval parent_compares_treatment_evidence_against_owned_baseline -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval historical_node_150_channel_treatment_reaches_current_generation_handoff -- --nocapture`
  passed.

Current boundary:

- The local proof ladder now covers:
  - parent patch-generation concurrency;
  - child materialization fanout;
  - child build promotion and scratch cleanup;
  - spawned C3 ready acknowledgement through the child channel;
  - spawned C4 failed terminal result observation through the child channel;
  - successful treatment evidence comparison and historical handoff logic.
- The remaining gap is live child self-evaluation producing real treatment
  evidence under the current source, not the parent/child channel authority
  mechanics themselves.

## Entry 21: Broader Ploke-Eval Validation

Observe:

- The focused tests passed, but the source changed enough to warrant a
  broader crate-level pass before another live step.

Orient:

- This is still cheaper and more informative than starting a long loop.
- Root disk is tight but stable enough for bounded crate tests.

Decide:

- Run the full `ploke-eval` local suite.
- Check disk at the same time.

Act:

- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval` passed.
  - lib tests: 762 passed, 21 ignored.
  - integration/doctest groups also passed: 13 passed, 6 passed, and 12 ignored
    in the remaining doctest group.
- Disk check:
  - `/`: 30G size, 26G used, 2.9G available, 90%.
  - `/home`: 854G size, 674G used, 137G available, 84%.

Current boundary:

- The local `ploke-eval` proof ladder is internally consistent after the
  changes.
- The next useful proof should be live and focused on child self-evaluation or
  an actual bounded Prototype 1 step that can produce treatment evidence, not
  another local-only channel mechanics test.

## Entry 22: Live Google One-Slot Child-Plan Step

Observe:

- Google auth was refreshed again and direct Google smoke tests passed outside
  the Prototype 1 path.
- The next focused proof was a one-slot live `prototype1-step` from child-plan
  phase, not a long loop.

Orient:

- This test exercises the parent patch-generation path through real
  `prototype1-step`, real direct-Google chat completions, and a real headless
  TUI tool loop.
- The success criterion for this rung is bounded live behavior plus durable
  child-plan evidence. A real admitted child would be stronger, but a rejected
  attempt is still useful if it explains why no child could be admitted.

Decide:

- Run the gated live Google test with protocol/debug output enabled.
- Treat provider success separately from model/task success.

Act:

- `RUSTFLAGS=-Awarnings PLOKE_PROTOCOL_DEBUG=1 cargo test -q -p ploke-eval --features live_api_tests live_google_step_child_plan -- --ignored --nocapture`
  passed in about 60 seconds.
- The direct-Google route made many successful 200 responses.
- The headless TUI attempt timed out at the configured 60-second bound and
  persisted rejected child-plan evidence instead of publishing a runnable child.
- The diagnostic summary showed:
  - `terminal=TimedOut { secs: 60 }`;
  - 62 events;
  - 31 tool requests;
  - 29 completed tools;
  - 2 failed tools;
  - 0 proposals.
- Timing/memory checkpoints showed:
  - Google auth checked at about 233 ms;
  - child-plan world minted and diagnosed by about 241 ms;
  - the step returned at about 60.25 seconds;
  - RSS rose to about 119 MB.
- The model did useful initial exploration, including `list_dir`, `cargo check`,
  `cargo test`, and reads of source/control files.
- The attempt still failed to stage an edit. The high-signal tool failures were:
  - reading `/tmp/.../eval-home/worktrees/prototype1-parent`, which was outside
    configured roots;
  - `cargo check -p ploke-eval`, which is not a package in the candidate target
    workspace.

Current boundary:

- Live Google routing is not the blocker.
- Parent patch-generation concurrency and bounded live execution are proven.
- The next blocker is task/evidence shaping for the live headless TUI patch
  generator: it can spend the whole live budget exploring available artifacts
  and target workspace state without producing a proposal.
- Do not start a long live loop until the patch-generation slot can reliably
  produce either an admissible child or a clearly classified, actionable
  rejection on a focused live step.

## Entry 23: Live Google Child-Plan Admission And Parallel Slots

Observe:

- The first stronger one-slot live test used a seeded canary fixture and direct
  Google through the real `prototype1-step` path.
- The model found the failing `src/lib.rs` function, staged a semantic edit,
  the headless adapter approved and applied it, and the model ran both
  `cargo test` and `cargo check` successfully.
- Initial admission failures were not provider failures:
  - the fixture `Cargo.lock` was non-canonical, so Cargo rewrote it and broad
    policy correctly rejected the protected filename;
  - after fixing that, the fixture was missing tool-description artifact
    files required by artifact-surface measurement.

Orient:

- `Cargo.lock` is intentionally outside
  `WorkspaceExceptPlokeEval`; rewriting it is not an admissible descendant
  patch.
- The missing artifact-surface files exposed a real ordering bug: broad
  admission could commit the candidate and only then fail while building the
  admitted transaction.
- The immediate proof target is parent patch generation, not long-loop
  execution.

Decide:

- Canonicalize the test fixture lockfile instead of weakening broad policy.
- Add the tool-description files the broad artifact surface expects.
- Add a local backend regression that proves artifact-surface input failures do
  not commit the candidate.
- Re-run the focused live one-slot and two-slot proofs.

Act:

- Added `broad_harness_admission_preflights_surface_before_commit`.
  - It removes tool-description artifacts from the source fixture.
  - It submits an otherwise valid README change.
  - It asserts the backend returns `MissingSurfaceFile`, the candidate `HEAD`
    is unchanged, and the original dirty README remains available for review.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_harness_admission_preflights_surface_before_commit -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval --features live_api_tests live_google_step_child_plan --no-run`
  passed.
- `RUSTFLAGS=-Awarnings PLOKE_RUN_LIVE_TESTS=1 cargo test -q -p ploke-eval --features live_api_tests live_google_step_child_plan -- --ignored --nocapture`
  passed in about 50 seconds.
  - One live Gemini slot applied the canary fix.
  - `cargo test` and `cargo check` both resolved to the candidate manifest and
    succeeded.
  - The child-plan advanced to `materialize` with one planned child.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_slots_run_in_parallel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_fanout_is_parallel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings PLOKE_RUN_LIVE_TESTS=1 cargo test -q -p ploke-eval --features live_api_tests live_google_parallel_slots -- --ignored --nocapture`
  passed in about 48 seconds.
  - Both live slots started together and built prompts at about 53-54 ms.
  - Both slots applied the canary fix in separate edit-harness workspaces.
  - Both slots ran successful `cargo test` against their candidate manifests.
  - Both slots were committed and the resulting step advanced to
    `materialize` with `patch_generation_parallel_cap: 2`.

Current boundary:

- The parent patch-generation slot unit can run in parallel under the configured
  cap and still serialize admission into durable child-plan authority.
- The next proof rung should exercise materialize/build for the admitted
  children, including cleanup of build products and preservation of the
  promoted child binary.

## Entry 24: C1-C5 Local Proof Ladder And Handoff Gate

Observe:

- After the live parallel-slot proof passed, the next question was whether the
  downstream parent mechanics were already covered strongly enough to justify a
  bounded fresh run.

Orient:

- The live child-plan proof uses a tiny canary crate, so it proves live
  parent patch generation and admission, but not a real child `ploke-eval`
  binary running treatment evaluation.
- The existing C1-C5 tests use the real parent typestate path and fake the
  child binary/cargo where appropriate. That is enough for parent mechanics,
  but not enough to prove real treatment self-evaluation.

Decide:

- Run the existing parent-path tests in order:
  - materialize/build cleanup;
  - spawn ready and failed-result channel handling;
  - observation rules that forbid projection-only success;
  - current-generation selection trace;
  - successor handoff hydration/rejection tests;
  - the full `ploke-eval` crate gate.

Act:

- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_build_promotes_binary_and_cleans_scratch -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_spawn_observes_ready -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_spawn_observes_failed_result -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval observe_child -- --nocapture`
  passed: 9 tests.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval parent_and_child_have_opposite_directions_from_existing_role_states -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval file_transport_reads_only_new_complete_records -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval current_generation_selector_trace_follows_child_channel_evidence_path -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_cleanup -- --nocapture`
  passed: 3 tests.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval handoff -- --nocapture`
  passed: 6 tests.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval` passed:
  - 763 lib tests passed, 21 ignored;
  - 13 tests passed in the next group;
  - 6 tests passed in the next group;
  - 12 doctests ignored.

Current boundary:

- Parent patch generation, child-plan admission, C1-C5 parent mechanics,
  projection-vs-channel observation rules, selection trace, handoff hydration,
  cleanup, and the `ploke-eval` crate gate are green.
- The remaining proof gap before trusting a long run is the real child
  self-evaluation step: built child binary -> treatment campaign -> eval closure
  -> protocol closure -> terminal channel result with treatment evidence.

## Entry 25: Child Runner Terminal Failure Evidence

Observe:

- The proof ladder still needed coverage for the real child runner function,
  not just parent-side spawn/observe mechanics.
- The child runner path is responsible for `Child<Starting> -> Child<Ready> ->
  Child<Evaluating> -> Child<ResultWritten>` journal projections, attempt and
  latest runner-result files, node status projection, and the terminal
  child-to-parent channel `Result`.

Orient:

- A local test should not fake treatment success. That would only prove the
  wrapper shape.
- The narrowest useful case is an early materialization failure inside
  `run_prototype1_resolved_branch_treatment`: it forces the real runner through
  the child execution path and proves failure evidence remains terminal and
  observable.
- This does not prove successful child self-eval with treatment evidence. It
  proves the child runner does not disappear silently on early treatment
  failure.

Decide:

- Add a focused test using existing production carriers:
  `ChildInvocation`, `Prototype1NodeRecord`, `Prototype1RunnerRequest`, and
  `ResolvedTreatmentBranch`.
- Force failure by leaving the target workspace file absent.
- Assert the durable surfaces that parent-side diagnostics depend on:
  attempt result, latest node result, node status, transition journal states,
  and typed child-to-parent channel messages.

Act:

- Added `child_runner_failure_records_terminal_channel`.
  - It writes an executable `ChildInvocation` bootstrap.
  - It calls `execute_prototype1_runner_invocation` directly.
  - It asserts `TreatmentFailed` / `Failed`.
  - It loads both `nodes/<node>/results/<runtime>.json` and
    `nodes/<node>/runner-result.json`.
  - It asserts the node projection is `Failed`.
  - It parses the child-to-parent channel envelopes and checks:
    `Ready`, `Evaluating`, then terminal `Result { treatment: None }`.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval child_runner_failure_records_terminal_channel -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval` passed:
  - 764 lib tests passed, 21 ignored;
  - 13 tests passed in the next group;
  - 6 tests passed in the next group;
  - 12 doctests ignored.

Current boundary:

- The proof ladder now covers real child-runner terminal failure persistence.
- The remaining child self-eval gap is the successful treatment path that
  carries treatment evidence through the terminal channel and into parent
  comparison.

## Entry 26: Historical Treatment Evidence Rebuild

Observe:

- The node-150 historical channel fixture already proves that parent observation
  can receive terminal treatment evidence from the authoritative child channel.
- That did not prove the child-side evidence builder can reconstruct treatment
  evidence from a treatment `closure-state.json` plus the run `record.json.gz`.
- The full crate gate initially failed in an unrelated broad-batch test because
  a test-only global slot-limit env var could be observed by another test.

Orient:

- The next useful proof rung is not a synthetic treatment object. It is a real
  historical treatment closure state wired to the existing compressed record
  fixture.
- The env-var failure is a test isolation problem around
  `PLOKE_EVAL_BROAD_TUI_SLOT_LIMIT`, not a failure in the new fixture.

Decide:

- Copy the real node-150 treatment closure-state fixture into
  `crates/ploke-eval/src/tests/fixtures/prototype1-node-150-handoff/`.
- Extend the existing historical node-150 handoff test so it calls
  `build_prototype1_treatment_evidence` and compares the rebuilt treatment with
  the terminal channel treatment payload.
- Pin slot-count-sensitive tests under the existing env guard so the
  concurrency probe cannot leak its temporary slot cap into unrelated tests.

Act:

- Added `treatment_closure_state.json` from the historical treatment campaign.
- Extended
  `historical_node_150_channel_treatment_reaches_current_generation_handoff`
  to:
  - load the real `ClosureState`;
  - redirect its record path to the hermetic copied `treatment-record.json.gz`;
  - build a `Prototype1LoopCampaign`;
  - call `build_prototype1_treatment_evidence`;
  - assert campaign id, branch id, instance count, metrics, and status match the
    terminal channel treatment evidence.
- Hardened broad harness slot-count tests against concurrent test-only env
  overrides.
- `cargo fmt --all` passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval historical_node_150_channel_treatment_reaches_current_generation_handoff -- --nocapture`
  passed.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval broad_harness_batch_admits_three_transactions_into_three_children -- --nocapture`
  passed after the env-race hardening.
- `RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval` passed:
  - 764 lib tests passed, 21 ignored;
  - 13 tests passed in the next group;
  - 6 tests passed in the next group;
  - 12 doctests ignored.

Current boundary:

- The proof ladder now covers historical successful treatment evidence rebuild
  from persisted child-side closure state and record data.
- The remaining gap is still a live successful
  `execute_prototype1_runner_invocation` run all the way through treatment eval,
  treatment protocol, patch projection validation, and terminal treatment
  channel result.
