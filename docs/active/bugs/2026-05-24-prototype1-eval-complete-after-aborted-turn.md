# Prototype 1 Eval Complete After Aborted Provider Turn

Status: fixed in `a8e3af717` with regression coverage; the repaired guard was
reverified live on 2026-07-30.

## Summary

Prototype 1 baseline eval previously could export a Multi-SWE-Bench patch and
mark eval closure complete even when the recorded agent turn ended as `aborted`
and produced no final assistant message. This is not admissible transition
evidence for the self-editing loop: protocol should not review a patch
projection that came from an aborted provider turn unless the run records an
explicit partial or failed state.

## Evidence

Observed during abandoned campaign
`p1-gemini35-flash-multigen-fresh-20260524-112232`.

The run was setup-invalid for the intended direct-Google lane because it used
the stale OpenRouter route for `google/gemini-3.5-flash`. During the eval step,
the provider request ended with an OpenRouter HTTP 402 credit/token-limit error.
Do not quote the raw provider body from the logs; it contains sensitive account
details.

Sanitized artifact facts:

- `agent-turn-summary.json` has `terminal_record.outcome = "aborted"`.
- `agent-turn-summary.json` has `final_assistant_message = null`.
- `execution-log.json` still includes `benchmark_turn_completed`,
  `write_msb_submission`, and `write_benchmark_patch_projection`.
- `closure status` reports eval `status = complete`.
- `benchmark-patch-projection.json` reports `check.status = "passed"`.
- The target checkout diff only adds an unused helper
  `replace_with_captures_at_restricted` in `crates/printer/src/util.rs`.
- The original callsite still uses `replace_with_captures_at`.
- `cargo check -p grep-printer` and `cargo test -p grep-printer` pass, but both
  warn that the new helper is dead code.
- `cargo fmt -- --check` passes.

This is a strong counterexample for using patch existence or compile success as
the eval-complete witness.

## Expected Behavior

If the terminal agent turn is aborted, the eval closure state should be failed
or partial, and protocol should not be the next clean transition. Patch export
may still be useful for debugging, but it should not satisfy the state-transition
invariant for a benchmark attempt.

## Reproduction Shape

Use a recorded or fixture-backed eval run with:

- terminal turn outcome `aborted`;
- no final assistant message;
- a non-empty checkout diff or submitted patch.

Assert that closure does not report eval `complete`, or that doctor blocks the
next protocol transition with an explicit artifact-accounting/invalid-transition
reason.

## Source Repair

Commit `a8e3af717` (`Fix aborted baseline closure classification`) made
`crates/ploke-eval/src/closure.rs` classify aborted or timed-out terminal
records as failed rather than treating record existence as completion. Focused
regressions cover aborted records through both direct closure classification
and completed run registration.

## 2026-07-30 Live Guard Reverification

The low-concurrency canary
`p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352` exercised the repaired
boundary with a different external failure class:

| Item | Observation |
| --- | --- |
| Worktree | `/home/brasides/.ploke-eval/setup-seeds/p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352` |
| Walk session | `350350a7-ab97-4ef8-b58b-77470ac1901d` |
| Failed edge | R5-to-R6 operation `eedf5dbe-2fc4-4933-983d-549e397ea71c`, transition `23bbb5e7-a401-5b56-89e1-129ff386c5db` |
| Eval run | `run-1785409245205-structured-current-policy-e8747710` |
| Provider result | Direct Google `google/gemini-2.5-flash-lite`; one attempt; `HTTP_SEND_TIMEOUT` after 39,088 ms; error id `9ebe7d77-ddfb-4fbb-86b0-7c82baf90f6a` |
| Turn result | `outcome = aborted`; `final_assistant_message = null` |
| Closure result | `eval.status = partial`, `failed_total = 1`, `complete_total = 0` |
| Controller result | Baseline instance `BurntSushi__ripgrep-2209` was `Failed`; the edge became `Indeterminate`, retained R5, and was explicitly abandoned |

Durable evidence:

- `/home/brasides/.ploke-eval/instances/prototype1/p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352/BurntSushi__ripgrep-2209/runs/run-1785409245205-structured-current-policy-e8747710/agent-turn-summary.json`
- `/home/brasides/.ploke-eval/campaigns/p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352/closure-state.json`
- `/home/brasides/.ploke-eval/walk/operations/010d47766949fa80/eedf5dbe-2fc4-4933-983d-549e397ea71c.json`
- `/home/brasides/.ploke-eval/campaigns/p1-v31-walkop-handoff-g25fl-3g1x1-20260730-035352/prototype1/control/sessions/2503708aa2ceb4a559a1195b3af9741cc5461d4db4bc8f92ba62e882da08571d/control-journal.jsonl`

Trace chain:

`direct_google request -> HTTP_SEND_TIMEOUT -> terminal turn aborted ->
baseline instance Failed -> closure partial with one failed instance -> guarded
R5-to-R6 attempt indeterminate -> cursor retained at R5`.

The timeout is an external transport failure, not a new source regression. The
important live result is that the repaired aborted-turn guard held. Preserve
this campaign as stop-use evidence; do not reinterpret the failed baseline as
progress or route it into protocol.

## Notes

This bug is separate from the operator setup error that selected the wrong route
source. The route error explains why this concrete run aborted; it does not make
the closure accounting acceptable.
