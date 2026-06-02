# Cargo Tool Tail Rendering And Timeout Layering

- date: 2026-05-22 local / 2026-05-22 UTC
- status: partially fixed in source checkout; live-loop confirmation pending
- regression marker: `regr:cargotail:22-05-26_14-10`

## Summary

The cargo tool can retain useful stdout/stderr tails but present them poorly to
operators and, in timeout cases, fail to return the useful output to the model.

The concrete `p1-google-live-run-20260521-4` symptom was visible in the
`ploke-egui` inspector for a successful focused `cargo test` against the
ripgrep `crates/regex` manifest. The UI details showed compile-progress stderr
and the oldest retained stdout test lines, while the final useful stdout lines
were omitted from the displayed "tail".

## Evidence

Observed manifest:

```text
/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-4/prototype1/nodes/node-a3d172020d3b81ad/instance-targets/p1-google-live-run-20260521-4-treatment-branch-2c12da797ee15200-1779472199309/BurntSushi/ripgrep/crates/regex/Cargo.toml
```

The inspected result decoded successfully:

```text
ok true
status Success
command Test
scope Focused
exit code 0
duration ms 1567
errors 0
warnings 0
diagnostics 0
truncated false
```

But the UI details labeled as tails showed compile progress first:

```text
Compiling memchr v2.4.1
Compiling log v0.4.14
...
Finished `test` profile ...
Running unittests src/lib.rs ...
Doc-tests grep_regex
```

and stdout stopped before the final test-result summary.

## Root Cause

`crates/ploke-tui/src/tools/cargo.rs::format_details` rendered:

```text
result.stderr_tail.iter().take(20)
result.non_json_stdout_tail.iter().take(20)
```

The underlying vectors already contain bounded retained tails in chronological
order. Taking the first lines of that retained window displays the oldest
retained lines, not the newest. For successful cargo runs, the stderr tail is
also dominated by Cargo progress lines rather than actionable diagnostics.

The model-facing payload is separate: the cargo tool serializes the full
`CargoToolResult` content for the tool message. However, the loop can still
lose useful cargo output if the outer tool-call waiter times out before the
cargo tool's own timeout completes. In headless Prototype 1 this layering is
especially easy to trigger because the chat/tool-loop timeout and cargo command
timeout are configured independently.

## Fix In Source Checkout

The source checkout now:

- renders the newest retained stdout/stderr lines in cargo UI details
- filters pure Cargo progress stderr from successful runs
- keeps non-progress stderr for failures

Regression:

```text
cargo test -p ploke-tui format_details_shows_latest_test_output_and_filters_success_progress -- --nocapture
```

This test pins the `p1-google-live-run-20260521-4` shape: successful cargo test
progress should not dominate the details, and the final test-result stdout line
must be visible.

## Remaining Risk

The timeout layering issue is not fully fixed by the detail-rendering patch.
The next implementation pass should make cargo tool execution use a
tool-specific waiter timeout, or otherwise ensure the outer tool-call timeout is
longer than the cargo command timeout plus kill/reporting grace. A timeout at
the outer waiter layer gives the model a generic tool timeout instead of the
cargo result payload, even if cargo eventually writes useful output.

## Next Live-Loop Confirmation

On the next fresh Google Prototype 1 loop, inspect a successful cargo result and
confirm:

- UI details omit successful compile-progress stderr
- stdout details include the final `test result` line when present
- model-visible tool result content includes cargo stdout/stderr tails when the
  cargo run finishes
- any cargo timeout is emitted as a cargo tool result, not only as an outer
  `Timed out waiting for tool result` event
