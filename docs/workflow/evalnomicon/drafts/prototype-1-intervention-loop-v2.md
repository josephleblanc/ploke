# Prototype 1 Intervention Loop V2

Current semantic source for Prototype 1 runtime/trampoline behavior after the
parent/child/successor runtime seam became explicit in the implementation. The
material change from v1 is:

```text
the modified state is not fully evaluable by the parent runtime
the child binary must evaluate itself
```

So Prototype 1 is no longer adequately described as:

```text
baseline -> apply intervention -> rerun treatment
```

It is better described as:

```text
baseline parent runtime
  -> synthesize descendant candidates
  -> realize one descendant into modified artifact state
  -> build descendant binary
  -> spawn descendant runtime
  -> descendant evaluates itself
  -> record outcome
  -> active runtime applies policy over recorded outcomes/history
  -> selected successor binary acknowledges handoff
```

This remains a narrow prototype, but the narrowness is a safety/budget choice,
not a semantic limit of the trampoline model. The current implementation has
focused on tool-description artifacts loaded into the binary with
`include_str!`; the long-term direction is a bounded source-edit surface that
may include the analysis protocol, evaluator framework, and other code used to
build future successors. Branch-control decisions still remain grounded in
mechanized evaluation and external oracle signals rather than LLM self-judgment
or child self-promotion.

## Framework Anchors

This version remains downstream of the same formal sources as v1:

- [formal-procedure-notation.md](formal-procedure-notation.md)
  Procedures/protocols as typed state transitions with explicit executors,
  evidential outputs, and composition rules.
- [framework-ext-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-01.md)
  Split between local procedure state `s`, mutable artifact state `Σ`, explicit
  procedure environment `Γ`, and staged reflective execution.
- [framework-ext-02.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-02.md)
  Clarifies total configuration `C_g`, history `H_g`, and the distinction
  between artifact state and semantic environment.
- [framework-ext-03.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-03.md)
  Introduces intervention spec `ι`, realized event `α`, and trajectory/history
  notation.
- [framework-ext-04.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-04.md)
  Adds the exploration graph `Ξ`, merge as an intervention over branch nodes,
  and the distinction between trajectory and exploration structure.

Two later notes are also directly relevant to the current code shape:

- [isomorphic-code-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/isomorphic-code-01.md)
  Strongest argument for phase-indexed/state-indexed intervention families.
- [module-tree-and-trait-algebra.md](module-tree-and-trait-algebra.md)
  Current draft for how the codebase should split formal kinds, concrete
  transitions, and implementation modules.

The practical consequence remains:

```text
analysis and synthesis are procedures/protocols
bounded realization happens through explicit capability seams
search/control sits above those procedures
history survives across generations
```

## Scope

Prototype 1 remains intentionally narrow.

- use programmatic pre-oracle metrics
- keep the editable surface explicitly bounded
- keep search shallow
- defer merge/union over disjoint edits
- defer unbounded arbitrary source mutation
- defer distributed/multi-machine consensus

The goal is still to prove one real end-to-end self-modifying loop over a small
surface before broadening the target class. The intended broadening is still
source-level self-editing: the valid patch target set may grow to include the
runtime, analysis protocols, and evaluation framework, provided the selected
successor compiles and passes the configured validation/oracle gates.

## Core Evaluation Boundary

The original v1 distinction still holds:

```text
adjudicated protocol output answers "what should we try?"
mechanized evaluation answers "should we keep this branch?"
```

In Prototype 1 that means:

- baseline protocol/eval output may guide intervention targeting
- mechanized metrics and benchmark/oracle signals decide keep/reject/continue
- the child does not get to self-promote by emitting nicer prose

The existing mechanized comparison still lives in:

- [branch_evaluation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/branch_evaluation.rs)

and compares baseline vs treatment `OperationalRunMetrics` directly.

## Why V2 Exists

The key semantic issue discovered during implementation is that there are at
least two state axes:

- codebase / artifact state
- binary / runtime state

Changing an `include_str!` tool-description file mutates the artifact state
immediately, but the parent binary is already compiled. So after mutation:

