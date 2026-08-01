# Walk Until Target Claims Again After Committed Handoff

Status: source repaired; focused and exact historical coverage pass; serial
`ploke-eval` suite passes; fresh multi-generation live validation pending.

Current contract update (2026-07-31): the historical R12-to-R13b bounded
request described below is no longer admitted. R13b is one of several possible
R12 outcomes and therefore does not post-dominate R12. The operator now takes
one bare Step with the checkout grant and follows its exact typed receipt. The
target-equality repair remains required for convergent bounded targets.

Discovered: 2026-07-16

Incident campaign:

```text
p1-v12-r12fix-keeponly-g35f-oropenai-3g1x3-p3-20260716-204252
```

## Broken Contract

A bounded `walk step --until <phase>` operation is complete when the controller
commits the requested target and releases that exact attempt. It must return the
version from the committed edge without acquiring another controller claim.

This is especially important for R12-to-R13b. That edge transfers authority to
the successor runtime. A predecessor process must not try to claim the
successor's controller session merely to discover that the requested target was
already reached.

The strict successor executable, checkout, History, profile, and digest checks
remain required. A mismatch must continue to fail closed.

Step mode also must not admit R13b or R14b as a bounded target from R12.
R12-to-R13b releases predecessor authority and transfers the shared walk
service to the successor. Continuous mode can retain the predecessor lease
long enough to finalize R14b; the step-mode service currently has no equivalent
predecessor-finalization authority after transfer.

## Persisted Evidence

The v12 predecessor validly completed R12-to-R13b at controller fence 18:

- predecessor session `d9d5ddbc-1f71-4a73-900a-d2d3d34a5bf7` reached R13b at
  journal revision 61;
- transition `d1b66192-ee2b-51cb-afcb-109bd6a0a7ec` committed and released;
- successor runtime `fae4c6a5-71f9-4d6d-bb3a-d4c7f0611c93` claimed its exact
  transferred authority and published a typed R4c Ready receipt;
- successor session `b87a4fa1-fe1e-4a61-9bb5-cfd6b5a50aa9` reached revision 7;
- the transition journal persisted the exact Ready/attempt handoff acceptance;
  and
- the sealed History block and installed successor Artifact remain intact.

After those effects committed, outer operation
`ee7bae91-f6a9-4e49-8121-27b6ac0aef5c` made a second controller claim from the
predecessor executable. The strict claim check rejected it because:

```text
successor executable '/home/brasides/code/ploke/target/debug/ploke-eval'
does not match transferred binary
'/home/brasides/.ploke-eval/setup-seeds/p1-v12-r12fix-keeponly-g35f-oropenai-3g1x3-p3-20260716-204252/target/debug/ploke-eval'
```

The operation therefore became `Indeterminate` with `phase_before = r12` and
`phase_after = r13b`, while its admitted version still named R12 revision 57.
This classification correctly preserved the evidence rather than inventing a
receipt after an error.

The exact persisted artifacts are checked in as hex-encoded fixtures under
`crates/ploke-eval/src/tests/fixtures/prototype1-v12-target-reached-handoff-20260716/`.
The journal values below are SHA-256 digests of the inflated JSONL bytes; the
invocation and operation values are digests of their decoded JSON bytes:

- predecessor controller journal:
  `933132a4076b666c7b947741631d5dc9dca6f1757b7c492bb1726f69b724dcf2`;
- successor controller journal:
  `ff5030308ebef6e89baee9313ef2b9eac3558da3c746b97abe79914673cdd3f5`;
- transition journal:
  `8e2bf04f1506f2fe400892692b1b9a212bf1c6edcb0e98b230a12ed9c7d639d0`;
- successor invocation:
  `2779a6cdd2f929a1f33d1ee300cde0176e3ca74766f705003081a8580831336e`;
  and
- predecessor operation:
  `d0d473aedf274fba24e371bf77234a7d082f71ca42efd66a806bf6a37a63d5e8`.

## Source Boundary

`WalkController::advance_until` previously performed this sequence:

