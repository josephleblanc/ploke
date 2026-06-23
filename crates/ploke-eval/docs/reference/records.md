# Records

`ploke-eval` writes several record families. They are not equally authoritative.

| Record family | Typical files | Trust role |
| --- | --- | --- |
| Run manifest | `instances/<id>/run.json` | Prepared task contract for one instance. |
| Attempt records | `instances/<id>/runs/run-*/record.json.gz` | Per-attempt run evidence and replay input. |
| Execution log | `execution-log.json` | High-level local run telemetry. |
| Repo/index/snapshot status | `repo-state.json`, `indexing-status.json`, `snapshot-status.json` | Setup evidence and diagnostics. |
| Submission artifact | `multi-swe-bench-submission.jsonl` | Benchmark candidate patch export. |
| Batch summary | `batch-run-summary.json` | Aggregate convenience projection. |
| Campaign manifest | `campaign.json` | Campaign configuration anchor. |
| Closure state | `closure-state.json` | Campaign progress state used by campaign export. |
| Prototype 1 run profile | `prototype1/run-profile.toml` plus commitment | Admitted operator policy. |
| Prototype 1 transition journal | `transition-journal.jsonl` | Append-only replay/debug evidence. |
| Prototype 1 History | `history/blocks/*` and indexes | Sealed handoff authority substrate. |
| Prototype 1 channels | `nodes/*/channels/*/*.jsonl` | Runtime lifecycle and child/successor evidence. |
| Prototype 1 projections | `scheduler.json`, `branches.json`, node JSON | Operational projections, not independent authority. |

## Practical rule

Before using a record to justify a transition, ask:

1. Was it admitted by the profile or typed transition path?
2. Is it sealed History, channel evidence, or only a projection?
3. Is it attempt-scoped, campaign-scoped, or checkout-carried?

If the answer is unclear, start with read-only diagnosis and source anchors
instead of editing records by hand.
