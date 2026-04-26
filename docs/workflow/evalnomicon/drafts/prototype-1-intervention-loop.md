# Prototype 1 Intervention Loop

> Historical v1 note. This document preserves the original non-trampoline
> branch/evaluation plan and remains useful background for mechanized metrics,
> issue selection, and shallow branch policy. It is superseded for runtime-loop
> semantics by
> [prototype-1-intervention-loop-v2.md](prototype-1-intervention-loop-v2.md),
> because the parent binary cannot fully evaluate descendants whose source
> changes are only present after rebuilding and spawning a child/successor
> binary.

Small bounded plan for a same-day proof of concept that connects protocol
artifacts, mechanized pre-oracle metrics, and shallow intervention search over
real editable artifacts.

## Framework Anchors

This prototype is intentionally downstream of the formal procedure framework and
its later mutable/reflexive extensions. When implementation details drift, use
these documents as the semantic source of truth:

- [formal-procedure-notation.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/drafts/formal-procedure-notation.md)
  defines procedures/protocols as typed state transitions with explicit
  executors, evidential outputs, recording/forwarding rules, and DAG
  composition.
- [framework-ext-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-01.md)
  introduces the split between local procedure state `s`, mutable artifact
  state `Σ`, explicit procedure environment `Γ`, and staged reflective
  execution.
- [framework-ext-02.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-02.md)
  clarifies total configuration `C_g`, history `H_g`, and the distinction
  between artifact state and semantic environment.
- [framework-ext-03.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-03.md)
  introduces trajectory/history notation, intervention spec `ι`, and realized
  intervention event `α`.
- [framework-ext-04.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-04.md)
  adds the branch/exploration graph `Ξ`, merge as an intervention over branch
  nodes, and the distinction between trajectory and exploration structure.

For Prototype 1, the practical consequence is:

```text
analysis and synthesis are modeled as procedures/protocols
execution/validation/commit remain separate layers
search/control sits above those procedures
```

## Scope

Prototype 1 is intentionally narrow.

- use strictly programmatic pre-oracle metrics
- use bounded intervention targets
- use shallow reset/fork/select search
- defer early stopping, path dedup, protocol self-modification, and arbitrary
  source mutation

## Core Split

```text
Layer 0: raw execution record
Layer 1: mechanized pre-oracle metrics
Layer 2: candidate-fix eligibility gate
Layer 3: oracle correctness
```

Interpretation:

- Layers 1 and 2 remain strictly mechanized.
- Layer 3 may be programmatic or adjudicated depending on the benchmark.
- Layer 3 must not define Layer 1.

## Analysis vs Control Boundary

Prototype 1 should preserve a strict distinction between:

```text
adjudicated analysis evidence
mechanized evaluation and branch control
```

Interpretation:

- LLM-adjudicated protocol outputs are admissible evidence for:
  - issue detection
  - intervention targeting
  - choosing what bounded intervention to try next
- LLM-adjudicated protocol outputs are not acceptance or promotion signals.
- Branch selection, continuation, and later promotion decisions must remain
  grounded in mechanized downstream outcomes:
  - operational metrics
  - oracle-eligibility gate
  - oracle or benchmark result where applicable

In short:

```text
adjudicated protocol output answers "where should we look / what should we try?"
mechanized evaluation answers "should we keep this branch?"
```

## Prototype Loop Shape

The original non-trampoline loop shape was:

```text
eval configuration
  -> baseline arm (= eval run -> protocol run)
  -> select intervention target
  -> apply intervention
  -> treatment arm (= eval run -> protocol run)
  -> compare baseline vs treatment
```

Terminology: `eval target` = the slice of benchmark instances to run; `intervention target` = the bounded artifact surface to edit.

This shape is no longer the authoritative runtime model. The current Prototype
1 direction is a trampoline: a Parent builds a Child/Successor binary from the
patched artifact state, the fresh binary evaluates or bootstraps under its own
compiled semantics, and branch control is grounded in recorded mechanized
outcomes rather than the original parent re-running treatment in place.

