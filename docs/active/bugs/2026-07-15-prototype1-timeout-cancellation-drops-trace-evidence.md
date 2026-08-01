# Prototype 1 Timeout Cancellation Drops Trace Evidence

Status: source repaired in `24f18e1cd` with a fail-before regression and full
library gates. A fresh live three-lane sweep verifies terminal response
persistence for provider and validation failures; the outer-timeout path itself
has not yet been re-exercised live after the repair.

Discovered: 2026-07-15

## Broken Contract

A bounded broad-headless-TUI attempt must retain every `HeadlessRun` event and
every successfully decoded provider response produced before cancellation. The
caller-owned run cannot become empty merely because the attempt future is
dropped, and response capture must either drain to sender disconnection or
surface an explicit failure.

## Evidence Chain

The first live concurrency probe after session-owned capture landed used three
standalone lanes with a 180-second outer timeout:

| Lane | Debug session | Debug steps | Response-tape bytes |
| --- | --- | ---: | ---: |
| `node-a4d20a11d03fcee5-r4` | `8493be96-a46a-46db-83f0-0a7e05b86a3d` | 9 | 0 |
| `node-a4d20a11d03fcee5-r5` | `2c2d1767-a61f-4490-bc86-cc4e95217180` | 14 | 0 |
| `node-593bba99f06efb0b-r2` | `0ccb7c9d-ea14-4978-a38f-1990efbfb666` | 5 | 0 |

All three debug manifests named the correct lane and workspace. Their 28
provider response IDs were globally unique with no pairwise intersection, so
the earlier cross-talk defect was no longer present. All three outer attempts
timed out, however, and all three full-response tapes were empty. The caller's
run evidence had also been replaced by an empty `HeadlessRun`.

The production path explained both losses:

```text
run_attempt
  -> replace caller HeadlessRun with HeadlessRun::new()
  -> move original run into the timed attempt future
  -> outer timeout drops future
  -> caller retains only replacement run

response capture
  -> drain on ChatTurnFinished
  -> cancellation can bypass that event
  -> detached request task may send after a one-shot drain
  -> queued or late responses are lost
```

## Source Boundary And Fix

Commit `24f18e1cd` keeps the production boundary fail-closed:

- `TuiHarness<'run>` borrows the caller's `&mut HeadlessRun`; cancellation no
  longer destroys mutations already made to the run.
- Explicit session capture carries `FullResponseTraceRecord`, including the
  assistant message identity, instead of erasing it before the eval boundary.
- `AttemptDriver` cancels the runtime, drops its own capture sender, and drains
  responses until every sender disconnects, bounded by two seconds.
- A capture that does not quiesce becomes
  `ToolFailed("response capture did not quiesce after runtime cancellation")`;
  the attempt cannot silently publish incomplete evidence.
- Response indexes are rebased only at the persisted lane boundary. Response
  and assistant identities remain unchanged.

The authority-bearing changes are in:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/driver.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/tui.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`

No persisted trace schema was loosened.

## Repro And Verification Coverage

- `cancelling_run_attempt_preserves_caller_run_evidence` failed before the
  implementation because its sentinel event disappeared, then passed with the
  borrowed-run repair.
- `late_response_is_drained` covers a response arriving after cancellation has
  begun and requires the drain to wait for sender disconnection.
- The response identity/rebase regression requires original response and
  assistant IDs with only a contiguous lane-local index rewrite.
- `cargo test -p ploke-tui --lib` passed 309 tests with 10 ignored.
- `cargo test -p ploke-eval --lib -- --test-threads=1` passed 1,327 tests with
  28 ignored.

## Live Verification Scope

The fresh r16-r18 sweep under `24f18e1cd` exercised three lane-local terminal
outcomes spanning two distinct failure classes through the same production
response-persistence boundary:

| Lane | Outcome | Debug steps | Tape records | Ordered ID diff |
| --- | --- | ---: | ---: | ---: |
| r16 | applied validation failed | 46 | 46 | 0 |
| r17 | provider response deserialization failed, attempt exhausted | 9 | 9 | 0 |
| r18 | applied validation failed | 39 | 39 | 0 |

Every tape index is contiguous, every tape carries exactly one matching
assistant ID, and pairwise response-ID intersections are empty. This proves
live terminal persistence for decoded responses across failure outcomes. It
does not substitute for a fresh post-repair outer-timeout canary, which remains
the narrow missing live check.

The r17 provider failure also demonstrates why the operator projection must
join layers explicitly: its headless bundle records the aborted turn and
`RESPONSE_DESERIALIZATION_FAILED` body excerpt, while its debug session remains
`paused` at the last successfully decoded response. Neither source should be
rewritten to pretend it contains the other layer's fact.

## Residual Scope

- The two-second quiescence-expiry branch has no focused regression of its own.
- The fail-before cancellation regression exercises the production attempt
  entrypoint but does not reproduce an entire spawned live runtime teardown.
- A bounded post-repair live timeout remains necessary before calling the
  timeout path live-verified.

## Non-Fixes

- Do not lengthen or remove the outer timeout to hide evidence loss.
- Do not treat a missing response tape as acceptable merely because debug steps
  exist elsewhere.
- Do not detach response tasks or accept a partial drain silently.
- Do not serialize parallel production fanout as a capture workaround.

## Related Bugs

- [`2026-07-15-prototype1-parallel-trace-capture-cross-talk.md`](./2026-07-15-prototype1-parallel-trace-capture-cross-talk.md)
- [`2026-05-22-cargo-tool-tail-rendering-and-timeout.md`](./2026-05-22-cargo-tool-tail-rendering-and-timeout.md)
