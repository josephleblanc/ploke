# Prototype 1 Headless Unpersisted Workspace Mutation

Status: open. The exposing campaign was abandoned without reconstructing the
missing response or child-plan state. The next live run must avoid forced
process interruption; a durable intent boundary and historical crash replay
remain to be implemented.

Discovered: 2026-07-16

## Broken Contract

An effectful headless tool call must not mutate a candidate workspace beyond
the last durable tool-loop frontier without leaving enough evidence to classify
that state on recovery. Either provider response and tool intent must become
durable before the effect starts, or recovery must prove that the workspace is
still equal to the settled frontier before allowing resume.

## Evidence Chain

The affected lane was R1 in campaign:

```text
p1-v10-multigen-g35f-oropenai-3g1x3-p3-20260716-011330
session f2720143-3f57-4f0d-93c6-071ed77befa0
```

The session persisted 62 settled steps numbered 0 through 61. Step 61 failed
canonical validation and did not mutate the workspace. Its durable timestamp
was approximately `07:43:27.515` local time, and the resume cursor advanced at
approximately `07:43:27.538`.

`crates/ploke-db/src/database.rs` changed at approximately `07:43:40.340`, but
there is no step 62, terminal debug step, or headless result describing that
mutation. The final diff implements the replacement attempted in step 61,
which is strong evidence of a corrected retry, but the exact provider response
and tool arguments are gone and must not be reconstructed as fact.

The server was later stopped and the R7 controller/session were explicitly
abandoned. No R8 receipt, child plan, or successor state was fabricated.

## Source Boundary

The current ordering is:

```text
run_chat_session receives provider response in memory
  -> execute_tools_via_event_bus
  -> ToolCallRequested
  -> apply_semantic_edit
  -> write_snippets_batch mutates filesystem
  -> rescan_for_changes
  -> ToolCallCompleted
  -> record_chat_debug_step
  -> ToolLoopDebugSink::persist settled step/resume
```

An interruption after the filesystem write but before rescan and
`record_chat_debug_step` leaves a dirty workspace with no durable provider
intent or settled step. The runtime database is in-memory, and response capture
does not become durable until the enclosing headless run publishes its result,
so neither closes this crash gap.

Moving `ToolCallCompleted` before rescan is not a valid fix: it would weaken the
existing invariant that a settled edit includes refreshed semantic state.

## Docs And Policy Expectation

Tool-loop steps are settled evidence. Recovery must never infer an absent
provider response from a matching final diff, and it must not declare a dirty
candidate resumable merely because the last persisted step was non-mutating.

Candidate workspaces are isolated lanes, so a typed timed-out or indeterminate
lane may fail without corrupting the parent. It must not publish an admitted
child transaction.

## Current Repro Coverage

Existing timeout/cancellation regressions prove that caller-owned headless
events and decoded response capture survive outer cancellation. They do not
cover interruption between an effectful filesystem write and settled debug-step
persistence.

The real R1 artifacts prove the frontier mismatch but cannot replay the missing
step 62 because its response does not exist.

## Missing Repro Or Validation

A faithful historical regression should:

1. load the real R1 session, resume frontier, step 61, base revision, and final
   workspace witness;
2. replay a real earlier R1 response that exercises the same semantic
   apply/write/rescan path;
3. pause at a deterministic test barrier after filesystem write or on rescan
   entry;
4. interrupt there and require recovery to classify the workspace as beyond the
   settled frontier, not cleanly paused or resumable.

The exact absent step 62 must not be replaced with a synthetic response.

## Fix Direction

Persist an explicit in-flight provider-response/tool-intent record before
effectful execution, then finalize the existing settled `ToolLoopStep` after
tool completion and rescan. Keep the settled step schema semantically strict.
Recovery should compare the current workspace against the durable before/after
frontier and refuse resume when effects exceed it. A captured-eval persistence
failure should stop the run rather than remain warning-only.

Until that work lands, do not manually kill a live broad lane during tool
execution. Let the configured outer deadline return a failed lane, preserve its
evidence, and refuse admission if mutation settlement is indeterminate.

## Related Bugs

- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