| Step | Role | Driver |
| --- | --- | --- |
| Eval configuration | choose eval slice and execution settings | programmatic |
| Baseline arm | run `eval -> protocol` on the chosen slice | mixed: eval may use LLMs internally; protocol includes adjudicated LLM calls |
| Select intervention target | reduce baseline protocol evidence to one bounded target | programmatic |
| Intervention synthesis | generate the content of the proposed intervention for the already-selected target | LLM-generated content |
| Intervention apply | realize the synthesized intervention deterministically and validate it | programmatic |
| Treatment arm | rerun `eval -> protocol` on the same slice after apply | mixed: eval may use LLMs internally; protocol includes adjudicated LLM calls |
| Compare baseline vs treatment | decide whether the intervention helped | programmatic |

```text
baseline run
  -> RunRecord + protocol artifacts
  -> IssueDetectionProcedure
       input:
         - RunRecord
         - ProtocolAggregate
         - at most a few narrow support facts
       output:
         - IssueDetectionOutput { cases: Vec<IssueCase> }
         - for Prototype 1, this is a reduction over protocol-reviewed tool issues,
           not a separate failure-analysis layer
  -> select_issue_case(...)
  -> InterventionSynthesisProcedure
       input:
         - selected IssueCase
       output:
         - InterventionSynthesisOutput
         - InterventionSpec
  -> execute intervention
  -> rerun bounded eval slice
  -> BranchEvaluationProcedure
       input:
         - baseline OperationalRunMetrics
         - treatment OperationalRunMetrics
         - oracle eligibility / oracle result where applicable
       output:
         - keep / reject / continue-from-here
```

The important boundary is:

```text
IssueDetectionProcedure does not take full OperationalRunMetrics as a primary input.
BranchEvaluationProcedure is where OperationalRunMetrics belongs.
```

## Prototype 1

1. Externalize mutable tool guidance text into first-class artifact files loaded
   with `include_str!`, replacing `ToolDescr` as the description carrier while
   keeping tool names and schemas typed in Rust.

2. Add a mechanized run-level `OperationalRunMetrics` summary in
   `ploke-eval` for:

   ```text
   convergence
   partial patch failure count
   same-file patch retry count
   same-file patch retry max streak
   aborted repair loop
   nonempty valid patch
   oracle eligibility
   ```

3. Expose those operational metrics through a compact CLI surface so treatment
   vs baseline comparisons can be inspected without large JSON dumps.

4. Define a minimal bounded intervention model:

   ```text
   InterventionSpec
   ValidationPolicy
   ```

   with an initial allowed target set covering:

   ```text
   tool description artifacts
   one small policy/config surface
   ```

   This step also needs the procedure-state and capability surfaces those
   entities presuppose. Otherwise the spec model becomes dead data with no
   explicit execution path.

   Prototype 1 treats `InterventionSpec` as the selected bounded action
   produced by an intervention-synthesis procedure, not as the procedure
   itself. The relevant procedure/protocol framing is defined in:

   - [formal-procedure-notation.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/drafts/formal-procedure-notation.md)
   - [framework-ext-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-01.md)
   - [framework-ext-03.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-03.md)

   The minimal state boundary is therefore:

   ```text
   InterventionSynthesisInput
   InterventionSynthesisOutput
   InterventionExecutionInput
   InterventionExecutionOutput
   ```

   Prototype 1 should therefore define the seams for:

   ```text
   materialize
   stage
   apply
   validate
   ```

   without requiring every backend to be fully implemented yet.

5. Add a branch-local candidate state ledger with:

   ```text
   state_id
   parent_state_id
   generation
   intervention record
   evaluation record
   status
   ```

   sufficient for reset / fork / compare / promote over a shallow search.

6. Implement prototype-1 search policy:

   ```text
   from one anchor state
   try up to X intervention steps on each of Y reset branches
   record every evaluated node
   select the best node rather than only a leaf
   ```

