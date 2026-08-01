# Prototype 1 Request-Code-Context No-Progress Loop

Status: source repaired with synthetic, session-level, and checked-in historical
replay coverage. Full crate gates and a fresh multi-generation live run remain
pending.

Discovered: 2026-07-16

## Broken Contract

A broad Prototype 1 edit attempt must return control to the outer attempt
driver when a model repeatedly calls one tool without changing strategy. A
technically successful `request_code_context` result is not progress by itself,
and the shared 500-step tool-chain budget is too broad to distinguish useful
multi-tool work from a single-tool retrieval loop.

The guard must not cancel an in-flight tool batch. The response, every tool
result in the threshold-containing batch, final request messages, and terminal
debug step must settle before the session returns.

## Evidence Chain

The abandoned campaign was:

```text
p1-v10-multigen-g35f-oropenai-3g1x3-p3-20260716-011330
```

R2 debug session `30dff829-27f6-45fe-b88c-a9e60f29818d` persisted 83 provider
steps without an edit or terminal result. Steps 29 through 82 were 54
consecutive `request_code_context` calls, mostly variants around
`sanitize_tool_args`. By step 82 the lane had accumulated about 325,000 prompt
tokens.

The admitted profile gave the broad lane 1,800 seconds, so the forced stop
occurred before its configured outer deadline. The existing session chain cap
was 500 and therefore did not activate.

Successful historical evidence rules out lowering the shared chain cap:

- successful R10 used 166 total tool calls and a maximum same-tool streak of
  10 (`list_dir`); its maximum `request_code_context` streak was 6;
- successful R3 had a maximum `request_code_context` streak of 4;
- the failed R2 streak was 54.

The broad-only limit of 15 preserves observed successful behavior with margin
while stopping the reproduced failure.

## Source Boundary

The relevant production path is:

```text
tui_adapter::start_runtime
  -> ChatPolicy.tool_streak_limit = Some(15)
  -> run_chat_session
  -> validate provider-declared ToolName calls in order
  -> execute and append every result in the containing batch
  -> persist terminal ChatDebugStep and final_messages
  -> SessionOutcome::Exhausted / TOOL_STREAK_LIMIT
  -> outer broad attempt driver
```

Ordinary TUI sessions, debug stepping, and benchmark baseline sessions retain
`tool_streak_limit = None`. The shared 500-step chain cap remains unchanged.

The first observer-side prototype was rejected because broadcast observation
and `cancel_chat` could race tool execution, start a sixteenth provider request,
or interrupt the threshold batch before its debug evidence was durable.

## Docs And Policy Expectation

The outer broad attempt budget remains authoritative. An inner liveness guard
may stop one session only when it returns a truthful non-success terminal; it
must not admit a child, silently discard a tool result, or weaken edit-surface
validation.

`None` means the streak policy is disabled. When enabled, calls are counted in
provider-declared order across responses in one session. A different typed tool
name resets the streak. Crossing the threshold latches the stop, but the whole
containing batch settles before return.

## Current Repro Coverage

The checked-in historical tape is:

```text
tests/fixtures/prototype1/r2-context-streak-20260716/llm-full-responses.jsonl
sha256 48edf0fff044a2374c4e496248ed1264ee58b2ab381055e191829333000c4606
```

It contains the 20 typed `RawFullResponseRecord` envelopes from historical R2
steps 29 through 48, rebased to contiguous response indices 0 through 19. No
response body or tool call was synthesized.

Coverage:

- `historical_r2_context_loop_hits_tool_streak_guard` loads the checked-in tape
  through the production replay loader and broad headless runtime. It requires
  exactly 15 requested and 15 settled `request_code_context` calls, with the
  terminal turn after settlement and no sixteenth provider request.
- `xfail_broad_headless_caps_provider_steps` exercises the same production path
  with repeated protected `non_semantic_patch` calls.
- `tool_streak_limit_settles_threshold_batch_before_exhaustion` proves a
  multi-call threshold batch fully settles and leaves the sentinel next
  provider response unconsumed.
- policy units cover disabled defaults, reset behavior, threshold latching,
  validation clamping, and TOML round-trip behavior.

## Missing Repro Or Validation

- The fresh multi-generation live campaign has not yet exercised the repaired
  guard.
- The outer headless terminal currently preserves the typed code, tool, count,
  and limit in its summary string; a future operator-schema change should carry
  the session `LoopError` structurally for direct UI/database queries.
- This guard bounds R2 but does not explain why retrieval omitted the explicitly
  seeded source file. That retrieval-quality issue remains separate.
- A threshold batch that applies an edit should receive a focused adapter
  regression proving it remains `AppliedTurnAborted` rather than generic
  no-progress evidence.

## Fix Direction

Implemented: keep the general 500-step cap, enable an optional 15-call streak
limit only for broad edit attempts, enforce it synchronously in
`run_chat_session`, settle first, then emit the typed model-behavior error and
return before another provider request.

Do not restore observer cancellation, lower the general chain cap, or classify
the stopped attempt as success.

## Related Bugs

- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
