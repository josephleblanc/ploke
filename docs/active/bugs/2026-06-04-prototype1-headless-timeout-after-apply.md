# Bug: Prototype 1 headless TUI can collapse applied post-apply turns into timed-out slots

## Status

Fixed in source on 2026-06-04 with local regression coverage. A direct-Google
live contract canary now confirms that a live applied edit missing
request-declared validation is persisted as `AppliedValidationMissing` and is
not published. A same-target live replay or fresh `prototype1-state` run that
hits the post-apply timeout/abort shape is still needed before calling the live
timeout workflow fully cleared.

Follow-up source fix on 2026-06-04 now makes the headless adapter run
request-declared cargo validations itself after a clean applied edit and cancel
the still-open chat turn before returning `HeadlessTerminal::Applied`. This
addresses the `p1-memfix-0604a` zero-admission shape without admitting
`applied_timed_out` or validation-missing children.

This report records the observed cause of the June 2026 `prototype1-state`
parent patch-generation timeouts. It does not make timeout-after-apply slots
admissible. The submitted-result guard still refuses to publish non-admissible
headless results; the fix is upstream terminal classification and
request-declared validation enforcement after an edit has already been applied.

Related earlier reports:

- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
- [`2026-05-25-headless-tui-timeout-submission-admission.md`](./2026-05-25-headless-tui-timeout-submission-admission.md)

## 2026-06-05 Post-Refactor Hypothesis

Status: hypothesis from the fresh Gemini 3.5 Flash run; not yet reproduced by a
focused test.

Fresh evidence:

```text
campaign = p1-admissionfix-g35flash-p25flash-20260604-221908
parent = node-740a46b2efbdf119
child plan = prototype1/messages/child-plan/node-740a46b2efbdf119.json
children = []
rejected_surface_attempts = 10
error = broad harness admitted 0 child transaction(s), fewer than required minimum 5
```

The old zero-admission persistence bug is not the active failure here: the
rejected-attempt-only child-plan message exists. The downstream admission guard
is also behaving conservatively: no submitted result JSON exists for these
slots, so no child transaction should be admitted.

