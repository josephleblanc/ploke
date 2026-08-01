# ADR 008: Prototype 1 Composable Edit Strategy Algebra

## Status
Proposed (2026-06-21; implementation deferred)

## Context

Prototype 1 currently has a relatively rigid inner edit-generation path. The
outer loop has typed phase/authority transitions, but the inner path that asks an
agent to review context, use tools, propose edits, validate them, and return
candidate evidence is mostly a fixed orchestration.

That fixed path is useful for the simplest policy:

```text
prepare candidate workspace -> run one LLM/tool loop -> collect patch evidence
```

But we want to experiment with richer strategies without rewriting the outer
Prototype 1 lifecycle each time. Examples include:

```text
review evidence -> plan edit -> implement plan -> validate -> admit candidates
```

and:

```text
review evidence -> plan edit -> fan out multiple implementation agents ->
validate all -> select admitted candidates
```

The existing `ploke-protocol` crate already has typed procedure concepts such as
steps, sequences, fan-out, and merge. Those ideas are relevant, but the desired
Prototype 1 edit strategy layer has a narrower operational purpose: make the
candidate edit-generation path configurable, typed, replayable, and evidence
preserving while keeping the outer loop typestate authority intact.

The detailed working design is tracked in the ploke-eval mdBook page:

- [`crates/ploke-eval/docs/prototype1/edit-strategy-algebra.md`](../../../crates/ploke-eval/docs/prototype1/edit-strategy-algebra.md)

## Decision

Adopt a future design direction in which Prototype 1 edit generation is modeled
as a composable typed strategy algebra.

The design will use typed stages shaped like arrows:

```text
Input -> Stage -> Output
```

A stage declares the typed input it requires and the typed output it produces.
Composition is valid only when a prior stage output satisfies a later stage
input. This gives the edit-generation path a functional composition style while
remaining distinct from the outer loop's rigid phase typestate.

The current behavior should become the default strategy:

```text
EditContext -> ToolLoopStage -> CandidatePatch
```

The first richer strategy should be fixed and named, not arbitrary user-authored
graph configuration:

```text
EditContext
  -> ReviewStage
  -> ReviewReport
  -> PlanStage
  -> EditPlan
  -> FanOut(ImplementPlanStage)
  -> CandidatePatches
  -> ValidateEach
  -> CandidateSet
  -> SelectAdmitted
  -> ChildPlanInputs
```

The configuration model should initially select named strategies, for example
`tool_loop` or `review_plan_fanout`. Arbitrary declarative graph configuration
may be added later only after the fixed strategies are stable and a runtime type
checker can reject invalid stage adjacency before execution.

## Terminology decision

Use **tool access profile**, not "tool surface", for the set of tools a strategy
stage may call.

This avoids collision with Prototype 1's existing authority terms:

- **tool access profile**: which runtime tools a stage may call, and in what
  modes.
- **mutable/immutable surface**: which repository/evidence paths may be read or
  mutated.

A stage may have access to a mutating tool such as `ns_patch`, but that tool
must still be constrained by the current mutable surface policy and protected
roots.

## Required properties

### Typed composition

Stages must have explicit typed inputs and outputs. A review output should not
be passed to an implementation stage unless the implementation stage declares it
can consume that shape directly or through a typed plan.

### Named strategy configurations first

Profiles should start with named strategies rather than arbitrary graphs:

- `tool_loop`
- `review_plan_fanout`

A later graph-based config must validate stage kind, input/output type ids,
recording policy, prompt ids, and tool access profiles before any live execution.

### Tool access profiles

Agent-calling stages must declare a deny-by-default tool access profile. Initial
profiles should include:

- `read_only_review`
- `planning_read_only`
- `implementation_edit`
- `validation_only`

The normalized profile should be persisted with the stage evidence so later
reviews know exactly which tools were available.

### Prompt stack registry

Agent-calling stages should use prompt stacks rather than one-off formatted
strings. A prompt stack should include:

