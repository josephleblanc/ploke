# Prototype 1 Parallel Trace Capture Cross-Talk

Status: session-owned parallel capture is fixed in `dc5868b66` and live
verified with the `24f18e1cd` binary. The companion timeout-cancellation repair
in `24f18e1cd` is source/test verified; a post-repair outer-timeout live canary
remains.

Discovered: 2026-07-15

## Broken Contract

Each parallel broad-headless-TUI attempt must persist its own workspace,
debug-step stream, and full provider-response tape. Concurrent attempts instead
installed process-global response and debug sinks, so the last installer could
receive and relabel evidence produced by its sibling sessions.

## Affected Run

```text
campaign: p1-handoff-drainfix-g35f-orembed-3g1x3-p3-20260715-112133
parent:   node-8d3096af1107cba7
fanout:   three concurrent broad-harness lanes
```

The affected evidence is under:

```text
~/.ploke-eval/campaigns/p1-handoff-drainfix-g35f-orembed-3g1x3-p3-20260715-112133/prototype1/
```

## Evidence Chain

The first debug step in each session contains a distinct focused workspace:

| Session | Prompt workspace | Debug steps |
| --- | --- | ---: |
| `84c25964-93cc-45d0-b47f-5ef9665cd026` | `node-8d3096af1107cba7` | 66 |
| `9571823f-e531-4114-b053-a66cbb6954dc` | `node-8d3096af1107cba7-r2` | 190 |
| `5285fec6-2197-4a6a-b271-583d2f7cce75` | `node-8d3096af1107cba7-r3` | 73 |

However, all three `debug/tool-loop/<session>/session.json` files claim the
`-r2` lane and workspace. The response sidecars are similarly crossed:

| Sidecar lane | Records |
| --- | ---: |
| base | 0 |
| `-r2` | 213 |
| `-r3` | 0 |

Joining `response.response.id` from the per-session debug steps to the `-r2`
sidecar proves that it contains all 66 base responses, 74 of 190 `-r2`
responses, and all 73 `-r3` responses: exactly 213 records. Every sidecar record
also carries the same `assistant_message_id`. This is not a UI projection bug;
the producer persisted sibling evidence through one globally selected sink.

The path is:

```text
parallel Attempt installation
  -> process-global CHAT_DEBUG_SINK / RESPONSE_TAP replacement
  -> sibling ChatSession writes through the most recent globals
  -> debug metadata and full-response sidecars persisted under the wrong lane
```

## Source Boundary And Fix

Commit `dc5868b66` adds `SessionCapture` at the chat-session boundary and threads
it through the existing production path:

```text
Attempt -> AttemptDriver -> captured runtime -> TestRuntime
        -> LlmRequestArgs -> ChatSession
```

The response sender and typed `ChatDebugSink` now belong to that path rather
than to a process-global installation guard. `SessionCapture::default()` keeps
the legacy global fallback for serialized replay and diagnostic callers;
`SessionCapture::new(...)` is explicit, including the important
`new(None, None)` case, and never re-enables the globals. Production broad
attempts use the explicit path.

The first bounded live probe then exposed a second contract: timeout
cancellation moved the caller's `HeadlessRun` into a dropped future and could
leave successfully decoded responses queued after the only drain point. Commit
`24f18e1cd` makes `TuiHarness` borrow the caller's run, carries assistant
identity in the explicit response record, and drains until every capture sender
disconnects after runtime cancellation. A capture that does not quiesce fails
explicitly instead of publishing incomplete evidence. See the companion
timeout report for that distinct failure chain.