7. Start with one concrete intervention-targeting reduction:

   ```text
   protocol-reviewed tool issues
     -> shortlist the highest-issue reviewed tool target
   ```

   and test whether interventions improve operational metrics on a bounded eval
   slice. Richer issue-family analysis can be revisited later if it proves
   necessary.

## Current Implementation Status

The first three Prototype 1 steps now have concrete implementation anchors in
the codebase.

### Step 1: Editable Artifact Surface

Tool guidance text is now externalized into real artifact files under
[crates/ploke-core/tool_text](/home/brasides/code/ploke/crates/ploke-core/tool_text),
and loaded through
[tool_descriptions.rs](/home/brasides/code/ploke/crates/ploke-core/src/tool_descriptions.rs:1)
via `include_str!`.

The typed binding layer remains in Rust:

- [tool_descriptions.rs](/home/brasides/code/ploke/crates/ploke-core/src/tool_descriptions.rs:5)
  maps each `ToolName` to an external text artifact
- [tool_types.rs](/home/brasides/code/ploke/crates/ploke-core/src/tool_types.rs:111)
  now uses `ToolFunctionDef.description: String`
- [tool_types.rs](/home/brasides/code/ploke/crates/ploke-core/src/tool_types.rs:133)
  exposes `ToolName::description()`

This is the current editable intervention surface for tool-guidance changes.

### Step 2: Mechanized Operational Metrics

Run-level mechanized pre-oracle metrics are now computed in
[operational_metrics.rs](/home/brasides/code/ploke/crates/ploke-eval/src/operational_metrics.rs:36)
as `OperationalRunMetrics`.

Current fields include:

```text
tool_calls_total
tool_calls_failed
patch_attempted
patch_apply_state
submission_artifact_state
partial_patch_failures
same_file_patch_retry_count
same_file_patch_max_streak
aborted
aborted_repair_loop
nonempty_valid_patch
convergence
oracle_eligible
```

Important implementation details:

- [operational_metrics.rs](/home/brasides/code/ploke/crates/ploke-eval/src/operational_metrics.rs:53)
  keeps `nonempty_valid_patch` as a conservative pre-oracle proxy rather than
  proof that a nonempty submission patch artifact exists on disk
- [operational_metrics.rs](/home/brasides/code/ploke/crates/ploke-eval/src/operational_metrics.rs:118)
  now treats `oracle_eligible` as the stricter adjudication gate:
  convergence plus a concrete nonempty submission artifact

The concrete packaging/output fact is persisted separately in the run record:

- [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1341)
  defines `PackagingPhase`
- [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1350)
  stores `submission_artifact_state`
- [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1942)
  and [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2449)
  populate that packaging state when writing run records

This separation is intentional:

```text
nonempty_valid_patch     = operational workflow-health proxy
submission_artifact_state = concrete packaging/output fact
```

### Step 3: Compact CLI Surface

The compact CLI surface for these metrics is now implemented under
[InspectOperationalCommand](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2242)
and exposed as:

```text
ploke-eval inspect operational
ploke-eval inspect metrics
```

Relevant implementation points:

- [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2108)
  adds `inspect operational` with `metrics` as an alias
- [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:5500)
  renders the compact run-level operational summary

This gives Prototype 1 a compact inspection surface for baseline/treatment
 comparison without requiring a large conversation JSON dump.

### Step 4: Bounded Intervention Model

Current Prototype 1 intervention work should be read against the framework
anchors above, especially the distinction between:

```text
procedure/protocol output state
intervention spec ι
realized intervention event α
artifact/configuration state Σ
```

That means:

- issue detection wants to live on the same protocol/artifact substrate already
  used by `ploke-eval protocol`
- intervention synthesis is a procedure that produces `InterventionSpec`
- execution, validation, and later commit/search logic remain separate
  controller-side steps
- protocol review and other adjudicated artifacts belong in the evidence state
  for issue detection and intervention synthesis
- those adjudicated artifacts do not determine branch continuation or branch
  promotion on their own

