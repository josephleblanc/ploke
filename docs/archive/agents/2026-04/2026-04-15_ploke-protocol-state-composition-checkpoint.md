# Ploke-Protocol State Composition Checkpoint

- date: 2026-04-15
- task title: ploke-protocol state composition checkpoint
- task description: second architectural checkpoint for `crates/ploke-protocol`, shifting the crate from typed value composition toward typed state transitions with explicit fork and merge semantics
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_ploke-protocol-architecture-checkpoint.md`, `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`

## What Changed

`crates/ploke-protocol` was rewritten a second time to align more directly with
the state-based procedure model in `formal-procedure-notation.md`.

The main semantic shift is:

- step specs now describe `InputState` and `OutputState`
- the crate now has an explicit `ProcedureState` boundary
- step execution now carries `StateDisposition`
- artifacts distinguish state payloads from recording/forwarding disposition
- forked execution now produces a branch-preserving `ForkState`
- merge is now a first-class combinator rather than only an implied follow-up

## Current Module Shape

Main files remain:

- `crates/ploke-protocol/src/core.rs`
- `crates/ploke-protocol/src/step.rs`
- `crates/ploke-protocol/src/procedure.rs`
- `crates/ploke-protocol/src/llm.rs`
- `crates/ploke-protocol/src/tool_calls/trace.rs`
- `crates/ploke-protocol/src/tool_calls/review.rs`

But the core vocabulary now includes:

- `ProcedureState`
- `StateDisposition`
- `StateEnvelope`
- `ForkState`
- `MergeArtifact`
- `Merge`
- `MergeError`

## Execution Model Now

The crate can now express:

1. typed state transition steps
2. sequential composition over typed states
3. forked branch execution with source-state preservation
4. explicit merge composition over branch-preserving fork states

This is still not a full graph execution model, but it is materially closer to
the procedure DAG semantics in the notation draft than the previous
`Procedure<Subject, Output>`-centric form.

## Compatibility Boundary

The live `ploke-eval protocol tool-call-review` surface still compiles without
requiring a redesign on the `ploke-eval` side.

To preserve that compatibility:

- `StepArtifact.input` and `StepArtifact.output` still expose the raw state
  payloads directly
- state-disposition metadata is carried in separate fields rather than wrapping
  the state payloads at the public access path

This keeps the new semantics while avoiding incidental downstream churn.

## Verification

Completed:

- `cargo fmt --all`
- `cargo check -p ploke-protocol -p ploke-eval`

Observed warnings were pre-existing unrelated warnings in `syn_parser` plus one
pre-existing dead-code warning in `ploke-eval`.

## What Is Better Now

- the crate is no longer pretending intermediate states are merely ordinary
  values
- recording and forwarding are explicit concerns in the execution model
- fork and merge are both first-class composition operations
- branch provenance is preserved structurally in `ForkState`

## What Is Still Missing

- no explicit DAG scheduler or graph node/edge model yet
- merge currently composes procedures, but there is not yet a more general
  branch-indexed graph representation
- no new useful multi-branch protocol has been implemented on top of this
  substrate yet
- `tool_call_review` still asks a weak question even though the architecture is
  now stronger
- no persistence work has been added in `ploke-eval`

## Best Next Step

The next best step is to build a genuinely useful first protocol on top of the
new state/fork/merge substrate.

Good targets remain:

- local marginal usefulness
- redundancy
- recoverability
- search thrash characterization

The important thing is that the next protocol should use the stronger
composition model rather than falling back to a single adjudicated
appropriateness label.