- stable base prompt id
- optional strategy overlay
- stage-specific instructions
- typed input rendering
- authority/guardrail appendix
- prompt content digests and rendered prompt artifact references

### Recorded invocation API

Strategy authors should have an ergonomic helper for invoking an agent and
persisting standard records. The working name is `run_recorded_agent_turn` or an
equivalent builder-style API.

The helper should accept:

- stage id
- strategy run id
- typed input payload
- prompt stack
- model/provider route
- tool access profile
- record root
- expected typed output shape

It should return typed output plus evidence references.

### Reuse existing replayable evidence shape

Agent-calling stages must persist or link the same class of records used by the
current headless TUI/test-harness tool loops:

- `*.headless-tui.json`
- `.turn-live/agent-turn-trace.json`
- `.turn-live/agent-turn-summary.json`
- `.turn-live/llm-full-responses.jsonl`
- `debug/tool-loop/<session>/steps/*.json` when response-stepped debugging is
  enabled

These records should remain compatible with the existing typed projections in
`crates/ploke-eval/src/record.rs`, replay commands, and protocol review inputs.

### Evidence-preserving failure semantics

The strategy layer should preserve bad attempts as loop evidence when authority
is not violated:

- provider/tool transient failures can become rejected attempt evidence
- validation failures can become rejected candidate evidence
- wrong-target or non-exportable patches can become rejected evidence if the
  authority boundary is preserved
- authority boundary violations and persistence/schema errors remain fatal

Do not silently drop records to keep the loop moving.

## Consequences

### Positive

- Edit-generation policies can be changed without rewriting outer Prototype 1
  phase transitions.
- Review, planning, implementation, validation, and selection can be tested as
  smaller pieces.
- Strategy runs become replayable and inspectable with existing record tooling.
- Tool exposure and prompt variants become explicit policy rather than hidden
  call-site behavior.
- Bad candidate attempts can be retained as useful evidence instead of becoming
  unexplained hard stops.

### Negative

- Adds another abstraction layer inside `ploke-eval`.
- Requires careful naming and documentation to avoid confusion with existing
  mutable/immutable surface authority.
- Requires config validation strong enough to reject invalid strategy shapes
  before live execution.
- Requires disciplined record writing so strategy artifacts do not drift from
  current headless TUI records.

### Neutral

- This ADR does not require immediate implementation.
- The first implementation should preserve current behavior by making
  `tool_loop` the default strategy.
- Generic composition traits may later move toward `ploke-protocol`, but the
  first slice can live inside `ploke-eval` to keep scope bounded.

## Deferred implementation plan

When implementation begins, use the mdBook design page as the working plan:

- [`crates/ploke-eval/docs/prototype1/edit-strategy-algebra.md`](../../../crates/ploke-eval/docs/prototype1/edit-strategy-algebra.md)

Recommended first slice:

```text
EditStrategyConfig::ToolLoop
EditContext
ToolAccessProfileSpec
PromptStackSpec
AgentTurnRecordRef
ToolLoopStage
StageRun<CandidatePatch>
StageEvidence
```

The first implementation must prove compatibility with today's broad headless
TUI path before adding `review_plan_fanout`.

## Affected areas when implemented

Likely areas include:

- `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/`
- `crates/ploke-eval/src/replay/`
- `crates/ploke-eval/src/record.rs`
- `crates/ploke-eval/docs/prototype1/edit-strategy-algebra.md`
- `crates/ploke-protocol` only if generic composition pieces are later promoted

## Open questions

1. Should prompt definitions live in code, profile files, or a versioned prompt
   registry?
2. Should tool access profiles be global Prototype 1 policy, strategy-local
   policy, or global policy with strategy-local narrowing only?
3. Which existing writer should become the canonical recorded-agent-turn API?
4. Should benchmark feedback be a first-class typed input or a field of
   `EditContext`?
5. When arbitrary graph config is added, what stable type-id vocabulary should
   it use?