The current code only partially realizes this split. Prototype 1 still uses a
local execution shim, but the semantic source of truth is the procedure
framework above, not the temporary execution helper.

Current implementation anchors:

- [intervention/mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/mod.rs)
  defines the narrow module surface
- [intervention/spec.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/spec.rs)
  defines `InterventionSpec`, `ValidationPolicy`, and the synthesis/execution
  state packets
- [intervention/issue.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/issue.rs)
  defines `IssueCase`, `IssueDetectionInput`, `IssueDetectionOutput`,
  `detect_issue_cases(...)`, `select_primary_issue(...)`, the protocol-backed
  issue evidence packet, and the persisted issue-detection packet input.
  Prototype 1 currently reduces protocol-reviewed call issues into a reviewed
  tool target rather than trying to infer a richer failure family.
- [intervention/synthesize.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/synthesize.rs)
  defines the current fan-out LLM synthesis procedure, which produces a
  candidate set of full replacement tool-description texts for one already
  selected target
- [intervention/apply.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/apply.rs)
  defines deterministic apply against the expected source content for one
  selected candidate
- [intervention/branch_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/branch_registry.rs)
  defines the loop-scoped branch registry and branch lifecycle/state transitions
- [intervention/execute.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/execute.rs)
  contains the current prototype-local execution shim
- [branch_evaluation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/branch_evaluation.rs)
  introduces the explicit branch-evaluation home for `OperationalRunMetrics`
- [intervention_issue_aggregate.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention_issue_aggregate.rs)
  loads the latest persisted `intervention_issue_detection` artifact back into
  a run-local aggregate shape for inspection
- [intervention/tests.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/tests.rs)
  exercises the current `detect -> select -> synthesize -> execute` slice
