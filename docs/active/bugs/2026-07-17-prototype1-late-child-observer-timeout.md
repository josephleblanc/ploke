# Prototype 1 Late-Child Observer Timeout

Status: config-mitigated for the next run; controller-level R10 recovery remains
open.

Discovered: 2026-07-17

## Broken Contract

An admitted R10 child fanout must reach one durable parent outcome. If a child
remains healthy past `observe_child_stale_after_secs` and then produces a valid
terminal result, the controller needs an evidence-backed way to reconcile that
late result. It must not guess, discard the result, duplicate the child, or
weaken the attempt/session fences.

The timeout is still a valid safety boundary. A repair must distinguish a
healthy late completion from an abandoned or contradictory attempt using the
persisted child receipt and channel evidence.

## Preserved Incident

Campaign:

```text
p1-v21-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-20260717-162038
```

The profile admitted three parallel generation-one children with:

```toml
observe_child_stale_after_secs = 1200
```

Two children were observed before the fence. The parent observation attempt
then timed out after approximately 1,200 seconds. The final child,
`node-c5f4cbe29892e7b4` / `branch-875fff86d72b187c`, remained healthy and
persisted a successful runner result roughly 114 seconds after that parent
timeout.

The transition journal therefore contains the terminal child event but no
matching final `observe_child.after` entry for that lane. The walk operation
remained `AttemptIndeterminate`; R10 did not commit to R11 and no selection
receipt was written.

The controller job and session were explicitly abandoned. V21 remains
preserved; its journal, child results, profile, database, hashes, and workspace
evidence were not modified to make it resumable.

## Useful Evidence Preserved Before The Timeout

The other two children completed full operational, protocol, and MBE
evaluation:

- `node-e8d23902813510a3` / `branch-d45e64188a459628` was a strict `Keep`.
  It resolved the benchmark, passed all 276 fix tests, reduced tool failures
  from 1 to 0, same-file retries from 1 to 0, and the maximum retry streak
  from 2 to 0.
- `node-2412c8602f128419` / `branch-5a14c0135e717316` resolved the benchmark
  but was a strict `Reject` because same-file retry count and streak regressed.

This proves the strict selection inputs can produce a valid Keep. It does not
prove selection or successor handoff because the parent fan-in never reached
R11.

## Source Boundary

Child-level recovery already has persisted-outcome and channel-reconciliation
paths. The missing boundary is the enclosing R10 controller attempt after its
durable fence has become indeterminate. Generic session abandonment clears
authority; it does not prove that a late child belongs to the timed-out
attempt, nor does it safely commit the missing parent observation.

A source repair therefore needs a historical replay of the exact v21 timing
and evidence chain through the production R10 recovery path. Making the
observer silently wait forever, accepting any late file, or relaxing the
attempt fence would weaken the operational invariant.

## Current Mitigation

V22 increased only the admitted observer timeout to 2,400 seconds. Its three
children completed and R10 committed to R11 after approximately 21 minutes 53
seconds. This confirms that 1,200 seconds was too short for the observed live
workload and that the profile change is a valid immediate mitigation.

The 2,400-second value should remain in fresh strict profiles until the
controller-level recovery contract has a real historical regression.

## Missing Regression

No checked-in test currently replays:

1. two children observed within the fence;
2. one healthy child completing after the fence;
3. a parent attempt becoming indeterminate;
4. restart/reconstruction from the exact stored child/channel evidence; and
5. either one idempotent R11 commit or a precise fail-closed blocker.

Until that exists, preserve v21 as the canonical incident and do not present
`recover --abandon-job` as late-result reconciliation.
