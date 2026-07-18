# P1 v21 Strict Keep And Late-Child Timeout Review

Campaign:
`p1-v21-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-162038`

Status: preserved failed live run; controller session abandoned at R10; no
selection commit or successor handoff.

## Verdict

V21 proved that the strict operational, protocol, and all-resolved MBE policy
can produce a genuine Keep from a live generation-one child. It did not prove
R11 selection or successor handoff because the third parallel child finished
after the admitted 1,200-second observer fence.

The correct disposition was to preserve and abandon the run. Rewriting the
late result into the timed-out attempt would have bypassed the controller
fence and invalidated the exact history the handoff is meant to protect.

## Admitted Shape

V21 used dual-strict eval storage, direct Google
`google/gemini-3.5-flash` for eval and protocol, OpenRouter
`mistralai/codestral-embed-2505` for indexing, one ripgrep target, MBE,
all-resolved oracle evidence, full-batch 1x3 children with parallel cap 3,
`require_keep_for_continuation = true`, and a three-generation/ten-node
search budget.

The distinguishing timeout was:

```toml
observe_child_stale_after_secs = 1200
```

## Child Outcomes

- `node-e8d23902813510a3` / `branch-d45e64188a459628`: strict `Keep`.
  MBE resolved, all 276 fix tests passed, tool failures improved 1→0,
  same-file retries 1→0, and maximum retry streak 2→0.
- `node-2412c8602f128419` / `branch-5a14c0135e717316`: strict `Reject`.
  MBE resolved and tool failures improved, but same-file retry count 1→2
  and maximum streak 2→3 regressed.
- `node-c5f4cbe29892e7b4` / `branch-875fff86d72b187c`: runner succeeded only
  after the parent observation fence had timed out.

Malformed protocol JSON was handled by bounded retry and did not exhaust the
run. The decisive failure was controller timing, not provider auth, protocol
closure, benchmark execution, or strict Keep policy.

## Terminal Boundary

The parent fence became indeterminate after approximately 1,200 seconds. The
late child wrote its successful terminal event roughly 114 seconds later, but
the enclosing R10 attempt had no safe reconciliation route. There is no R11
selection entry, History selection block, successor checkout, or runtime
handoff.

`walk recover --abandon-job` can clear the job layer; it cannot prove that a
late child result should be committed into the timed-out controller attempt.
The session was therefore abandoned and the server stopped.

## Follow-Up

V22 raised only the observer timeout to 2,400 seconds and completed the same
three-child fan-in, confirming the configuration mitigation. A separate
historical R10 late-result replay is still needed before controller recovery
can safely consume evidence like v21.

The next live proof must retain the strict policy and 2,400-second observer
budget, obtain a Keep, commit R11/R12, and cross the verified R13b successor
handoff without modifying predecessor evidence.