- [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
  now exposes:
  - `ploke-eval protocol issue-detection`
  - `ploke-eval inspect issue-overview`
  - `ploke-eval loop prototype1`
  - `ploke-eval loop prototype1-branch status/show/apply/select/restore/evaluate`

The current Prototype 1 chain is therefore:

```text
RunRecord
-> IssueDetectionOutput
-> persisted intervention_issue_detection artifact
-> IssueDetectionAggregate
-> select_primary_issue(...)
-> InterventionSynthesisOutput
-> InterventionCandidateSet
-> InterventionApplyOutput
-> branch registry state
-> branch evaluation artifact/report
```

The current issue-detection evidence packet is now:

```text
RunRecord
ProtocolAggregate? (latest available segmentation/review evidence)
```

This preserves the intended boundary:

- protocol-derived adjudicated evidence may inform issue detection and
  intervention targeting
- full `OperationalRunMetrics` now belongs to branch evaluation rather than
  issue detection
- mechanized metrics and later oracle outcomes remain the branch-control layer

This is still only a partial projection of the intended protocol/controller
loop. The missing live frontier is treatment-arm execution through the full
loop wrapper on real branches; however, the synthesis/apply/branch/evaluate
surfaces now exist concretely in code and CLI.

## Step 4 Clarification

Step 4 should be implemented as three smaller moves, not one large implicit
bundle.

### Step 4a: Define bounded intervention entities

This is the object-model part:

```text
InterventionSpec
ValidationPolicy
```

This is where the code should make explicit:

- what bounded action is being selected
- what evidence/provenance a spec carries
- what validation policy attaches to the spec
- which intervention families are valid and how each family carries its own
  target fields

Prototype 1 now also needs a sibling issue-detection packet boundary:

```text
IssueDetectionInput
IssueDetectionOutput
```

so detection is not silently collapsed into local helper selection logic.

### Step 4b: Define capability surfaces

This is the missing layer between intervention specs and real treatment states.

Prototype 1 should define capability seams for:

```text
materialize a selected spec into an edit payload
stage that payload against a bounded target
apply the staged edit
validate the resulting treatment state
```

These seams should exist even if some concrete implementations remain `todo!()`
 for now.

The important constraint is:

```text
do not silently hard-code backend choices into the proposal model itself
```

Examples of backend choices that should stay behind the capability seam:

- direct file editing vs reuse of the `ploke-tui` editing substrate
- shelling out to `cargo` vs reusing an existing tool surface
- direct branch/worktree manipulation vs a future environment adapter

### Step 4c: Implement one narrow concrete path

Prototype 1 does not need every target class or backend immediately.

It does need one real end-to-end path so the step is not purely abstract.

The intended first path is:

```text
IssueSelectionBasis
  -> InterventionSpec
  -> tool description artifact target
  -> stage/apply through one real execution path
  -> validate
```

Everything outside that first path may remain explicitly unimplemented until it
is actually needed.

## Implementation Discipline For Step 4

The reason for the 4a / 4b / 4c split is to avoid accidentally implementing too
many unrelated behaviors at once.

In particular, Step 4 should not silently expand into:

```text
full branch management
full candidate-state search
arbitrary source mutation
multiple validation backends
implicit shell-only execution policy
```

Instead, Step 4 should give the codebase:

```text
the intervention entities
the capability seams they presuppose
one narrow concrete adapter path
```

That keeps later steps honest and reduces the chance that unused helper paths
or dead-code abstractions accumulate around the prototype.

## First Target Surface

The first intervention surface should be real artifacts, not embedded enum
serialization strings.

Recommended first targets:

```text
tool_description(apply_code_edit)
tool_description(insert_rust_item)
tool_description(non_semantic_patch)
one eval prompt fragment
one small policy/config surface for raw-patch gating
```

## Candidate-Fix Gate

Oracle evaluation should only be considered after a run crosses a mechanized
eligibility threshold.

```text
EligibleForOracle(r) ⇔
  completed(r)
  ∧ nonempty_valid_patch(r)
  ∧ patch_apply_state(r) = yes
  ∧ ¬aborted_repair_loop(r)
```

Interpretation:

- empty-patch, partial-apply, and aborted repair-loop runs are not meaningful
  oracle datapoints
- those runs should first be improved at the operational layer

## Deferred

The following are explicitly deferred from Prototype 1:

```text
early stopping within a path
path dedup / near-duplicate detection
protocol self-modification
arbitrary source patching
oracle-driven optimization inside the intervention loop
```

## Same-Day Success Condition

Prototype 1 is successful if it provides:

```text
one editable artifact surface
one mechanized operational metric summary
one bounded intervention target set
one shallow reset/fork/select loop
one concrete failure family exercised on a bounded eval slice
```

As of this draft update, the first three pieces are implemented:

```text
editable artifact surface
mechanized operational metric summary
compact CLI inspection surface
```

The next unfinished pieces are:

```text
bounded intervention target model
capability surfaces for materialize / stage / apply / validate
candidate-state ledger
shallow reset/fork/select loop
first end-to-end intervention experiment
```

### Step 4 Status

The current code now has the first semantic boundary in place:

- [mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/mod.rs:1)
  defines the intervention module boundary and keeps the public surface narrow
- [spec.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/spec.rs:57)
  defines `InterventionSpec` as the selected bounded action spec
- [spec.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/spec.rs:121)
  defines the procedure IO boundary:
  `InterventionSynthesisInput/Output` and
  `InterventionExecutionInput/Output`
- [execute.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/execute.rs:18)
  keeps the prototype-local concrete execution seams for:
  `materialize`, `stage`, `apply`, and `validate`
- [execute.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/execute.rs:237)
  provides the current narrow execution entrypoint:
  `execute_tool_text_intervention(...)`
- [tests.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/tests.rs:1)
  covers the current narrow path:
  successful tool-text mutation, explicit policy-target unimplemented status,
  and disallowed-target validation failure

Current `ploke-protocol` integration should still be treated as adapter code.
The canonical semantic boundary for this prototype is the intervention
synthesis/execution state types above, pending the larger `type-state.md`
procedure refactor.