The current suspect is the refactored headless bridge lifecycle in
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`.
`run_attempt` records applied edits when a completed tool batch is settled, but
it only runs request-declared validation and returns `HeadlessTerminal::Applied`
inside the later `ChatTurnFinished { outcome: "completed" }` branch. If the
model keeps issuing tool calls, stalls, or otherwise never reaches that clean
completed turn boundary after one or more edits have already been applied, the
outer `run_headless_with_model_inner` wall-clock timeout fires and
`timeout_terminal_for_run` classifies the slot as `AppliedTimedOut`.

Line-level boundary in the refactored file:

```text
tui_bridge.rs:121-143   outer 900s timeout wraps the whole attempt loop
tui_bridge.rs:497-562   ToolCallCompleted settles staged batches and extends `applied`
tui_bridge.rs:662-792   ChatTurnFinished is the only path that runs declared validation and returns Applied
tui_bridge.rs:819-826   outer timeout converts any run with applied evidence into AppliedTimedOut
```

Broken contract, restated for the refactored bridge:

```text
A broad headless-TUI slot with applied edits must cross a bounded post-apply
validation/admission boundary owned by the harness. It must not depend on the
model eventually producing a clean completed chat turn after the current tool
batch has already applied candidate edits.
```

The missing regression should exercise the event-loop path, not only the
terminal classifier. It should construct or replay a post-apply turn where the
tool batch has applied at least one allowed edit but no subsequent completed
chat-turn boundary arrives before the global timeout. Expected behavior after a
fix: the harness either runs request-declared validation and publishes an
`Applied` submitted result when validation passes, or returns a typed
non-admission terminal quickly when validation fails or is unsupported. It
should not burn the full slot timeout as `AppliedTimedOut` merely because the
model did not decide to stop.

## 2026-06-05 Refined Source Boundary: Inner Policy Repair Loop

Status: source-level ordering bug identified and removed locally by the user.
The fresh `p1-admissionfix-g35flash-p25flash-20260604-221908` artifacts do not
prove this branch fired, because every sidecar recorded zero `turn` events and
the branch was only reachable inside the `ChatTurnFinished` handler. The source
boundary was still wrong and could explain future or adjacent post-apply stalls.

The removed block lived inside `run_attempt`'s `ChatTurnFinished` arm, before the
adapter checked `outcome != "completed"`, before it checked whether `applied` was
empty, and before request-declared validation/classification. It consumed
`policy_feedbacks`, submitted an in-place repair prompt with `submit_prompt`,
cleared local batch state, replaced `active_parent_id`, and continued inside the
same attempt.

That placement mixed two responsibilities:

- `policy_feedbacks` describe staged edits rejected before application, such as
  empty material edits, protected/out-of-surface paths, or same-batch overlap.
- applied-edit admission is a post-turn harness boundary. Once at least one
  allowed edit has been applied and the chat turn completes, rejected side
  proposals must not outrank request-declared validation of the applied
  candidate.

Broken source contract for that block:

```text
Rejected side proposals may inform an outer retry only when no allowed edit has
been applied. They must not create an inner hidden sub-turn after an allowed edit
has already been applied, and they must not bypass the post-stop validation /
submitted-result admission path.
```

Correct layering:

```text
run_attempt observes one TUI chat/tool session.
run_headless_with_model_inner owns retries between fresh TUI attempts.
```

If a policy rejection leaves no applied edit, `run_attempt` should return an
`AttemptEnd::Retry*` value and let the outer loop rebuild `next_prompt`,
advance the attempt budget, drop the old runtime, and start a fresh session. If
an allowed edit has already been applied, `run_attempt` should continue through
post-turn validation and terminal classification.

After the local removal, the current `ChatTurnFinished` ordering in
`tui_bridge.rs` is:

```text
provider failure checks
outcome != "completed" -> abort/retry handling
applied.is_empty() -> no-edit retry handling
applied non-empty -> request-declared validation -> classify_applied_terminal
```

Missing repro for this specific boundary: construct or replay a completed turn
where one staged proposal is rejected by policy and another allowed proposal is
applied. The expected terminal should validate/classify the applied candidate,
not submit an inner repair prompt or wait for another hidden turn.

## Broken Contract

A broad headless-TUI slot should preserve an authoritative final status for an
applied candidate attempt. Before this fix, an attempt could apply a patch, run
successful cargo validation, then end with an aborted chat turn or continue tool
use until the outer 900-second budget fired. The persisted terminal then became
plain `timed_out`, which lost the important distinction between:

- no edit was ever produced;
- an edit was applied but validation failed;
- an edit was applied and validation passed, but the chat turn aborted;
- an edit was applied and the model kept working until the slot budget expired.

That ambiguity blocked submitted-result publication, left the candidate checkout
dirty, and made parent selection evidence depend on manual joins between
`.headless-tui.json`, candidate git state, validation summaries, and missing
submitted-result JSON.

## June 2026 Evidence

Campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-g31pro-direct-p35flash-20260604-011844
```

Run profile:

```text
model.id = "google/gemini-3.1-pro-preview"
model.route_source = "direct-google"
protocol.model_id = "google/gemini-3.5-flash"
protocol.route_source = "direct-google"
search.children.min = 2
search.children.max = 3
control.parallel_cap = 2
execution.stop_after = "complete"
```

All nine broad edit-harness requests were published at approximately
`2026-06-03 18:42:19` local filesystem time:

```text
prototype1/messages/edit-harness-request/node-5f7331e8c5220d7b.json
prototype1/messages/edit-harness-request/node-5f7331e8c5220d7b-r2.json
...
prototype1/messages/edit-harness-request/node-5f7331e8c5220d7b-r9.json
```

Result sidecars then landed roughly one 900-second timeout apart, except for
the one successful submitted result:

```text
18:57:19 node-5f7331e8c5220d7b.headless-tui.json
19:12:19 node-5f7331e8c5220d7b-r2.headless-tui.json
19:27:19 node-5f7331e8c5220d7b-r3.headless-tui.json
19:43:53 node-5f7331e8c5220d7b-r4.headless-tui.json
19:58:53 node-5f7331e8c5220d7b-r5.headless-tui.json
20:13:53 node-5f7331e8c5220d7b-r6.headless-tui.json
20:28:53 node-5f7331e8c5220d7b-r7.headless-tui.json
20:43:20 node-5f7331e8c5220d7b-r8.headless-tui.json
20:43:20 node-5f7331e8c5220d7b-r8.json
20:58:22 node-5f7331e8c5220d7b-r9.headless-tui.json
```

Observed sidecar summary:

