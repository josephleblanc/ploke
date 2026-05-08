# 2026-04-17 Packet P2: Parser And Indexing Blocker RCA

- task_id: `AUDIT-IMPL-P2`
- title: root-cause the active parser/indexing blocker families
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A2`
- related_hypothesis: current eval closure is blocked less by missing runs than
  by three recurring parser/indexing failure families
- design_intent: turn the audited blocker clusters into minimal defendable RCAs
  and bounded implementation slices
- scope:
  - isolate `generic_lifetime` relation failures
  - isolate duplicate `crate::commands` module-path collisions
  - determine whether the current `nushell` `indexing_completed` timeouts are
    pure scaling symptoms or a masked lower-level failure
- non_goals:
  - do not collapse all three families into one mixed “nushell is bad” bucket
  - do not change parser semantics speculatively without a defended RCA
  - do not weaken validation or import semantics
- owned_files:
  - to be assigned after permission; likely outside `crates/ploke-eval/`
- dependencies:
  - [failure-inventory.md](./failure-inventory.md)
  - [2026-04-17-generic-lifetime-transform-failure.md](../../bugs/2026-04-17-generic-lifetime-transform-failure.md)
  - [2026-04-17-nushell-duplicate-commands-module-path.md](../../bugs/2026-04-17-nushell-duplicate-commands-module-path.md)
  - [2026-04-17-nushell-indexing-completed-timeout.md](../../bugs/2026-04-17-nushell-indexing-completed-timeout.md)
- acceptance_criteria:
  1. each blocker family has a defended primary failure location or narrowed
     subsystem owner
  2. each family yields either a bounded implementation slice or a stronger
     documented limitation with clear re-entry conditions
  3. timeout rows are separated cleanly from parser/transform failures rather
     than treated as generic leftovers
- required_evidence:
  - concrete artifact paths and minimal repro or checkpoint evidence per family
  - explicit mapping from failure family to proposed owning subsystem
  - if implementation is proposed, exact file ownership and why it is the
    smallest correct slice
- report_back_location:
  - this audit directory plus bug-note updates
- status: `ready`

## Current Family Counts

- duplicate `crate::commands`: `4`
- `generic_lifetime`: `6`
- `indexing_completed` timeout: `6`
