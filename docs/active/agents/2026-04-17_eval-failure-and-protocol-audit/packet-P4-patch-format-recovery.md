# 2026-04-17 Packet P4: Patch Format Recovery

- task_id: `AUDIT-IMPL-P4`
- title: improve recoverability around malformed unified diffs
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A4`
- related_hypothesis: malformed-diff failures are already recoverable, but the
  first failure could be made easier to correct without weakening validation
- design_intent: preserve strict patch validation while shortening the recovery
  loop after an invalid diff payload
- scope:
  - improve the error/help surface for malformed unified diff submissions
  - consider adding a minimal template or more explicit shape reminder
- non_goals:
  - do not relax unified-diff validation
  - do not silently rewrite invalid patches into valid ones
  - do not bundle this with unrelated path-recovery changes unless the write set
    is clearly coherent
- owned_files:
  - to be assigned after permission; likely outside `crates/ploke-eval/`
- dependencies:
  - [blind-trace-sample-summary.md](./blind-trace-sample-summary.md)
  - sampled run `sharkdp__bat-1402`
- acceptance_criteria:
  1. the first malformed-diff failure returns a clearer correction surface than
     the current message
  2. validation remains strict
  3. at least one before/after repro demonstrates a shorter recovery path or
     better user-facing guidance
- required_evidence:
  - exact malformed-diff repro
  - before/after error text or structured error shape
  - statement that validation semantics did not change
- report_back_location:
  - this audit directory plus a bounded implementation report
- status: `ready`
