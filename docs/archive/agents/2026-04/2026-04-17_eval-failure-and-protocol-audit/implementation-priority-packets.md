# 2026-04-17 Implementation Priority Packets

- date: 2026-04-17
- task title: post-audit implementation packet list
- task description: prioritized follow-up packets derived from the failed-run
  audit and blind protocol-validation sample
- related planning files:
  - [failure-inventory.md](./failure-inventory.md)
  - [blind-trace-sample-summary.md](./blind-trace-sample-summary.md)
  - [control-plane.md](./control-plane.md)

## Priority Order

| priority | packet | focus | why now | permission note |
|---|---|---|---|---|
| `P0` | [packet-P0-campaign-protocol-triage.md](./packet-P0-campaign-protocol-triage.md) | campaign-scoped protocol triage dashboard and family drilldown | implemented on 2026-04-17; this is now the operator entrypoint for finding the next harness/tool fix | no extra permission required for the delivered `crates/ploke-eval/` aggregate/report work |
| `P1` | [packet-P1-path-recovery.md](./packet-P1-path-recovery.md) | missing-file and repo-root/path recovery | this is the dominant issue in the blind sample and likely highest leverage for agent-harness quality | touches production tool behavior outside `crates/ploke-eval/`; requires your permission before implementation |
| `P2` | [packet-P2-parser-blockers.md](./packet-P2-parser-blockers.md) | `generic_lifetime`, duplicate `crate::commands`, and timeout RCA | these six-plus-four-plus-six failures are the main remaining eval-closure blockers | likely touches crates outside `crates/ploke-eval/`; requires your permission before implementation |
| `P3` | [packet-P3-protocol-calibration.md](./packet-P3-protocol-calibration.md) | run-level protocol aggregate calibration | the blind sample suggests current run summaries over-label short clean traces | can likely start in `crates/ploke-eval/` without extra permission if kept to analysis/aggregate surfaces |
| `P4` | [packet-P4-patch-format-recovery.md](./packet-P4-patch-format-recovery.md) | malformed unified-diff recovery | smaller than `P1`, but clearly exposed by `sharkdp__bat-1402` and likely cheap to improve | likely touches production tool behavior outside `crates/ploke-eval/`; requires your permission before implementation |

## Suggested Execution Order

1. Start from `P0`’s delivered dashboard if you want to choose the next
   concrete harness/tool fix from campaign data.
2. Start with `P3` if you want a narrower no-permission-needed slice inside
   `crates/ploke-eval/` that improves analysis fidelity within the existing
   aggregate shape.
3. Start with `P1` if you want the biggest likely end-user impact on current
   agent traces and are ready to authorize production changes outside
   `crates/ploke-eval/`.
4. Keep `P2` as the main eval-closure unblocker packet once we switch from
   harness behavior back to parser/indexing infrastructure.
5. Fold `P4` into `P1` only if the write set stays coherent; otherwise keep it
   separate because the patch-entry surface is a different failure mode from
   path recovery.