- the parent runtime still embodies the old binary semantics
- the artifact world may already reflect the child lineage
- only a newly built child binary actually inhabits the modified tool surface

So the parent can:

- evaluate the baseline
- choose bounded descendants
- realize/build/spawn descendants
- observe descendant outcomes

But the parent cannot fully evaluate the descendant binary semantics from
inside itself. The descendant must evaluate itself.

That is the material change from v1.

## Revised Loop Shape

The intended Prototype 1 loop is now:

```text
active generation runtime
  -> run baseline eval/protocol on bounded slice
  -> reduce evidence to one bounded intervention target
  -> synthesize one or more descendant candidates
  -> record candidates in shared history
  -> for each candidate selected for realization:
       -> materialize descendant artifact state
       -> build descendant binary
       -> spawn descendant runtime
       -> wait for descendant acknowledgement
       -> descendant runs treatment eval/protocol
       -> descendant records outcome
  -> compare candidate outcomes
  -> choose one successor branch/runtime or stop
  -> materialize/build selected successor if needed
  -> hand off authority to selected successor
  -> old parent exits
```

This is still one active generation governing the loop. There is no permanent
external controller above the generations. The currently active runtime owns:

- synthesis
- realization of descendants
- policy application
- restore/revisit behavior
- authority handoff to the selected successor

The concurrency invariant should be stated explicitly: a generation may propose
or realize many children, and future implementations may run a bounded set of
children concurrently, but at most one successor receives parent authority for
the next generation.

## Descendant State Distinctions

A child may exist in more than one sense.

```text
proposed child
  patch exists, not yet applied

realized child artifact
  patch applied to the parent's recoverable base artifact state

built child
  descendant binary exists but is not yet acknowledged/running

acknowledged child runtime
  descendant has started and identified itself through the shared record

evaluated child
  descendant has completed one treatment run and recorded its result

selected successor
  one evaluated child chosen by parent policy as the next authoritative lineage

successor runtime
  fresh binary built from the selected artifact state; it acknowledges handoff,
  becomes the next Parent, and continues only if policy/budget checks permit
```

These distinctions are exactly where typestate is useful.

The current scaffold already models the first runtime-succession seam with:

```text
C1 = parent binary over parent artifact world
C2 = parent binary over child artifact world
C3 = parent binary over child artifact world, child binary present
C4 = parent binary over child artifact world, child acknowledged
```

Current code anchors:

- [intervention/algebra/mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/algebra/mod.rs)
- [cli/prototype1_state/c1.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c1.rs)
- [cli/prototype1_state/c2.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c2.rs)
- [cli/prototype1_state/c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs)
- [cli/prototype1_state/journal.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/journal.rs)

## Policy

Prototype 1 already has a useful first policy surface in the old
implementation.

Current policy anchors:

- [intervention/scheduler.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs)
- [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)

What `Policy` should mean in this prototype:

- stop conditions
- continuation vs termination
- branch ranking/selection
- restore/revisit/fork choice
- bounded search limits

What `Policy` should not mean:

- the execution of local artifact/binary transitions themselves

Those remain concrete `Intervention`s.

## History And Journal

Prototype 1 needs one shared machine-readable record substrate across
generations.

That substrate has to support:

- candidate/branch lineage
- node status
- descendant runtime acknowledgement
- evaluation outcomes
- continuation decisions
- later restore/revisit decisions

Today that information is split across:

- [intervention/branch_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/branch_registry.rs)
- [intervention/scheduler.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs)
- node-local artifacts like `runner-request.json` and `runner-result.json`
- the new typed journal in [cli/prototype1_state/journal.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/journal.rs)

That split is acceptable during transition, but the direction is:

```text
history/journal = shared durable substrate
policy = generation-local reasoning over that substrate
intervention = local typed transition between configurations
```

## Current Implementation Status

There are currently two overlapping implementation paths. They should not be
treated as equivalent.

