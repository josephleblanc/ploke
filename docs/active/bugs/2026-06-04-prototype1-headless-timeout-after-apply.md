# Bug: Prototype 1 headless TUI can collapse applied post-apply turns into timed-out slots

## Status

Fixed in source on 2026-06-04 with local regression coverage. A direct-Google
live contract canary now confirms that a live applied edit missing
request-declared validation is persisted as `AppliedValidationMissing` and is
not published. A same-target live replay or fresh `prototype1-state` run that
hits the post-apply timeout/abort shape is still needed before calling the live
timeout workflow fully cleared.

This report records the observed cause of the June 2026 `prototype1-state`
parent patch-generation timeouts. It does not make timeout-after-apply slots
admissible. The submitted-result guard still refuses to publish non-admissible
headless results; the fix is upstream terminal classification and
request-declared validation enforcement after an edit has already been applied.

Related earlier reports:

- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
- [`2026-05-25-headless-tui-timeout-submission-admission.md`](./2026-05-25-headless-tui-timeout-submission-admission.md)

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
