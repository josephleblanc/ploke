# 2026-04-17 Eval Failure And Protocol Audit Control Plane

- date: 2026-04-17
- task title: eval failure and blind protocol audit
- task description: bounded orchestration pass to catalog current eval failures, reconcile them against known limitations and bugs, run blind CLI-only trace reviews over a random sample of completed protocol runs, and compare those blind reviews against persisted protocol outputs.
- related planning files:
  - [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md)
  - [recent-activity.md](../../workflow/handoffs/recent-activity.md)
  - [2026-04-12_eval-orchestration-protocol.md](../2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md)

## Scope

- Audit the `18` current eval failures in the `rust-baseline-grok4-xai`
  campaign.
- Track whether each failure family is already documented as a known limitation
  or bug.
- Add missing documentation where the evidence supports it.
- Run blind CLI-only review on a random sample drawn from protocol-complete
  runs.
- Compare blind-review judgments to persisted protocol outputs and aggregate
  the resulting agreement/disagreement surface.

## Current State

| task_id | status | owner | layer_workstream | packet_link | latest_report_link | next_action |
|---|---|---|---|---|---|---|
| `AUDIT-F1` | `implemented_self_checked` | orchestrator + workers | `A2` | [failure-audit-packet.md](./failure-audit-packet.md) | [failure-inventory.md](./failure-inventory.md) | use the audited blocker families to drive the next implementation slice |
| `AUDIT-P1` | `implemented_self_checked` | orchestrator + workers | `A4` | [blind-trace-review-packet.md](./blind-trace-review-packet.md) | [blind-trace-sample-summary.md](./blind-trace-sample-summary.md) | use the blind sample to target protocol aggregate calibration and harness recovery work |
| `AUDIT-P2` | `implemented_self_checked` | orchestrator | `A4` | [blind-trace-review-packet.md](./blind-trace-review-packet.md) | [blind-trace-sample-summary.md](./blind-trace-sample-summary.md) | convert the comparison result into implementation packets |
| `AUDIT-IMPL-P0` | `implemented_self_checked` | orchestrator | `A4` | [packet-P0-campaign-protocol-triage.md](./packet-P0-campaign-protocol-triage.md) | [2026-04-17_protocol-triage-implementation-report.md](./2026-04-17_protocol-triage-implementation-report.md) | use the new campaign triage surface to prioritize the next concrete `ploke-tui` tool/harness fix |
| `AUDIT-IMPL-P1` | `ready` | orchestrator | `A4` | [packet-P1-path-recovery.md](./packet-P1-path-recovery.md) | pending | await permission if you want production tool recovery changes outside `crates/ploke-eval/` |
| `AUDIT-IMPL-P2` | `ready` | orchestrator | `A2` | [packet-P2-parser-blockers.md](./packet-P2-parser-blockers.md) | pending | await permission if you want parser/indexing blocker implementation work |
| `AUDIT-IMPL-P3` | `ready` | orchestrator | `A4` | [packet-P3-protocol-calibration.md](./packet-P3-protocol-calibration.md) | pending | can start immediately inside `crates/ploke-eval/` if you want the lowest-friction next slice |
| `AUDIT-IMPL-P4` | `ready` | orchestrator | `A4` | [packet-P4-patch-format-recovery.md](./packet-P4-patch-format-recovery.md) | pending | await permission if you want patch-entry recovery changes outside `crates/ploke-eval/` |

## Notes

- Blind reviewers must not be told that the sampled runs were selected for
  completed protocol coverage.
- Blind reviewers should use the CLI-first trace workflow (`inspect tool-calls`
  and nearby inspect commands), not `protocol` commands or protocol artifacts.
- Known parser limitations belong in `docs/design/syn_parser_known_limitations.md`
  plus `docs/design/known_limitations/`.
- Target/run-policy limitations belong in
  `docs/active/workflow/target-capability-registry.md`.
