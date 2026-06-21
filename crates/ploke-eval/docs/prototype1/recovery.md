# Recovery

Prototype 1 recovery should preserve correctness boundaries. Do not make the
system silently tolerate missing authority, schema drift, or projection mismatch
just to get unstuck.

## Recovery order

1. **Diagnose read-only.** Run `prototype1-doctor` and `walk summary/replay`.
2. **Identify the authority family.** Is the problem in History, child channels,
   the admitted profile, or a projection?
3. **Prefer replay/reconstruction over manual edits.** Use typed/replay paths
   when available.
4. **Regenerate projections rather than editing authority.** Scheduler, branch,
   node, and report files are often projections.
5. **Stop before weakening invariants.** If the fix would accept missing or
   malformed authority, ask for explicit approval.

## Safe recovery examples

| Situation | Safer action |
| --- | --- |
| CLI table looks stale | Re-run read-only summary/doctor; check source evidence. |
| Node projection disagrees with channel | Treat channel as lifecycle evidence; inspect reconstruction path. |
| Large JSONL suspected corrupt | Query narrowly; preserve original file before any tool rewrite. |
| Provider route failed | Fix profile/provider preference; do not rewrite campaign route fields to look symmetric. |
| Child worktree build failed | Re-run the diagnosed build phase after confirming worktree and binary provenance. |

## Dangerous recovery anti-patterns

- Editing `history/blocks/*` by hand.
- Adding missing `provider_slug` for direct Google just to satisfy symmetry.
- Marking a child terminal from scheduler status without channel/result evidence.
- Treating CLI output as authority over admitted profile or sealed History.
- Reusing a binary from a different checkout for live parent execution.
- Relaxing child-plan validation to accept malformed or wrong-parent files.

## Escalation checklist

Before changing code or authority-bearing files, record:

- campaign id;
- active parent checkout path;
- current doctor phase/blocker;
- relevant profile route/model/provider tuple;
- exact missing or mismatched file;
- whether sealed History, transition journal, child channel, or projection is involved.