1. call `step_toward(target)`;
2. append the committed edge and update the returned version;
3. check only for overshoot or the wrong branch; and
4. loop and call `step_toward(target)` again.

The second call entered the no-op `current == target` path. That path still
claims the active session so it can return a guarded durable version. At a
handoff boundary, however, the active parent identity and session now belong to
the successor. `Claim::from_successor` therefore rejected the predecessor
executable exactly as designed.

The defect is confined to the bounded controller loop. It is not in History
verification, successor transfer, claim validation, or executable identity.

## Repair

After a committed edge passes the existing wrong-branch and overshoot checks,
`advance_until` now breaks immediately when `self.phase() == target`. The
operation returns the version already produced by the committed/released edge.

The step-mode target resolver also rejects R14b before any edge executes. The
later post-dominator preflight rejects R13b as a bound from R12 before any edge
claim as well; an operator admits one bare Step and inspects whether the typed
receipt realized R13a, R13b, or R13c. The transferred successor endpoint begins
its own durable session at R4c; it cannot be used to claim or finalize the
predecessor's R14b cursor.

No validation is weakened, no persisted run is rewritten, and no synthetic
handoff receipt is manufactured. A direct no-op request that starts at its
target still uses the existing guarded claim path; only a request that reaches
its target during the current bounded operation avoids the redundant claim.

## Regression Coverage

The focused controller regression extends the production fresh-setup path. It
first requests R14b from R3 and requires rejection with no controller-journal
change. Before the boundary repair the request durably advanced through R4a to
R5, then failed only because live-provider authority was absent. It then
advances R3-to-R4a and requires exactly four new journal records: Acquired,
AttemptBegan, AttemptFinished, and Released. Before the target-equality repair
the controller made a second no-op claim and appended six records, so the
assertion failed at revision 9 instead of the expected revision 7.

The historical R12 replay also requires a continuable handoff to admit R13b but
reject R14b through the production target resolver before any owner DB,
transition journal, History, or checkout mutation.

`v12_handoff_operation_replays_target_reached_before_second_claim_failure`
replays the exact predecessor and successor controller journals through
`Store`, the exact invocation through the production authority loader, and the
exact transition journal through `PrototypeJournal`. It requires the committed
R13b handoff and R4c Ready acceptance to precede the preserved executable
mismatch in the indeterminate outer operation. This is incident-evidence replay;
the focused controller and R12 target tests gate the source behavior.

Fix-after evidence:

```text
fresh_setup_reconstruction_claims_first_walk_session_at_r3: passed
r12_reject_replay: passed
v12_handoff_operation_replays_target_reached_before_second_claim_failure: passed
r13b_next_output_explains_step_authority_boundary: passed
prototype1_transition_inventory_covers_source_edges: passed
prototype1_transition_inventory_generated_doc_matches_source: passed
cargo test -p ploke-eval -- --test-threads=1: 1,375 passed, 0 failed, 45 ignored
```

## Remaining Validation Plan

1. Commit the source repair and fixtures without user-owned worktree changes.
2. Prepare a fresh strict campaign from that commit; do not recover or mutate
   v12.
3. Drive multiple generations through the walk service and require each
   R12-to-R13b operation to publish a terminal receipt before endpoint transfer.
4. Confirm the successor endpoint owns the shared walk source of truth and can
   continue from R4c without a predecessor executable mismatch.

## Run Disposition

The v12 run is preserved as historical evidence. It must not be retried through
an altered checkout, rewritten digest, relaxed claim, or permissive History
path. Fresh live validation uses a new campaign and new admitted commitments.

## Related Documents

- [`2026-07-15-prototype1-successor-retirement-before-walk-receipt.md`](./2026-07-15-prototype1-successor-retirement-before-walk-receipt.md)
- [`2026-07-16-prototype1-r12-policy-stop-routing.md`](./2026-07-16-prototype1-r12-policy-stop-routing.md)
- [`../agents/2026-07-13_prototype1-loop-operator-control-plan.md`](../agents/2026-07-13_prototype1-loop-operator-control-plan.md)
