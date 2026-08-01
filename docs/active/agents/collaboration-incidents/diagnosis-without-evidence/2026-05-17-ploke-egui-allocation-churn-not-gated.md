# 2026-05-17 Ploke Egui Allocation Churn Not Gated

- Trigger:
  User reviewed the new native allocation benchmark output and pointed out that
  the UI is churning allocations rapidly, which is unacceptable for the intended
  reference-based `ploke-egui` architecture. The user asked to add allocation
  checking to the performance skill so this does not disappear between turns.
- User-visible failure:
  The benchmark work made allocation data available, but the workflow still did
  not force agents to treat allocation churn itself as a first-class failure.
  Reporting risked emphasizing "suite completed" and frame timings while the
  steady per-frame allocation counts and bytes clearly showed a deeper hot-path
  design problem.
- Touched code surface:
  - `.codex/skills/ploke-egui-benchmarking/SKILL.md`
  - `crates/ploke-egui/src/allocation.rs`
  - `crates/ploke-egui/src/benchmark.rs`
  - `crates/ploke-egui/src/ui/**`
  - `crates/ploke-egui/docs/profiling/benchmarks/`
- What the agent did:
  - added span/group attribution and cheap allocator counters, but did not
    immediately harden the reusable benchmark workflow around allocation debt
  - treated callsite absence as an expected standard-mode limitation without
    also making the available churn/group data a required review gate
  - left room for future agents to compare only frame timing or "no regression"
    against a bad allocation baseline
- Skipped docs / skills / instructions:
  - the `ploke-egui` typed projection rule that live UI paths should preserve
    borrowed graph-derived witnesses instead of drifting into owned DTO copies
  - the user's repeated reference-based performance direction around avoiding
    transitive allocations in UI render paths
  - the benchmarking skill lacked concrete allocation-report requirements and
    absolute allocation-debt tripwires
- Why the behavior was risky:
  `ploke-egui` can look responsive while still allocating hundreds of thousands
  of objects across a 300-frame scenario. If that churn is normalized as the
  baseline, agents may keep adding owned strings, vectors, row carriers, JSON
  projections, and render-boundary caches without an invalidation model. That
  undermines the intended borrowed `Graph` projection architecture and makes
  later performance fixes harder.
- Concrete prevention rule:
  Every relevant `ploke-egui` benchmark report and final answer must summarize
  allocation count/frame, allocated bytes/frame, live bytes, heap slope, top
  allocation groups, and callsite-attribution status. Treat steady-state
  allocation churn above explicit tripwires as allocation debt even when the
  change does not regress against the nearest report. Hot-path UI edits must
  explain how borrowed references, cache keys, or invalidation boundaries avoid
  transitive per-frame allocation.
- Memory hypothesis:
  Memory should bias future agents to inspect allocation churn before accepting
  a `ploke-egui` performance change, and to treat the current high-churn native
  reports as debt to pay down rather than acceptable baselines.
