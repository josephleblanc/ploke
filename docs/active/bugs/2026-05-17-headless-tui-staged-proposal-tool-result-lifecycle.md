# 2026-05-17 Headless TUI Staged Proposal Tool-Result Lifecycle Mismatch

## Summary

Headless Prototype 1 TUI runs can report a code-edit tool call as completed when
`ns_patch` has only staged a proposal:

```text
{"ok":true,"staged":1,"applied":0,...}
```

The headless harness then separately accepts, applies, denies, or rejects that
proposal. When denial or apply failure is bridged back as `ToolCallFailed` using
the same `(request_id, call_id)`, observability correctly rejects the terminal
status change:

```text
Cannot change terminal status once recorded
```

The more serious runtime effect is model-facing: the LLM session has already
accepted the staged result as the tool result for the next request. The later
proposal admission result is not guaranteed to reach the model before it
continues tool-calling, so the model can validate, inspect, or keep patching as
if a workspace change happened when no allowed edit was applied.

## Affected Surface

- `crates/ploke-tui/src/tools/mod.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/llm/manager/session.rs`
- `crates/ploke-tui/src/observability.rs`
- `crates/ploke-db/src/observability.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
- Prototype 1 broad-harness headless TUI runs

## Concrete Failure

Observed during live campaign:

```text
p1-selection-metrics-3g1x3-20260517-3
node-a395b777dc618d6a
```

The `r2` headless result timed out after 900 seconds:

```text
prototype1/messages/edit-harness-result/node-a395b777dc618d6a-r2.headless-tui.json
terminal = {"terminal":"timed_out","secs":900}
```

That result contains repeated protected-path attempts against
`crates/ploke-eval/src/successor_selection/metrics.rs`. The event pattern is:

1. `non_semantic_patch` emits `ToolCallCompleted` with `staged:1, applied:0`.
2. the headless adapter classifies the path as protected and sends
   `DenyEdits`.
3. `deny_edits` emits `ToolCallFailed` for the same call id with
   `Edit proposal denied by user`.
4. observability logs the terminal transition warning.

The later `r3` live observation stream shows the model-facing side of the same
bug. The model staged a protected patch to:

```text
crates/ploke-eval/src/operational_metrics.rs
```

at `2026-05-17T08:28:06Z` and received:

```text
{"ok":true,"staged":1,"applied":0,"files":["crates/ploke-eval/src/operational_metrics.rs"],...}
```

The workspace still had no applied edit, but the model continued with reads and
`cargo check`. At `2026-05-17T08:28:48Z`, `cargo check -p ploke-eval`
completed successfully against the unchanged `r3` workspace. As of the last
bounded observation, no `runner-result.json` existed for the node and only the
`r1`/`r2` headless result files had been written.

## Related Recent History

This is adjacent to, but not fixed by, the recent stale-anchor work:

- `e17e65b4 x`
  - added a real `rescan_for_changes(...)` call after successful non-semantic
    apply in `crates/ploke-tui/src/rag/editing.rs`
  - added `docs/active/bugs/2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`
- `526e78a4 Document headless TUI stale anchor retry bug and hyperagents incident`
  - added
    `docs/active/bugs/2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`
- `df77a461 Wait for headless TUI turn completion after apply`
  - delayed terminal return after an applied proposal until `ChatTurnFinished`
- `4af3e2fd prototype1: apply broad harness edits after turn completion`
  - collected staged proposals and applied them after the model turn completed
- `a6264381 prototype1: apply allowed harness edits during turn`
  - changed the adapter to apply or deny allowed/protected proposals during the
    turn

Those changes address stale indexes, same-file stale anchors, and when the
adapter applies proposals. They do not fix the fact that the LLM session treats
the first staging event as the definitive tool-call result before proposal
admission is known.

## Root Cause

`ns_patch` has two lifecycles that currently share one tool-call identity:

1. tool execution stages a proposal;
2. proposal admission/apply later decides whether the workspace changed.

In `crates/ploke-tui/src/tools/mod.rs`, `NsPatch::execute(...)` returns a
successful `ToolResult`, then `emit_completed(...)` emits
`SystemEvent::ToolCallCompleted` for the staging result.

In `crates/ploke-tui/src/llm/manager/session.rs`, the tool dispatcher removes
the waiter for that `call_id` on the first `ToolCallCompleted` and appends that
content as the next `role: tool` message.

In `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`,
the headless adapter observes the same completion event, classifies the staged
proposal, and then either applies it or sends a denial command.

In `crates/ploke-tui/src/rag/editing.rs`, `deny_edits(...)` bridges denial as
`SystemEvent::ToolCallFailed` for the same `call_id`. The DB layer rejects that
terminal status flip by design.

So the persisted warning is not the cause. It is a symptom of a collapsed
boundary: proposal staging, proposal denial/apply, model replay, and durable
tool-call terminal status are all being projected through the same
Completed/Failed tool-call channel.

## Why The Existing Fix Did Not Cover This

The `e17e65b4` rescan fix only runs after a non-semantic proposal applies. It
does not help protected-path denials because no edit is applied and no rescan is
appropriate.

The stale-anchor bug covers later proposals whose file hashes are invalidated
after an earlier applied edit. This bug happens even before that: a staged-only
or denied proposal is already enough to make the model and observability believe
the tool call reached a terminal success/failure boundary.

## Validity Impact

This does not look like silent partial file mutation by `ns_patch`.

It does threaten run usefulness and can threaten candidate scoring if downstream
projections treat tool activity as candidate progress without requiring applied
workspace evidence. In the current operational metrics path, `staged:1,
applied:0` is not counted as an applied patch, but repeated staged/denied edit
calls still look like patch attempts and consume headless runtime budget.

`cargo check` after a staged-only result should not be interpreted as validating
a candidate improvement unless the harness has recorded an applied proposal or
other authoritative workspace-diff evidence.

## Expected Behavior

The headless path needs a single authoritative tool result for model replay:

1. If a proposal is staged but protected, the model-facing result for that tool
   call should be denied/rejected, not `ok:true`.
2. If a proposal is staged and allowed, the model-facing result should be the
   apply result or an explicit non-terminal staging state that prevents further
   tool calls until apply/deny completes.
3. Observability must not record `Completed -> Failed` or `Failed -> Completed`
   for the same `(request_id, call_id)`.
4. Proposal denial/apply should be represented as proposal lifecycle evidence,
   not as a second terminal status for the original tool call, unless the
   original tool call is kept open until admission completes.

## Fix Direction

Possible fixes:

1. Split events structurally:
   - `ToolCallCompleted` means the model-facing tool result is final.
   - `ProposalStaged`, `ProposalApplied`, and `ProposalDenied` are separate
     proposal lifecycle events keyed to `proposal_id`.
2. For headless eval, intercept code-edit tools so the LLM session does not
   replay the staging result. Wait for the adapter admission/apply decision and
   replay that result instead.
3. If interactive TUI still needs `staged` as a successful tool result, add a
   headless-specific gate that disables further model tool calls after staging
   until proposal admission has produced a final model-facing result.
4. Add typed evidence fields to distinguish:
   - `tool_call_completed`
   - `proposal_staged`
   - `proposal_allowed`
   - `proposal_denied`
   - `proposal_applied`
   - `workspace_changed`

## Test Gap

Add a headless adapter regression where the model emits `ns_patch` to a
protected path. The test should assert:

- the proposal is denied;
- the model-facing replay does not contain `ok:true, staged:1` as if the edit
  were useful work;
- observability does not attempt to change terminal status for the same
  `(request_id, call_id)`;
- no validation command after that staged-only result is treated as candidate
  evidence.

