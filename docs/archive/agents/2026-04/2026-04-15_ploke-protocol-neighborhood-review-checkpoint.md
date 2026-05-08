# Ploke-Protocol Neighborhood Review Checkpoint

- date: 2026-04-15
- task title: ploke-protocol neighborhood review checkpoint
- task description: third architectural checkpoint for `crates/ploke-protocol`, replacing the weak single-call review with a richer adapter-backed neighborhood procedure composed from mechanized context, forked adjudication branches, and explicit merge
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_ploke-protocol-state-composition-checkpoint.md`, `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`

## What Changed

This slice implemented the near-term milestone that had been identified after
the state-composition rewrite:

1. richer projected state than the old flat `Trace`
2. mechanized decomposition before adjudication
3. forked adjudication over distinct local questions
4. explicit merge into one structured local assessment
5. a real adapter boundary from `ploke-eval`

The old `tool_call_review` flow was effectively:

- select one indexed call
- ask one weak appropriateness question

The new flow is:

- project a bounded same-turn neighborhood around a focal tool call
- derive mechanized neighborhood signals
- adjudicate three distinct questions over the same bounded context
- merge those branch results into one structured local assessment

## Current Module Shape

The main protocol substrate files remain:

- `crates/ploke-protocol/src/core.rs`
- `crates/ploke-protocol/src/step.rs`
- `crates/ploke-protocol/src/procedure.rs`
- `crates/ploke-protocol/src/llm.rs`

The main concrete procedure files for this slice are now:

- `crates/ploke-protocol/src/tool_calls/trace.rs`
- `crates/ploke-protocol/src/tool_calls/review.rs`

The `ploke-eval` adapter and CLI integration remain in:

- `crates/ploke-eval/src/cli.rs`

## New Protocol-Facing State Surface

`tool_calls::trace` now contains a richer protocol-facing state family:

- `NeighborhoodRequest`
- `NeighborhoodSource`
- `ToolCallNeighborhood`
- `TurnContext`
- `NeighborhoodCall`
- `ToolKind`

This keeps `ploke-protocol` decoupled from `RunRecord` while still letting
`ploke-eval` project concrete eval artifacts into admissible procedure states.

The old `Trace` type still exists as a reduced surface, but the live review
protocol no longer depends on it.

## New Review Procedure Shape

The review procedure now composes these states and steps:

1. `ContextualizeNeighborhood`
   - mechanized
   - input: `ToolCallNeighborhood`
   - output: `ReviewContext`
   - derives typed local signals such as repeated-tool clusters, similar-search
     counts, later reads, later searches, and candidate concerns

2. `AssessLocalUsefulness`
   - LLM adjudicated
   - input: `ReviewContext`
   - output: `UsefulnessAssessment`

3. `AssessRedundancy`
   - LLM adjudicated
   - input: `ReviewContext`
   - output: `RedundancyAssessment`

4. `AssessRecoverability`
   - LLM adjudicated
   - input: `ReviewContext`
   - output: `RecoverabilityAssessment`

5. `AssembleAssessment`
   - mechanized merge
   - input: nested branch-preserving `ForkState`
   - output: `LocalToolCallAssessment`

This means the first real protocol now uses:

- typed intermediate states
- forked execution
- explicit merge
- preserved branch provenance
- distinct evidential questions rather than one generic label

## Current Output Surface

`LocalToolCallAssessment` now includes:

- the bounded neighborhood
- mechanized local signals
- usefulness assessment
- redundancy assessment
- recoverability assessment
- merged overall verdict and confidence
- synthesis rationale

This makes the output materially closer to the older CLI-first useful workflow,
because it can now represent local sequence structure and not just one isolated
tool call.

## Adapter Boundary

`ploke-eval` now provides a concrete adapter path through a local
`NeighborhoodSource` implementation that projects `RunRecord` into
`ToolCallNeighborhood`.

Important properties of the current adapter:

- bounded to the focal call's turn rather than the whole run
- keeps global stable tool-call index for the focal call
- preserves same-turn neighborhood ordering
- derives compact normalized fields such as:
  - tool kind
  - argument preview
  - result preview
  - `search_term`
  - `path_hint`

This is the first real protocol-facing adapter boundary rather than ad hoc
subject-building inside the protocol crate itself.

## CLI Surface

The command remains:

- `ploke-eval protocol tool-call-review`

But it no longer represents a one-shot isolated-call review. The table output
now shows:

- focal neighborhood window
- mechanized signals
- branch-local assessments
- merged overall assessment

## Verification

Completed:

- `cargo fmt --all`
- `cargo check -p ploke-protocol -p ploke-eval`
- `cargo run -p ploke-eval -- protocol --help`

Observed warnings were pre-existing unrelated warnings in `syn_parser` plus one
pre-existing dead-code warning in `ploke-eval`.

## What Is Better Now

- the first real protocol is no longer weaker than the architecture beneath it
- `ploke-protocol` now has a concrete example of adapter-backed projected state
- fork/merge semantics are exercised by a useful procedure, not only toy tests
- the procedure is materially closer to the older `inspect tool-calls` plus
  `inspect turn --show loop` workflow because it preserves local sequence
  context

## What Is Still Missing

- protocol procedure artifacts are still not persisted beside the run
- there is still only one substantial protocol on the new substrate
- the adapter boundary is real but still lives inside CLI code rather than a
  clearer reusable `ploke-eval` adapter surface
- merge is explicit, but there is still no more general graph scheduler or
  branch-indexed DAG execution model
- there is no turn-level or run-level aggregation procedure yet
- there is no calibration or disagreement surface yet

## Best Next Step

The near-term milestone is complete enough that the next work should move toward
the long-view `10` target without letting the current architecture drift back
toward one-off commands.

Best next implementation order:

1. move the `NeighborhoodSource` projection into a clearer reusable
   `ploke-eval` adapter module rather than leaving it in CLI code
2. add a second bounded protocol on the same projected neighborhood state
3. persist protocol procedure artifacts beside the run
4. add a bounded aggregation procedure from local assessments toward turn-level
   or run-level supporting states
5. begin adding calibration/disagreement surfaces so the path toward the long
   `10` milestone stays explicit
