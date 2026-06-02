# Target Capability Registry

This document defines the durable structure for tracking benchmark-target and
task-level limitations that affect eval interpretability, run policy, and
future re-entry decisions.

Use the living registry in `docs/active/workflow/target-capability-registry.md`
for current target annotations. Use this file for the stable taxonomy, field
definitions, and update rules.

## Purpose

The registry exists to keep known target limitations from being lost in prose or
misattributed to the wrong layer of the eval programme.

It answers questions such as:

- which targets are currently out of scope for graph-based evals?
- which targets are only partially in scope because known semantic regions are
  missing from the graph?
- which targets are valid only under subset or special-run execution policy?
- which targets should be revisited after a parser or macro-handling change?

## Registry Levels

Track limitations at two levels:

1. **Target level**
   The repository, benchmark instance, or reusable eval target as a whole.
2. **Task level**
   A specific issue, benchmark task, or bounded task family within a target when
   only part of the target is degraded.

Target-level entries should be the default. Add task-level overrides only when
the limitation is not uniform across the target.

## Limitation Classes

### 1. Parser Blocker

Use when the ingest path cannot construct the IR/DB substrate for the affected
file, region, or target.

Properties:

- blocks fair graph-based evaluation by default
- usually maps to `A2` support risk and `SETUP_ENVIRONMENT` or
  `INDEX_FIDELITY` failure classes downstream
- often justifies `skip_by_default` or `run_only_for_feature_X`

Examples:

- edition/syntax incompatibility that trips `syn::Error`
- unresolved proc-macro token stream that cannot be parsed into the item
  visitor pipeline

### 2. Modeling Coverage Gap

Use when parsing/indexing succeeds enough to build the graph, but known semantic
regions are absent, incomplete, or misrepresented.

Properties:

- does not always block the whole target
- often means some tasks are `graph_degraded` rather than `graph_invalid`
- should warn against attributing downstream failures entirely to tool or agent
  quality

Examples:

- macro_rules content omitted from the graph
- known dangling references caused by partial macro modeling
- proc-macro expansion absent, making some generated symbols unreachable

### 3. Scaling Constraint

Use when the approach is conceptually applicable, but runtime, token cost, or
indexing latency makes the target impractical under current constraints.

Properties:

- mostly drives execution policy rather than correctness interpretation
- should specify subset, cap, or special-run rules explicitly

Examples:

- multi-million-LoC project exceeds acceptable indexing window
- embedding/indexing cost too high for normal formal-run cadence

## Interpretability Flags

Each target or task entry should carry an interpretability flag:

- `graph_valid`
  The graph substrate is good enough for normal graph-based eval interpretation.
- `graph_degraded`
  The graph is usable, but known blind spots or omissions affect some expected
  behaviors.
- `graph_invalid`
  The graph substrate is not sufficient for fair graph-based evaluation.
- `performance_restricted`
  The graph is conceptually valid, but execution is constrained by time/cost.

## Run Policy Values

Each entry should also declare a run policy:

- `default_run`
- `skip_by_default`
- `subset_only`
- `run_only_for_feature`
- `allow_non_semantic_fallback`
- `special_run_only`

If `run_only_for_feature` is used, include the named feature or workstream that
would make the run informative, such as `macro_rules modeling` or
`proc-macro parse path`.

If `allow_non_semantic_fallback` is used, state the exact fallback boundary so
the run does not silently stop being a graph-based eval.

## Minimum Fields

Each entry should include:

- `target_id`
- `task_id` or `task_scope` when the entry is task-specific
- `status`
- `limitation_class`
- `interpretability_flag`
- `run_policy`
- `affected_surface`
- `summary`
- `evidence`
- `workaround`
- `reentry_condition`
- `owner_workstream`
- `review_trigger`
- `last_reviewed`

Recommended optional fields:

- `linked_hypotheses`
- `linked_failure_codes`
- `linked_packets`
- `linked_runs`
- `linked_postmortems`

## Update Rules

- Add or update an entry when a repeated target limitation is backed by a
  reviewed run, postmortem, parser bug, audit, or packet report.
- Prefer one durable entry per stable limitation, updated over time, rather than
  many one-off notes.
- When a parser/modeling change brings a target back into scope, update the
  entry rather than deleting it; preserve the prior limitation history.
- If a limitation changes the fairness of normal eval interpretation, reflect it
  in run policy before scheduling more formal runs.

## Relationship To Existing Workflow Artifacts

- Use the [failure taxonomy](../../active/workflow/failure-taxonomy.md) for
  run-level failure labeling.
- Use the living target capability registry for target/task readiness and
  interpretability state.
- Use the evidence ledger when a limitation meaningfully changes what we believe
  about eval validity or corpus scope.

The taxonomy classifies what happened in a run. The capability registry says
what was already known about whether a run on that target would be fair or
informative.
