# Prototype 1 Eval Complete After Aborted Provider Turn

Status: open

## Summary

Prototype 1 baseline eval can export a Multi-SWE-Bench patch and mark eval
closure complete even when the recorded agent turn ended as `aborted` and
produced no final assistant message. This is not admissible transition evidence
for the self-editing loop: protocol should not review a patch projection that
came from an aborted provider turn unless the run records an explicit partial
or failed state.

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

## Notes

This bug is separate from the operator setup error that selected the wrong route
source. The route error explains why this concrete run aborted; it does not make
the closure accounting acceptable.
