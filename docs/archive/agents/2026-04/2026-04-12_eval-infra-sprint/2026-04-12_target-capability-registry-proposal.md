# Target Capability Registry Proposal

- Date: 2026-04-12
- Status: proposed
- Layer/workstream: `A2` / `A4`
- Related planning docs: [eval-design.md](../../plans/evals/eval-design.md), [failure-taxonomy.md](../../workflow/failure-taxonomy.md), [hypothesis-registry.md](../../workflow/hypothesis-registry.md)

## Problem

Known target limitations are currently easy to lose in chat or prose:

- parser blockers that abort ingest entirely
- modeling coverage gaps that make some graph-based failures expected
- scaling constraints that make a target operationally impractical

Without a structured registry, we risk:

- burning tokens on unfair or known-impossible runs
- misattributing failures to tools or agents when the target is not actually in
  scope for fair graph-based evaluation
- losing the ability to track when a new parser/modeling feature brings a
  target back into scope

## Proposal

Add a target capability registry to the workflow layer:

1. Durable schema/rules doc:
   [docs/workflow/target-capability-registry.md](../../../workflow/target-capability-registry.md)
2. Living registry:
   [docs/active/workflow/target-capability-registry.md](../../workflow/target-capability-registry.md)

The registry should track:

- limitation class:
  `parser_blocker`, `modeling_coverage_gap`, `scaling_constraint`
- interpretability flag:
  `graph_valid`, `graph_degraded`, `graph_invalid`, `performance_restricted`
- run policy:
  `default_run`, `skip_by_default`, `subset_only`, `run_only_for_feature`,
  `allow_non_semantic_fallback`, `special_run_only`

## Why This Fits The Existing Eval Design

This proposal belongs in the measurement-validity layer, not as an incidental
parser note.

- `eval-design.md` already separates outcome metrics from validity/health
  metrics.
- Phase 1 and Phase 2 depend on fair attribution.
- The failure taxonomy says what happened during a run.
- The target capability registry says what was already known about whether the
  target was fair or informative to run.

That separation makes it easier to:

- exclude unfair targets from default graph-based comparisons
- keep hard cases available as explicit re-entry probes
- measure progress when parser/modeling work changes the target capability state

## Recommended Next Integration Steps

1. Add the registry to the active workflow entry points so it is visible on cold
   restart.
2. When a parser/modeling/scaling limitation changes target fairness, update the
   registry before scheduling more formal runs.
3. When a target moves from blocked/degraded to potentially usable, schedule an
   explicit re-entry packet or run instead of silently assuming the problem is
   gone.
4. If target/task annotations become operationally important for run scheduling,
   add a small packet to connect the registry to the benchmark/task selection
   workflow.

Current example:

- `BurntSushi__ripgrep` now represents the "resolved blocker with sentinel value"
  case: the mixed-edition parser blocker was real, the fix landed, the target
  has already been exercised successfully afterward, and future reruns should be
  treated as regression spot-checks rather than as proof that the target is
  still out of scope.
