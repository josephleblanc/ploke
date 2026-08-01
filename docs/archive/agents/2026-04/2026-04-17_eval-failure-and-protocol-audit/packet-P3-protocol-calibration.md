# 2026-04-17 Packet P3: Protocol Aggregate Calibration

- task_id: `AUDIT-IMPL-P3`
- title: calibrate run-level protocol aggregate labels against blind review
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A4`
- related_hypothesis: the current protocol aggregate over-labels
  `search_thrash` / `mixed` on short or otherwise clean traces
- design_intent: improve operator-facing protocol summaries without changing the
  underlying persisted review artifacts unnecessarily
- scope:
  - inspect how `inspect protocol-overview` derives run-level summaries from
    call-level and segment-level outputs
  - reduce false-positive “mixed/thrash” presentation on short clean traces
  - preserve the useful call-level issue detail already visible in cases like
    `sharkdp__bat-1402`
- non_goals:
  - do not redesign `ploke-protocol` procedures in this slice
  - do not rerun protocols
  - do not hide genuine path/layout detours on the noisier traces
- owned_files:
  - likely inside `crates/ploke-eval/` if kept to aggregate/report surfaces
- dependencies:
  - [blind-trace-sample-summary.md](./blind-trace-sample-summary.md)
  - `/tmp/proto-sample/*.json`-equivalent per-run outputs from
    `inspect protocol-overview --format json`
- acceptance_criteria:
  1. run-level summaries for short clean traces become less misleading
  2. `sharkdp__bat-1402` still surfaces the patch-format failure clearly
  3. the changed aggregate/reporting logic is backed by before/after examples on
     at least the sampled mismatch cases
- required_evidence:
  - before/after render or JSON comparison for:
    - `tokio-rs__tokio-4789`
    - `tokio-rs__tokio-6252`
    - `sharkdp__bat-1402`
  - exact file references for the aggregation logic changed
- report_back_location:
  - this audit directory plus a bounded implementation report
- status: `ready`

## Why This Packet Is Distinct

This slice is analysis-surface work, not a protocol-procedure redesign. It is
the lowest-friction implementation packet because it can likely stay inside
`crates/ploke-eval/`.