| Slot | Terminal | Applied evidence | Turn outcomes | Validation evidence | Submitted result |
| --- | --- | --- | --- | --- | --- |
| base | `timed_out` | no | none recorded | `cargo check: success` | absent |
| r2 | `timed_out` | no | none recorded | `cargo test: success`, `cargo check -p xtask: success` | absent |
| r3 | `timed_out` | no | none recorded | `cargo check: success` | absent |
| r4 | `timed_out` | yes, `crates/ploke-core/src/tool_descriptions.rs` | `aborted`, `aborted` | `cargo test --profile release: success` | absent |
| r5 | `timed_out` | yes | none recorded | two successes, then failed cargo test entries | absent |
| r6 | `timed_out` | no | none recorded | none recorded | absent |
| r7 | `timed_out` | yes, `crates/ingest/ploke-mbe/src/parse.rs` | none recorded | `cargo check -p ploke-eval: success`, `cargo check -p ploke-mbe: success`, `cargo test -p ploke-transform --benches: success` | absent |
| r8 | `applied` | yes, three applied proposals | `aborted`, then `completed` | mixed failures and later successes | present |
| r9 | `timed_out` | yes, `crates/ploke-protocol/src/procedure.rs` | `aborted` | `cargo check: success`, `cargo test: success` | absent |

The critical contrast is `r8`: it also had an earlier aborted turn, but later
received a `completed` turn, so `HeadlessTerminal::Applied` was recorded and
the submitted-result JSON was written. The timed-out applied slots did not get
that final completed turn before the outer budget expired.

Important caveat: `r8` was adapter-clean, not contract-clean. Its submitted
result exists because the current adapter happy path only requires a completed
turn, at least one applied edit, and no latest failed cargo validation. The
request-declared validation commands were:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

The persisted headless validations for `r8` were instead:

```text
cargo test -p ploke-db-derive        success
cargo test -p ploke-db               tests_failed_or_runtime
cargo test -p ploke-db               tests_failed_or_runtime
cargo check -p ploke-db              success
cargo test -p ploke-db-derive        success
```

The pre-fix terminal gate checked only the latest observed failed cargo
validation. That allowed `r8` to finish as `HeadlessTerminal::Applied` after a
later successful `ploke-db-derive` test, even though the request's declared
`ploke-eval` validation contract was never run. This was a separate but related
admission gap: `r8` should not be treated as the ideal happy path. It is the
contrast case that explains why the timed-out slots failed the old adapter gate,
while also showing that the old successful gate was too weak.

No `*.turn-live/` raw-response directories were present beside these June 2026
sidecars, so this report uses the persisted headless sidecar events rather than
provider response tapes.

## Source Trace

Parent broad patch generation follows this path:

```text
run_broad_headless_tui_attempt
-> tui_adapter::run_headless_with_model_capture_responses
-> tui_adapter::run_headless_with_model_inner
-> run_attempt
-> finish_broad_headless_tui_attempt
```

The headless adapter wraps the whole retry loop in one wall-clock timeout:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:198-303
```

Before the fix, when that outer timeout fired, the terminal became:

```text
HeadlessTerminal::TimedOut { secs: budget.timeout_secs() }
```

The pre-fix per-turn terminal gate was narrower:

```text
crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:736-854
```

On `ChatTurnFinished`, the adapter returned retry failure before considering the
applied-edit terminal path when `outcome != "completed"`. It also retried when
the latest observed cargo validation failed. Only after a completed turn,
non-empty applied list, and no latest failed cargo validation did it return
`HeadlessTerminal::Applied`.

The submitted-result guard rejected timeouts, including timeouts with applied
edits:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1685-1758
```

The `r8` contrast also pointed at the validation gate in
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`, where
the successful applied path consulted only the latest failed cargo validation
before returning `HeadlessTerminal::Applied`. It did not prove that the
request-declared validation commands were run or passed.

That submitted-result guard was intentionally conservative and was not relaxed.
The fixed source adds more precise upstream status and request-declared
validation gating.

## Fix Implemented

Source changes:

- `run_broad_headless_tui_attempt_with_options` now passes
  `BroadHarnessRequest.contract.validation.commands` into
  `tui_adapter::run_headless_with_model_capture_responses`.
- `tui_adapter` now preserves post-apply terminal states as:
  `AppliedTimedOut`, `AppliedTurnAborted`, `AppliedValidationFailed`, and
  `AppliedValidationMissing`.
- Plain `TimedOut` remains the pre-apply/no-applied-evidence terminal.
- `classify_applied_terminal` returns `HeadlessTerminal::Applied` only when the
  latest observed result for each request-declared validation command exists and
  passes. Running a different cargo command no longer makes the slot
  contract-clean.
- `finish_broad_headless_tui_attempt` still publishes submitted results only for
  `HeadlessTerminal::Applied`; the new typed post-apply terminals are rejected
  with proposal-aware diagnostics.

Regression coverage:

- `timeout_after_apply_terminal_preserves_applied_evidence`
- `aborted_turn_after_apply_terminal_preserves_applied_evidence`
- `requested_validation_missing_blocks_applied_terminal`
- `requested_validation_failure_blocks_applied_terminal`
- `requested_validation_passes_applied_terminal`
- `evidence_applied_timeout_terminal_carries_post_apply_state`
- `applied_timed_out_headless_tui_blocks_submitted_result_with_typed_detail`

Verification run:

```text
cargo test -p ploke-eval edit_surface -- --nocapture
123 passed; 0 failed; 10 ignored; 654 filtered out