The older `loop prototype1` path owns most of the practical branch machinery:
candidate registration, branch evaluation, metric summaries, and simple branch
selection. It can evaluate multiple branches in one controller run, but it is
not the long-term runtime-succession design because it does not make the
selected successor binary become the next authoritative parent.

The newer `prototype1_state` / invocation path is the intended replacement for
runtime succession. It models the Parent -> Child -> Completed child ->
Selected successor -> Successor bootstrap object explicitly, but currently only
implements the single-child path plus bounded successor acknowledgement. The
next cleanup work is to move the older path's useful policy/history/evaluation
pieces behind the newer typed trampoline carriers rather than extending the old
non-trampoline controller as the primary architecture.

The current live implementation already answers several practical Prototype 1
questions:

- branch evaluation record shape
- continuation/search policy
- restore semantics
- branch ranking over mechanized summaries

Relevant current files:

- [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
- [cli/prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs)
- [intervention/branch_registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/branch_registry.rs)
- [intervention/scheduler.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs)
- [branch_evaluation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/branch_evaluation.rs)

More concretely:

- `branch_registry.rs`
  already records source nodes, descendant branches, branch ancestry, selected
  branch state, active targets, restore state, and latest evaluation summary.
- `scheduler.rs`
  already records node identity, runner request/result paths, node status, and
  continuation policy.
- `prototype1_process.rs`
  already owns the dangerous process seam:
  materialize -> build -> spawn child -> wait -> read result, plus bounded
  successor bootstrap after selection.
- `branch_evaluation.rs`
  already defines the mechanized keep/reject rule over baseline vs treatment
  metrics.

## Current Replacement Status

The new scaffold does not yet replace the entire live implementation, but it is
the architectural direction for multi-generation self-improvement.

What it already replaces well:

- the local runtime-succession path for one node:
  - materialize
  - build
  - spawn/ack
- explicit before/after journal commit
- typed rejection vs broken-transition distinction
- replay classification for the currently modeled transition families

What it does not yet replace:

- child-side result completion and post-`C4` states
- full evaluation artifact/result integration
- policy-driven sibling comparison in the new layer
- durable selected-successor decisions
- successor rehydration as a real Parent entrypoint
- restore/fork behavior as explicit typed transitions
- full consolidation of branch registry + scheduler + journal into one coherent
  history substrate

So the current state is:

```text
old implementation still owns most multi-branch evaluation machinery
new implementation owns the intended runtime-succession/trampoline model
next work is to migrate useful old policy/history/evaluation pieces into the
new typed transition structure, not to treat the old path as the final loop
```

## Capability Seams Still Required

The v1 requirement from Step 4b remains valid, but its meaning is now slightly
broader.

Prototype 1 still needs explicit seams for:

```text
materialize
stage
apply
validate
```

and now also for the binary/runtime side of descendant realization:

```text
build
spawn
acknowledge
observe outcome
restore
```

The important discipline remains the same:

```text
do not silently hard-code backend choices into the proposal model itself
```

The editable target remains a bounded artifact surface. The runtime/build side
is the realization mechanism needed to let the child evaluate itself.

## Intended Near-Term Shape

The near-term replacement target is:

1. Preserve the existing mechanized evaluation and branch-policy logic.
2. Keep the editable surface bounded, starting with the current
   tool-description artifacts and later admitting wider source surfaces only
   through explicit validation gates.
3. Move local node realization behind typed `Intervention`s.
4. Re-home shared history/journal concerns so generations can reason over a
   common durable substrate.
5. Re-home policy so the active runtime can:
   - stop
   - continue
   - pick next descendant
   - restore and fork from prior states
6. Extend the typestate family past `C4` to cover:
   - child-completed
   - result-observed
   - successor-selected
   - successor-bootstrapped
   - parent-exited
   - restore-ready

## Summary

Prototype 1 is now best understood as:

```text
a generation-indexed, policy-guided, bounded self-modification loop
over a shared history/journal substrate,
where descendants must evaluate themselves because binary semantics lag artifact
mutation until rebuild/spawn.
```

That is the main correction to v1, and the current implementation work should
continue from that corrected object rather than from a simpler
"apply then rerun in place" picture.