The authority-bearing changes are in:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/attempt.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/driver.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`
- `crates/ploke-eval/src/runner/mod.rs`
- `crates/ploke-tui/src/llm/manager/mod.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`

No persisted trace schema changed.

## Docs And Operator Expectation

The operator-control plan requires the UI and CLI to inspect the same exact
trace evidence and names replacement of parallel singleton trace sinks as the
third item in the current work packet. Cross-lane evidence would make model,
tool-call, and protocol review attribution untrustworthy even when the loop's
typestate transitions themselves were correct.

## Current Repro Coverage

- `concurrent_chat_sessions_keep_capture_provenance` runs two production chat
  sessions concurrently with distinct existing typed response channels and
  debug sinks, then requires strict left/left and right/right provenance.
- `explicit_empty_capture_disables_legacy_fallback` protects the distinction
  between legacy fallback and explicitly disabled capture.
- `attempt_capture_responses_keeps_tap_installed_across_run` exercises the real
  `Attempt` propagation path.
- `cancelling_run_attempt_preserves_caller_run_evidence` is the fail-before
  regression for cancellation losing the caller's partial run.
- `late_response_is_drained` requires cancellation cleanup to retain a response
  sent after draining begins.
- `cargo test -p ploke-tui --lib` passed 309 tests with 10 ignored.
- `cargo test -p ploke-eval --lib -- --test-threads=1` passed 1,327 tests with
  28 ignored.

## Live Validation

A clean three-lane canary was admitted at `dc5868b66`:

```text
campaign: p1-tracecapture-g35f-orembed-3g1x3-p3-20260715-210447
phase:    baseline_eval
```

Direct Google protocol preflight and headless sparse/BM25 setup both pass. The
production embedding preflight stops before eval with OpenRouter HTTP 403,
classified `provider_account`, because the inherited key has exceeded its
monthly limit. The campaign has no invalid eval or transition evidence and may
resume after the credential/account limit is corrected.

That full-loop canary remains a valid end-to-end lifecycle check, but standalone
production broad-harness requests can validate capture without changing its
admitted campaign. Three bounded sweeps were preserved rather than rewriting a
failed lane into a clean one:

1. The first 180-second sweep under `dc5868b66` produced three correctly
   isolated debug sessions and 28 globally unique response IDs, but all three
   attempts timed out with empty response tapes. This isolated the companion
   cancellation bug fixed in `24f18e1cd`.
2. The first post-fix r13-r15 sweep was stopped after `/home` reached zero free
   bytes and the debug sink reported `No space left on device` while writing
   session `360fcc4c-416c-45f2-996b-31e2393dc453`. Its 402,773-byte resume
   record and zero-byte r15 manifest remain preserved; r13 has a zero-byte
   headless file, while r14 retained partial headless and turn-live evidence.
   The r13 source edit is also preserved. No submitted result JSON was
   published, and those lanes are abandoned as invalid full-run evidence.
3. After cleaning only stopped build outputs, untouched r16-r18 ran with the
   committed binary, Direct Google `google/gemini-3.5-flash`, one attempt,
   600-second bounds, and parallelism three.

The accepted live evidence is:

| Lane | Session | Terminal outcome | Debug steps | Tape records |
| --- | --- | --- | ---: | ---: |
| r16 | `0cf7c223-7b45-4e6e-81bb-5078b6fa00b8` | applied validation failed | 46 | 46 |
| r17 | `b548822d-0966-4cd8-8125-595de4ac8132` | provider response deserialization failed; exhausted | 9 | 9 |
| r18 | `596aebf9-a621-4d66-9282-7dfe069e6754` | applied validation failed | 39 | 39 |

For every lane, the session manifest names the exact lane and workspace, every
debug step carries that session ID, response indexes are contiguous from zero,
the ordered debug response IDs exactly equal the ordered response-tape IDs, and
the tape's one assistant ID equals its resume record. All three pairwise
response-ID intersections are empty. The failed validations and provider
response were correctly rejected, so no submitted broad-harness result JSON was
published; the headless and turn-live evidence bundles remain available for
inspection.

This closes the parallel production capture contract. It does not live-verify
child-runner reaping or unblock the separate full-loop OpenRouter embedding
canary.

## Residual Scope

`REQUEST_TAP` and the recorded-response source used by replay probes remain
process-global compatibility surfaces. This repair establishes concurrent
production response/debug ownership; it does not claim that arbitrary
concurrent executable replays are isolated. That is a separate contract and
must not be inferred from this fix.

## Non-Fixes

- Do not serialize production child fanout merely to protect trace capture.
- Do not weaken provenance validation or merge sidecars after the fact.
- Do not switch this canary to direct OpenAI as an implicit workaround: that
  route still lacks the approved per-snippet truncation policy and has already
  failed on this benchmark's 66,807-byte parsed node.

## Related Bugs

- [`2026-07-13-prototype1-embedding-preflight-after-admission.md`](./2026-07-13-prototype1-embedding-preflight-after-admission.md)
- [`2026-07-14-direct-openai-embedding-overlong-snippet.md`](./2026-07-14-direct-openai-embedding-overlong-snippet.md)
- [`2026-07-15-prototype1-child-runner-processes-not-reaped.md`](./2026-07-15-prototype1-child-runner-processes-not-reaped.md)
- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