cargo test -p ploke-eval timed_out_headless_tui_applied_attempt_blocks_submitted_result_for_admission -- --nocapture
1 passed; 0 failed

cargo test -p ploke-eval applied_timed_out_headless_tui_blocks_submitted_result_with_typed_detail -- --nocapture
1 passed; 0 failed

PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID=google/gemini-2.5-flash \
  cargo test -p ploke-eval --features live_api_tests \
  live_google_direct_broad_headless_tui_rejects_applied_edit_missing_declared_validation \
  -- --ignored --nocapture --test-threads=1
1 passed; 0 failed
```

## Existing Regression Anchor

This existing focused test pins the downstream guard:

```text
crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs
timed_out_headless_tui_applied_attempt_blocks_submitted_result_for_admission
```

It constructs a `HeadlessRun` with an applied attempt and
`HeadlessTerminal::TimedOut { secs: 900 }`, then asserts that
`finish_broad_headless_tui_attempt` refuses to write the submitted result while
the candidate workspace still contains an accepted diff.

That legacy test remains a downstream guard. It proves post-timeout admission
rejection, not the upstream adapter behavior that previously produced
`TimedOut` after an applied turn.

## Remaining Validation

The local minimal repro coverage now exercises the adapter behavior before
`finish_broad_headless_tui_attempt`:

1. applied proposal plus outer timeout now yields `AppliedTimedOut`;
2. applied proposal plus aborted turn now yields `AppliedTurnAborted`;
3. applied proposal plus wrong validation command now yields
   `AppliedValidationMissing`;
4. applied proposal plus failed requested validation now yields
   `AppliedValidationFailed`;
5. applied proposal plus successful requested validation yields
   `HeadlessTerminal::Applied`.

If a future raw-provider tape is available, prefer a `run replay self-edit-live`
fixture from the request and raw full-response sidecar. For the June 2026
campaign above, the available durable evidence is the `.headless-tui.json`
sidecar set, not raw response tapes.

Live direct-Google contract validation exists for the request-declared
validation gate:

```text
live_google_direct_broad_headless_tui_rejects_applied_edit_missing_declared_validation
```

That canary synthesizes a small published broad-harness request, forces direct
Google, prompts the live model to call `apply_code_edit`, and intentionally does
not run the request-declared validation commands. It confirms that the live path
persists `AppliedValidationMissing`, includes the missing validation commands in
the rejection, leaves the candidate diff inspectable, and writes no submitted
result.

The remaining timeout-specific validation is live-path confidence for the
original failure shape: rerun a same-target live replay or bounded fresh
`prototype1-state` broad TUI attempt and confirm the sidecar terminal uses
`AppliedTimedOut` or `AppliedTurnAborted` instead of plain `timed_out` when a
patch has already been applied.

## Fresh Validation: `p1-memfix-0604a`

The fresh run rooted at:

```text
/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a
```

confirms the old ambiguous terminal did not recur for applied patches. Ten broad
headless-TUI result sidecars were written under:

```text
prototype1/messages/edit-harness-result/*.headless-tui.json
```

Terminal distribution:

```text
8 applied_timed_out
1 applied_validation_missing
1 timed_out
```

The child plan:

```text
prototype1/messages/child-plan/node-a212c1db6c2774de.json
```

had `children = []` and ten `rejected_surface_attempts`.

This validates the terminal-classification side of the fix: applied candidates
are now persisted as typed post-apply failures with proposal ids and changed
paths instead of collapsing into plain `timed_out`. It also confirms the
admission guard remains strict. The batch failed because no candidate satisfied
the submitted-result contract, not because broad patch generation failed to
produce artifacts.

The remaining live-run blocker is behavioral: expensive headless slots can
continue until the 900-second budget after applying edits, and those candidates
are rightly rejected unless they reach `HeadlessTerminal::Applied` with the
request-declared validation complete. The follow-up fix below makes the adapter
produce that contract-clean terminal itself after an applied edit plus requested
validation. It does not admit
`applied_timed_out` children as successful selections.

## Follow-up Fix: Adapter-Owned Declared Validation

The `p1-memfix-0604a` run showed that typed post-apply terminals were necessary
but not sufficient. The submitted-result gate was not looking for stale files;
it correctly refused non-`Applied` terminals. The remaining bug was that
`run_attempt` treated request-declared validation as a model obligation. If the
model applied a valid patch and then stopped, kept reading, or failed to call the
exact `cargo` tool commands from `BroadHarnessRequest.contract.validation`, the
adapter had no validation observations to classify.

Minimal repro:

```text
recorded_replay_runs_declared_validation_after_applied_edit
```

Pre-fix failure:

```text
Terminal(AppliedValidationMissing { missing: ["cargo check"], ... })
validations = []
```

The fix keeps admission strict and moves only evidence production:

- after a clean applied edit batch, if the request declares validation commands,
  `run_attempt` maps supported `cargo check` and `cargo test` contracts into the
  existing `ploke-tui` `CargoTool`;
- the adapter records the resulting `CargoValidationObservation` entries using
  the same path already used for model-issued cargo tool completions;
- it cancels the open chat turn through the harness cancel channel before
  returning, so the background LLM loop is not left running against a dropped
  state manager;
- `classify_applied_terminal` remains the only gate: failed, unsupported, or
  missing requested validations still prevent `HeadlessTerminal::Applied`.

Verification:

```text
cargo test -p ploke-eval recorded_replay_runs_declared_validation_after_applied_edit -- --nocapture
1 passed

cargo test -p ploke-eval requested_validation -- --nocapture
3 passed

cargo test -p ploke-eval gated_replay_sends_applied_ns_patch_instead_of_staged_success -- --nocapture
1 passed

cargo test -p ploke-eval edit_surface -- --nocapture
127 passed; 0 failed; 10 ignored
```

## Follow-up Fix: Turn-Live Replay Tape Coordinates

The `p1-memfix-0604a` follow-up run also exposed a replay evidence bug in the
turn-live sidecar. Historical r10 has a replayable near-tail patch event:

```text
/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a/prototype1/messages/edit-harness-result/node-a212c1db6c2774de-r10.turn-live/
event 88: non_semantic_patch
call_id: function-call-34662591-b7b8-4c3e-b589-f70d5b7fb2c1
provider response_index: 31
```

The replay resolver can map that `TurnCursor` to the provider response via
`run replay turn-live` semantics, but the raw sidecar written by the old
response-tap drain had duplicate response indices. Each continued chat session
reported a session-local `chain_index` starting at zero, and the headless run
persisted those local indices under one assistant message. That made full-prefix
turn-live replay ambiguous even when the provider responses themselves were
present.

Source fix:

- `HeadlessRun` now owns a monotonic `next_response_index` for the persisted
  turn-live sidecar;
- `drain_response_records` rewrites response-tap records onto that run-level
  sequence before writing `RawFullResponseRecord`s;
- recorded provider responses still flow through the normal `ploke-tui`
  parser/session/tool path. The fix changes replay coordinates, not tool
  results.

Regression coverage:

```text
cargo test -p ploke-eval response_tap_drain_rebases_session_local_indices_to_run_tape -- --nocapture
1 passed

cargo test -p ploke-eval historical_r10_near_tail_turn_live_tape_applies_ns_patch_through_tool_loop -- --nocapture
1 passed
```

The historical replay test does two things: first, it resolves the r10
`TurnCursor` through the turn-live tape to response index 31; second, it takes
that near-tail historical provider response and executes its
`non_semantic_patch` through the current headless TUI tool loop against a
fixture workspace prepared at the pre-call file state. This proves the replay
path exercises current tool behavior rather than injecting historical tool
results.

## Notes For Fix Design

- Do not reintroduce the May 25 invalid-submission bug. A timed-out applied
  slot should not silently write `submitted_result_path` unless admission policy
  explicitly allows it.
- Preserve the candidate diff and validation summary in a parent-readable
  rejected/quarantined result so selection can distinguish no-edit timeouts from
  applied-but-nonterminal slots.
- Enforce or explicitly classify request-declared validation. A slot that runs
  different cargo checks than the `BroadHarnessRequest` asked for should not be
  indistinguishable from a fully validated `Applied` child.
- Consider ending a headless attempt immediately after the first applied edit
  plus successful requested validation, or add an explicit "finalize candidate"
  protocol so the model cannot spend the rest of the 900-second budget on
  unrelated reads after a usable patch exists.
