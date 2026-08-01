# Prototype 1 Reconstruction Rechecks Node Cap After Child Planning

Status: source repaired and locally validated; V28 preserved as stop-use evidence;
fresh-run validation pending.

## Broken Contract

Durable reconstruction of an already-recorded R6 -> R7 -> R8 sequence must replay
the policy and child-plan evidence that was valid when it was admitted; it must
not apply the pre-planning `max_total_nodes` check to the post-planning node
count and reject its own recorded transition.

## Evidence

- Campaign:
  `p1-v28-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260718-123722`
- Parent checkout:
  `/home/brasides/.ploke-eval/setup-seeds/p1-v28-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260718-123722`
- Parent/runtime:
  `node-e508c41e6dbf21fe`,
  `8b6a82f6-dfa7-4509-8ec7-42dca59cdfba`, generation 2.
- Walk session:
  `b6de4536-1ff1-42d1-8141-1a26f12b11b5`.
- R7 -> R8 job:
  `5cdcf18c-1329-4c11-ab06-6de75ddabb84` succeeded at journal revision 23
  with cursor evidence
  `2cfbf872e6703547ae5e4148be1339601ca2b2c50b5f12ddf0a86d381211871e`.
- Failing R8 -> R10 job:
  `02550399-971a-4772-8bbd-9be5575a8b37`.
- Failure:
  `controller cursor requires r8, but durable reconstruction reached r6`.
- `walk show` identifies the inner cause:
  `prototype1 hard stop before child planning: persisted node count 10 has reached max_total_nodes 10`.
- The durable child plan exists at
  `prototype1/messages/child-plan/node-e508c41e6dbf21fe.json` and records
  three generation-3 children:
  `node-4339e5434ed748f6`, `node-ccd74f432ffa8d3e`, and
  `node-eb0c2be240771819`.
- The controller journal at
  `prototype1/control/sessions/2c64bff5ed852d813f528ca6b10bf616a015be94b05b258aa4ca99ceac45b323/control-journal.jsonl`
  contains successful `r6_to_r7` and `r7_to_r8` finished entries.
- The generation began with seven persisted nodes. Planning three children was
  legal and brought the total to the exact configured cap of ten.

The persisted child plan, node records, and controller journal agree. This is
valid transition evidence; the blocker is a read-side replay error.

## Source Trace

`walk step --until r10`
-> controller exact reconstruction
-> `driver/reconstruct.rs::reconstruct`
-> `cli_facing.rs::resolve_parent_policy_budget`
-> `persisted_prototype1_node_count`
-> `reserve_complete_child_budget`
-> post-plan count `10 >= max_total_nodes 10`
-> reconstruction stops at R6
-> controller correctly enters `recovery_required` instead of advancing from a
cursor it cannot reproduce.

The downstream controller guard is correct. The upstream error is that
reconstruction uses the live pre-planning admission helper after the child plan
has already materialized its nodes.

## Docs/Policy Expectation

The run profile requires `max_total_nodes = 10`; that cap must remain strict for
new child planning. The operator policy also forbids rebuilding or changing the
source epoch inside a live historical run merely to bypass digest and handoff
checks. V28 therefore remains immutable evidence and will not use
`walk recover --admit-epoch`.

## Repro Coverage

An ignored historical regression now replays the exact preserved V28 R8
boundary through production `reconstruct_handoff_at`. It failed before the
repair because reconstruction stopped at R6 with the post-plan node-cap
blocker; after the repair it reconstructs exact R8 and consumes the persisted
child-plan evidence.

A focused synthetic regression verifies that the replay count excludes
materialized children from later generations. Existing live budget tests still
verify that new planning reserves only the remaining slots and rejects both a
below-minimum remainder and a count already at the cap.

## Validation

Passed locally:

1. exact V28 historical replay: 1 passed;
2. generation-scoped replay count regression: 1 passed;
3. complete child-budget regressions: 3 passed;
4. reconstruction module: 16 passed, with the artifact-gated historical test
   ignored in the ordinary suite.

Still required: a fresh strict campaign must fill the node cap through a legal
child plan and continue to its configured terminal state.

## Fix Direction

`resolve_parent_policy_budget` remains strict for the live R6 -> R7 edge. At the
reconstruction boundary, the implementation first validates the existing child
plan, then derives the historical pre-plan count from typed,
generation-scoped node records and reserves the same budget that was available
before this parent's children were materialized.

Do not:

- raise or ignore `max_total_nodes`;
- make malformed child plans acceptable;
- reinterpret missing journal evidence as success;
- modify V28's checkout, source epoch, digests, or run artifacts.

## Related Bugs

- [`2026-07-08-prototype1-max-generation-handoff.md`](./2026-07-08-prototype1-max-generation-handoff.md)
  covers terminal continuation at `max_generations`, not replay at
  `max_total_nodes`.
- [`2026-07-17-prototype1-handoff-replaced-executable-epoch.md`](./2026-07-17-prototype1-handoff-replaced-executable-epoch.md)
  records why replacing an active executable epoch is not an acceptable
  recovery shortcut.
