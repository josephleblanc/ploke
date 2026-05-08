# Ploke-Protocol Architecture Checkpoint

- date: 2026-04-15
- task title: ploke-protocol architecture checkpoint
- task description: checkpoint note for the first major rewrite of `crates/ploke-protocol` from a one-shot review bootstrap into a typed composed procedure model with per-step artifacts
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/agents/2026-04-15_protocol-cold-start-reference.md`, `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md`, `docs/workflow/evalnomicon/protocol-typing-scratch.md`, `docs/workflow/evalnomicon/src/core/conceptual-framework.md`

## What Changed

`crates/ploke-protocol` was rewritten away from the earlier minimal
`Protocol::run(subject) -> output` bootstrap into a composed procedure model.

The new core introduces:

- typed `StepSpec`
- executor-specific `StepExecutor`
- per-step `StepArtifact`
- composed `Procedure` execution
- `Sequence` composition
- `FanOut` composition scaffold
- `NamedProcedure` for stable protocol identity
- `JsonAdjudicator` as an executor rather than a free-floating helper

This means the crate now models:

- typed intermediate states
- explicit step outputs
- provenance per step
- composition of mechanized and adjudicated steps
- a path toward branching and later join/aggregation work

## Current Module Shape

Main files now are:

- `crates/ploke-protocol/src/core.rs`
- `crates/ploke-protocol/src/step.rs`
- `crates/ploke-protocol/src/procedure.rs`
- `crates/ploke-protocol/src/llm.rs`
- `crates/ploke-protocol/src/tool_calls/trace.rs`
- `crates/ploke-protocol/src/tool_calls/review.rs`

## Important Design Decision

The crate does not try to own `RunRecord` or `ploke-eval` persistence.

Current division:

- `ploke-eval`
  - owns concrete run artifacts
  - builds concrete subjects from run records
  - remains the adapter from persisted eval evidence into protocol subjects
- `ploke-protocol`
  - owns typed execution and composition semantics
  - owns step/procedure artifacts
  - owns executor abstractions

This avoids creating a dependency cycle while keeping the architectural boundary
clean.

## First Concrete Procedure

The old tool-call review command now runs through the new procedure model.

Current shape:

1. mechanized step: `SelectIndexedCall`
2. adjudicated step: `ReviewEvidence`
3. named composed procedure: `ToolCallReview`

This gives a real end-to-end example of:

- subject -> intermediate evidence packet -> judgment

with both step artifacts preserved.

## Verification Status

At this checkpoint:

- `cargo fmt --all` passes
- `cargo check -p ploke-protocol -p ploke-eval` passes
- crate-local `ploke-protocol` tests were added for:
  - linear sequence artifact preservation
  - fan-out branch artifact preservation
- `ploke-eval` test builds still appear vulnerable to environment/linker issues
  rather than a clear logic failure in this protocol rewrite

## What Is Still Thin

- no join combinator yet beyond sequencing a tuple-producing branch result into a
  later step
- no persisted on-disk protocol artifact writer in `ploke-eval` yet
- no second concrete protocol yet to pressure-test the abstraction
- no calibration/agreement layer yet
- no richer subject-packet construction moved out of CLI helpers yet

## Best Next Step

The next meaningful implementation slice is not another generic refactor.

It should be one of:

1. persist protocol procedure artifacts beside the run in `ploke-eval`
2. add a second bounded protocol to test whether the architecture generalizes
3. move more subject-building logic out of ad hoc CLI code into clearer adapter
   surfaces on the `ploke-eval` side

The preferred order is probably:

1. protocol artifact persistence
2. second bounded protocol

That will test whether the new architecture is actually durable rather than just
clean on paper.
