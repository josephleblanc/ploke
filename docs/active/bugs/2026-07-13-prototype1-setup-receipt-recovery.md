# Prototype 1 setup could not resume its own durable admission receipt

Status: fixed in source; original live recovery verified, hardened regression pending

Discovered: 2026-07-13

## Broken Contract

`prototype1-setup` writes an `Admitting` receipt before campaign, profile,
closure, database, scheduler, branch, identity, or commit effects. Repeating the
exact previewed setup must load that receipt, reconcile already-completed
effects, finish missing effects, and then make completed retries read-only.

The first implementation violated that contract at three recovery seams:

1. `Option::unwrap_or` eagerly evaluated `capture_setup_admission`, so a loaded
   matching receipt still attempted fresh capture and rejected its own campaign
   artifacts as unreceipted state.
2. closure validation compared rich campaign slice metadata to the canonical
   file-derived registry projection, even though production closure
   recomputation intentionally normalizes a generated slice to `label =
   "slice"` with no source URL;
3. bootstrap checkout reconciliation rejected a clean, tracked parent identity
   inherited from the exact receipt-bound base commit instead of replacing it
   on the new parent branch.

None of these failures indicated corrupt persisted state. They prevented the
receipt from performing the recovery it was created to authorize.

## Evidence Chain

The live campaign was admitted from preview SHA:

```text
campaign: p1-stage1-receipt-g35f-pplxembed-3g1x3-p3-20260713-1
plan:     a4de23644b0090482420966f82812dd95db32e2488c6f12b1ebc43c567e3b962
parent:   node-81e3097807ae52b3
base:     697c7468a1a8895ccfb3a9ff87009385e088529b
```

The first attempt persisted the receipt and stopped at closure validation. The
next exact attempt proved the eager fallback defect by reporting that
`campaign.json` existed without a receipt while the receipt remained present
and byte-readable. After that fix, reconciliation created the owner database,
root node, request, and scheduler, switched to the bootstrap branch, and
stopped at the inherited old parent identity. The final retry reused the
original receipt carriers and completed at Git head:

```text
d2a5e8f146e94427dd7932091b0c7597d2c9e652
```

An immediate exact completed retry preserved all of the following:

- receipt SHA-256 and mtime;
- scheduler SHA-256 and mtime;
- owner Cozo file SHA-256 and mtime;
- Git HEAD;
- node ID, plan SHA, profile commitment, and completion receipt.

## Source Boundary

```text
prototype1-setup
-> admit_prototype1_parent_setup
-> load_matching_admission
-> reconcile campaign/profile/closure/owner DB/root node/checkout
-> replace_setup_admission(Admitting -> Complete)
```

Closure verification now derives the same read-only registry source projection
as `ResolvedCampaignConfig::closure_recompute_request` instead of requiring the
closure projection to retain manifest-only label and URL metadata.

Checkout reconciliation replaces a mismatching inherited identity only when it
is clean at the receipt-bound base HEAD. A dirty mismatching identity still
fails closed. A dirty identity that exactly matches the receipt remains an
admissible interrupted-write recovery.

## Regression Coverage

`prototype1_setup_recovers_and_completed_retry_is_read_only` exercises the
production setup entrypoint against a temporary Git repository whose seed
commit contains an older parent identity. It asserts that setup replaces and
commits the new identity, completes its receipt, and that an exact retry leaves
the complete eval-home tree and Git HEAD unchanged.

The strengthened implementation also binds the reviewed source branch and HEAD
into the version-2 plan digest, serializes admission under a stable lock in the
Git worktree's administrative directory, writes small authority-bearing files
by atomic replacement with directory fsync, and verifies every completed
artifact (including root node/request/scheduler, identity, branch, parent
commit, and completion HEAD) on an exact retry. Re-previewing an admitted setup
uses the receipt's reviewed checkout only when all newly resolved plan intent
still matches, so recovery remains reachable after the checkout has switched
without letting the receipt hide command/profile drift.

The regression now begins from a real partially persisted `Admitting` state,
first proves that a meaningful unreceipted entry still fails closed, then
simulates a crash before the receipt's no-clobber publication by leaving its
exact atomic staging file. Under the worktree lock, retry removes only the
numeric PID/sequence staging remnant, tolerates the otherwise empty receipt
directory, and publishes the receipt. The same test then simulates a second
crash immediately after the bootstrap branch switch, re-resolves the expected
plan through the public setup service, lets the production entrypoint recover
it, proves an exact completed retry is read-only, and proves branch drift, a
missing scheduler, and receipt-field drift fail closed.

Final source verification passes the focused durability and setup-recovery
checks plus the complete `ploke-eval` library suite: 1007 passed, 0 failed, 27
ignored. An independent pre-commit review reports no remaining blocker.

A fresh live setup/no-write retry will be recorded after the hardened slice is
committed; the earlier live plan digest cannot be reused because checkout
identity is now part of the reviewed plan.

The original live campaign remains at `baseline_eval`; it was not advanced
because the separate live embedding readiness probe is blocked by the
OpenRouter key's monthly limit.
