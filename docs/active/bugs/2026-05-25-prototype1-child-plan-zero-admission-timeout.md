# Prototype 1 Broad-Harness Zero-Admission Child Plan

Status: source fixed; fresh live handoff verification still pending
Discovered: 2026-05-25
Fixed: 2026-05-25

## Summary

The `p1-gemini35-flash-direct-15g2x3-20260525-140904` campaign reached
`child_plan` after a completed baseline eval and protocol pass. The broad
headless-TUI child planner ran a slot for parent node
`node-18f71c7f3b1718b8`, but the slot timed out and admitted zero children.

The visible error was:

```text
broad harness admitted 0 child transaction(s), fewer than required minimum 2
```

That timeout was not the controller bug. The bug was that the below-minimum
broad-harness batch returned before writing any authority-bearing child-plan
evidence. On retry, `resolve_child_plan` saw no child-plan file and treated the
phase as fresh work, so it minted new broad-harness request slots instead of
reusing the failed batch as durable rejected-attempt evidence.

## Broken Contract

Prototype 1 child planning must be idempotent over persisted transition
evidence.

Once a broad-harness batch has been published and executed, a below-minimum
admission result must still be recorded as a child-plan outcome before the step
returns. The record can contain no admitted children, but it must contain
parent-readable rejected surface-attempt evidence for the attempted slots.

Without that record, the next `prototype1-step` or `prototype1-continue` cannot
distinguish "no child plan has run yet" from "a child plan ran and failed to
admit enough children." That makes retry semantics non-idempotent and hides the
true failed transition behind fresh prompt publication.

## Evidence

Campaign:

```text
p1-gemini35-flash-direct-15g2x3-20260525-140904
```

Parent node:

```text
node-18f71c7f3b1718b8
```

Policy:

```text
child_budget.min = 2
child_budget.max = 3
child_schedule_mode = full-batch
```

Historical broad request:

```text
prototype1/messages/edit-harness-request/node-18f71c7f3b1718b8.json
request_id = broad-harness-request:node-18f71c7f3b1718b8
```

Historical headless-TUI summary:

```text
prototype1/messages/edit-harness-result/node-18f71c7f3b1718b8.headless-tui.json
terminal = TimedOut { secs: 240 }
```

Observed retry shape:

```text
doctor phase = child_plan
doctor blockers = []
allowed_actions included continue and step
request files existed for node-18f71c7f3b1718b8 and later retries
no child-plan authority file existed for the failed zero-admission batch
```

## Not The Bug

This is not primarily a provider timeout bug, auth bug, or model-quality bug.
Those can explain why a slot failed to produce an admissible edit.

This report is about the controller contract after that slot failure: the
failed broad-harness batch must be made durable before returning the below-min
error.

## Fix

`publish_broad_harness_child_plan_from_admitted_batch` now persists a
rejected-attempt-only child plan before returning the below-minimum
`InvalidBatchSelection` error. The rejected attempt reason is derived from the
real headless-TUI diagnostics using `tui_adapter::evidence::Summary`, including
the historical `TimedOut { secs: 240 }` terminal.

The persisted rejected plan:

- records rejected evidence for attempted slots that were not admitted;
- excludes any slot that was already admitted, so admitted children are not
  misreported as rejected;
- marks the parent projection failed;
- remains readable by `resolve_child_plan` on retry, so retry does not mint a
  fresh broad-harness batch for the same phase.

The child-plan validator still rejects a completely empty plan. It now permits
an empty admitted-child set only when rejected surface-attempt evidence is
present.

## Regression

Regression test:

```text
cargo test -p ploke-eval zero_admission_batch_is_persisted -- --nocapture
```

The test uses fixture copies of the real historical request and headless-TUI
summary from the campaign above. It reproduces the zero-admission batch with
`min = 2`, proves the batch still returns the below-min error, then calls
`resolve_child_plan` again and verifies:

- no children were admitted;
- rejected surface-attempt evidence was persisted;
- the rejected reason includes `timed out after 240 seconds`;
- the second resolve did not publish additional broad-harness request files.

Before the source fix, the test failed because retrying `resolve_child_plan`
returned the same below-minimum batch-selection error after publishing fresh
slots. That was the actual regression signal.

## Remaining Risk

This source fix proves the controller can persist and reuse failed
zero-admission child-plan evidence. It does not prove a fresh live Prototype 1
run can complete a successful parent/child handoff; that still requires a fresh
run or a stronger historical handoff replay using the relevant parent/child
transition artifacts.
