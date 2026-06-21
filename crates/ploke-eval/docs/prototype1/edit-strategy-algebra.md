# Prototype 1 Edit Strategy Algebra

Status: **In progress**

Last updated: 2026-06-21

Decision record: [ADR 008: Prototype 1 Composable Edit Strategy Algebra](../../../../docs/active/ADRs/008-prototype1-edit-strategy-algebra.md)

## Purpose

Prototype 1 currently has a relatively rigid edit-generation path: prepare a
candidate checkout, call an LLM-powered tool loop, collect whatever patch the
loop produces, validate it, and admit or reject the result. That is sufficient
for the simplest policy, but it makes it expensive to experiment with richer
agent workflows such as review-before-plan, plan-before-edit, multi-agent
implementation, or adjudicated candidate selection.

This document sketches an in-progress design for a composable edit-strategy
layer inside `ploke-eval`. The goal is to make the inner "produce candidate
edits" step modular in the same spirit as Prototype 1's outer typestate edges,
while keeping the strategy pieces easier to compose, configure, and swap than
full loop-phase typestate transitions.

## Design intent

The edit-strategy layer should model edit generation as typed, composable
stages:

```text
Input -> Stage<A> -> Intermediate -> Stage<B> -> Output
```

Examples:

```text
EditContext
  -> ToolLoop
  -> CandidatePatch
```

```text
EditContext
  -> Review
  -> ReviewReport
  -> Plan
  -> EditPlan
  -> FanOut(ImplementPlan)
  -> CandidatePatches
  -> ValidateEach
  -> ValidatedCandidates
  -> SelectAdmitted
  -> ChildPlanInputs
```

Each stage declares the type of input it accepts and the type of output it
produces. Composition is valid only when the previous output type satisfies the
next input type. This gives us the functional-programming shape we want without
making every edit workflow another hard-coded Prototype 1 phase.

## Relationship to existing systems

This layer should not replace the outer Prototype 1 typestate pipeline.

| Layer | Responsibility |
| --- | --- |
| Prototype 1 typestate | Parent lifecycle authority: startup, baseline, policy, child planning, execution, selection, handoff. |
| `ploke-protocol` | Formal procedures and adjudicated protocol artifacts, including typed procedure runs and review procedures. |
| Edit strategy algebra | Configurable inner workflows that produce, validate, and rank edit candidates for a parent/child-plan edge. |

The edit strategy may reuse ideas and types from `ploke-protocol` (`StepSpec`,
`Sequence`, `FanOut`, `Merge`) where helpful, but it should not force every edit
strategy into the same artifact/procedure shape. Its first job is operational:
let Prototype 1 swap between candidate-generation strategies while preserving
authority, validation, and durable evidence.

## Core abstractions

A minimal stage trait can be shaped as a typed arrow:

```rust
trait EditStage {
    type Input;
    type Output;

    async fn run(
        &self,
        ctx: &EditStrategyCtx,
        input: Self::Input,
    ) -> Result<StageRun<Self::Output>, EditStrategyError>;
}
```

`StageRun<T>` should carry both the typed output and durable evidence metadata:

```rust
struct StageRun<T> {
    output: T,
    evidence: StageEvidence,
}
```

A composition helper can encode adjacency:

```rust
struct Then<A, B> {
    first: A,
    second: B,
}

// Valid only when A::Output can feed B::Input.
```

The exact Rust shape can evolve, but the invariant should remain:

> A configured strategy must not call a stage until the required typed inputs
> have been produced by earlier stages or supplied by the parent context.

## Tool surfaces, prompts, and recorded invocation API

A strategy stage that calls an agent should not hand-build all prompt text,
tool lists, persistence paths, and replay metadata at every call site. The
strategy layer needs a small ergonomic API for "run this stage's agent turn and
persist it in the standard ploke-eval way".

The desired call shape is approximately:

```rust
let run = ctx
    .agent_turn("review", ReviewInput::from(context))
    .model(review_model)
    .prompt(review_prompt)
    .tools(ToolAccessProfileRef::ReadOnlyReview)
    .record_under(StageArtifactRef::for_stage("review"))
    .run_json::<ReviewReport>()
    .await?;

let report: ReviewReport = run.output;
let evidence: StageEvidence = run.evidence;
```

This does not prescribe the final API, but it captures the ergonomic goal:
strategy authors should call one helper that handles provider invocation,
tool-loop execution, typed output parsing, and durable records.

### Tool access profile specification

Every agent-calling stage must declare a **tool access profile**: the set of
runtime tools the stage is allowed to call, plus per-tool authority constraints.
This is intentionally not called a "surface" because Prototype 1 already uses
"mutable surface" and "immutable surface" for edit-target authority.

A tool access profile answers:

```text
Which tools may this stage call, and in what modes?
```

A mutable/immutable surface answers:

```text
Which repository/evidence paths may this stage read or mutate?
```

The two are related but distinct. For example, an implementation stage may have
access to `ns_patch`, but `ns_patch` must still be constrained by the current
mutable surface policy and protected roots.

Tool access profiles should be named, reusable, and deny-by-default. A stage may
expose fewer tools than the underlying TUI/runtime supports, but it must not
silently gain tools because a new tool was added elsewhere.

Initial tool access profiles:

| Profile | Intended stages | Allowed behavior |
| --- | --- | --- |
| `read_only_review` | `review`, protocol-style analysis | Read evidence and code context, list directories, inspect prior records; no edits or cargo mutations. |
| `planning_read_only` | `plan` | Same as review plus access to stage outputs such as `ReviewReport`; no edits. |
| `implementation_edit` | `tool_loop`, `implement_plan` | Read context, propose/apply edits through the approved edit tools, and run allowed validation commands. |
| `validation_only` | `validate_each` | Run configured validation/check commands and inspect outputs; no source edits. |

A concrete `ToolAccessProfileSpec` should include at least:

- allowed tool names
- whether filesystem writes are allowed through those tools
- mutable surface policy and protected roots referenced by mutating tools
- validation command allowlist
- read-only evidence roots
- candidate workspace root
- per-tool timeout/attempt limits where applicable

Config should reference named profiles rather than raw tool arrays in most
profiles. Raw tool arrays can be useful for experiments, but the loader should
normalize them into a `ToolAccessProfileSpec` and persist the normalized result.

### Prompt stack specification

Prompting should be compositional in the same way stages are. A stage should be
able to declare a base prompt and a sequence of overlays instead of formatting
one large ad hoc string.

A `PromptStackSpec` should support:

- stable base prompt id, e.g. `prototype1.edit.review.v1`
- optional strategy-level overlay, e.g. `review_plan_fanout.v1`
- stage-specific instructions
- typed input rendering, e.g. `ReviewInput`, `EditPlan`, or `CandidateSet`
- authority/guardrail appendix
- validation requirements
- evidence citations injected from `EditContext`

Persist both:

- prompt ids and content digests, so the run can be compared across versions
- rendered prompt artifact paths when prompts are materialized for an actual
  provider call

Prompt rendering should be deterministic for a fixed input and prompt registry.
If the rendered prompt includes volatile paths or timestamps, those should be
isolated in a clearly labeled runtime section.

### Standard recorded agent turn

The strategy layer should reuse the current evidence shape from live/replayable
headless TUI tool loops rather than inventing a parallel format. For an
agent-calling stage, the recorder should be able to persist or link:

- `*.headless-tui.json` for compact headless run outcome
- `.turn-live/agent-turn-trace.json` for typed turn/tool events
- `.turn-live/agent-turn-summary.json`
- `.turn-live/llm-full-responses.jsonl`
- `debug/tool-loop/<session>/steps/*.json` when response-stepped debugging is
  enabled

The stage evidence should contain typed references to these records, not just
free-form strings. Those references should be compatible with the existing
`record.rs` typed projections over `agent-turn-trace.json` and tool-call
records, so protocol review and replay tools can consume strategy-created runs
without special cases.

A non-agent stage should still use the same recorder family where possible. For
example, `validate_each` can record a stage artifact with command, exit status,
stdout/stderr references, and validation summary even though it does not create
LLM turn records.

## Candidate data types

Initial typed boundaries should stay small and explicit:

- `EditContext`: parent identity, campaign paths, mutable surface policy,
  evidence roots, request metadata, model route source, and child budget hints.
- `ReviewReport`: structured assessment of prior runs, protocol artifacts,
  detected failure modes, and recommended focus.
- `EditPlan`: concrete target files/items, intended change, expected validation,
  and constraints.
- `ToolLoopSpec`: prompt stack, model, tool access profile, max turns,
  validation policy, persistence policy, and timeout budget for one
  implementation attempt.
- `ToolAccessProfileSpec`: normalized tool exposure policy for one stage,
  including allowed tool names and filesystem/mutable-surface authority.
- `PromptStackSpec`: base prompt id, overlays, rendered input sections, and
  prompt content digests.
- `AgentTurnRecordRef`: typed references to headless TUI output,
  `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`,
  and optional response-stepped debugger records.
- `CandidatePatch`: candidate workspace root, changed paths, patch summary,
  tool-loop trace references, and submission artifact references.
- `ValidatedCandidate`: candidate patch plus validation result and admission
  status.
- `CandidateSet`: a collection of validated and rejected attempts with enough
  detail to become child-plan evidence.

The initial implementation does not need all of these as stable public types on
day one. The important part is to avoid flattening everything into untyped JSON
between local Rust stages.

## Built-in stages

Start with a small catalog that can express the current behavior and one richer
workflow.

### `tool_loop`

Current behavior represented as a single stage:

```text
EditContext -> CandidatePatch
```

Configurable fields:

- model route
- max tool turns / attempts
- timeout
- validation command
- mutable surface policy

### `review`

Structured review before planning:

```text
EditContext -> ReviewReport
```

This may use a lightweight protocol/model route to summarize:

- prior parent/child evidence
- rejected attempts
- protocol guidance
- benchmark feedback
- likely edit surface

### `plan`

Plan from context plus review:

```text
(EditContext, ReviewReport) -> EditPlan
```

The plan should be concrete enough to constrain implementation agents but not so
rigid that a small correction requires failing the attempt.

### `implement_plan`

Run a tool loop against an explicit plan:

```text
(EditContext, EditPlan) -> CandidatePatch
```

This can share most execution machinery with `tool_loop` but should receive the
plan as first-class input instead of burying it in prompt text only.

### `fan_out`

Run one stage multiple times over the same input:

```text
A -> Vec<B>
```

Typical use:

```text
FanOut(ImplementPlan, parallel = 5)
```

### `validate_each`

Validate every produced candidate and preserve rejected attempts:

```text
Vec<CandidatePatch> -> CandidateSet
```

Invalid, wrong-target, non-exportable, or failed-validation candidates should be
recorded as evidence unless they violate an authority boundary that must remain
fatal.

### `select_admitted`

Select candidates for child materialization:

```text
CandidateSet -> ChildPlanInputs
```

This is where minimum/maximum child budget checks live for the edit strategy.
The outer child-plan authority still owns final durable admission.

## Configuration model

Rust types can prove static composition, but TOML cannot by itself. Start with
named strategies in config, not arbitrary user-authored graphs.

Example:

```toml
[execution.edit_strategy]
name = "tool_loop"

[execution.edit_strategy.tool_loop]
model = "google/gemini-2.5-pro"
max_turns = 24
timeout_secs = 900
tool_access_profile = "implementation_edit"
prompt = "prototype1.edit.tool_loop.v1"
record_mode = "standard_agent_turn"

[execution.edit_strategy.tool_access_profiles.implementation_edit]
allow_tools = [
  "request_code_context",
  "code_item_lookup",
  "ns_read",
  "ns_patch",
  "cargo",
  "list_dir",
  "create_file",
]
allow_writes = true
validation_commands = ["cargo check", "cargo test"]
```

Richer strategy:

```toml
[execution.edit_strategy]
name = "review_plan_fanout"

[execution.edit_strategy.review]
model = "google/gemini-2.5-flash"
timeout_secs = 300
tool_access_profile = "read_only_review"
prompt = "prototype1.edit.review.v1"
record_mode = "standard_agent_turn"

[execution.edit_strategy.plan]
model = "google/gemini-2.5-flash"
timeout_secs = 300
tool_access_profile = "planning_read_only"
prompt = "prototype1.edit.plan.v1"
record_mode = "standard_agent_turn"

[execution.edit_strategy.implement]
model = "google/gemini-2.5-pro"
parallel = 5
max_turns = 24
timeout_secs = 900
tool_access_profile = "implementation_edit"
prompt = "prototype1.edit.implement_plan.v1"
record_mode = "standard_agent_turn"

[execution.edit_strategy.validate]
command = "cargo check -p ploke-eval"
tool_access_profile = "validation_only"
record_mode = "stage_artifact"

[execution.edit_strategy.tool_access_profiles.read_only_review]
allow_tools = ["request_code_context", "code_item_lookup", "ns_read", "list_dir"]
allow_writes = false

[execution.edit_strategy.tool_access_profiles.planning_read_only]
allow_tools = ["request_code_context", "code_item_lookup", "ns_read", "list_dir"]
allow_writes = false

[execution.edit_strategy.tool_access_profiles.implementation_edit]
allow_tools = [
  "request_code_context",
  "code_item_lookup",
  "ns_read",
  "ns_patch",
  "cargo",
  "list_dir",
  "create_file",
]
allow_writes = true
validation_commands = ["cargo check", "cargo test"]

[execution.edit_strategy.tool_access_profiles.validation_only]
allow_tools = ["cargo", "ns_read", "list_dir"]
allow_writes = false
validation_commands = ["cargo check -p ploke-eval"]
```

Later, after the fixed strategies are stable, add an optional declarative graph
syntax with a runtime type checker:

```toml
[[execution.edit_strategy.stages]]
id = "review"
kind = "review"

[[execution.edit_strategy.stages]]
id = "plan"
kind = "plan"
input = "review"

[[execution.edit_strategy.stages]]
id = "implement"
kind = "fan_out"
stage = "implement_plan"
input = "plan"
parallel = 5
```

The runtime checker must reject any graph where a stage input cannot be
satisfied by prior outputs or by `EditContext`.

## Evidence and failure semantics

Every stage boundary should persist enough evidence to explain the strategy run:

- stage id and kind
- input digest or input artifact references
- output digest or output artifact references
- normalized tool access profile used by the stage
- prompt stack ids, prompt content digests, and rendered prompt references
- model/provider route and request parameters
- standard agent-turn record references where relevant
- validation result
- admission/rejection reason
- timing and timeout metadata

Recording invariants:

1. Agent-calling stages must produce the same class of replayable records as the
   current headless TUI/test-harness loop unless explicitly configured as
   non-recording for a test.
2. Stage records must identify the parent stage, candidate workspace, request id,
   and strategy run id.
3. A rejected candidate must retain its tool-loop and prompt evidence even when
   no child node is materialized.
4. Persisted stage artifacts should use typed refs to existing records instead
   of duplicating large LLM responses.
5. If record persistence fails, the stage should fail hard; silent loss of
   evidence is not an acceptable way to keep the loop moving.

Failure policy should distinguish:

| Failure class | Desired behavior |
| --- | --- |
| Provider/tool transient failure before output | Usually rejected attempt evidence, unless all attempts fail below minimum. |
| Validation failure after patch | Rejected candidate evidence. |
| Wrong-target or non-exportable patch | Rejected candidate evidence if authority is preserved. |
| Authority boundary violation | Fatal unless explicitly downgraded by a reviewed policy. |
| Persistence/schema error | Fatal. |

This preserves the current guardrail: do not weaken correctness or import
semantics just to keep the loop moving.

## Implementation plan

### Phase 0 — Inventory current seams

Goal: identify the minimal insertion point for the first strategy wrapper.

Tasks:

1. Map current broad TUI child-plan path from request publication through
   submitted-result admission and child materialization.
2. Identify the smallest current function boundary that can become
   `tool_loop(EditContext) -> CandidatePatch` without changing behavior.
3. Record existing artifact paths that should become `StageEvidence` references.
4. Identify the current helper(s) that write or expose `.headless-tui.json`,
   `.turn-live/agent-turn-trace.json`, `agent-turn-summary.json`, and
   `llm-full-responses.jsonl`.
5. Inventory current tool exposure in the broad headless TUI path and split it
   into candidate named surfaces: review, planning, implementation, validation.

Verification:

- existing broad harness tests still pass unchanged
- no change to live profile behavior when strategy is omitted or set to
  `tool_loop`
- inventory doc or code comments identify the canonical record writer/references
  that the strategy wrapper must reuse

### Phase 1 — Add typed strategy DTOs and config enum

Goal: make strategy selection explicit while preserving current behavior.

Tasks:

1. Add `EditStrategyConfig` to the Prototype 1 run profile.
2. Add `EditStrategyKind::ToolLoop` as the default.
3. Add typed DTOs for `EditContext`, `CandidatePatch`, `CandidateSet`, and
   `StageEvidence` in a focused module.
4. Add `ToolAccessProfileSpec`, `PromptStackSpec`, `AgentTurnRecordRef`, and
   `RecordMode` DTOs.
5. Wire config loading/validation without changing execution flow yet.
6. Normalize named tool access profiles into explicit allowed-tool lists at
   profile admission time.

Verification:

- profile defaulting test for absent `execution.edit_strategy`
- profile validation rejects unknown strategy names
- profile validation rejects unknown tool names, unknown prompt ids, and a stage
  configured with writes under a read-only surface
- serialization snapshot or focused unit tests for the new config shape

### Phase 2 — Wrap current behavior as `tool_loop`

Goal: prove the algebra can express today's behavior exactly.

Tasks:

1. Introduce a `ToolLoopStage` wrapper around the current headless TUI/tool-loop
   execution path.
2. Add an ergonomic recorded invocation helper, provisionally named
   `run_recorded_agent_turn`, that accepts a stage id, prompt stack, tool
   surface, model route, input payload, and record location.
3. Return `StageRun<CandidatePatch>` for successful submitted edits.
4. Return rejected-attempt evidence for non-admissible edits, preserving current
   below-min behavior.
5. Keep final child-plan authority in the existing admission/materialization
   layer.
6. Ensure `ToolLoopStage` writes or links the standard headless TUI artifacts:
   `.headless-tui.json`, `.turn-live/agent-turn-trace.json`,
   `.turn-live/agent-turn-summary.json`, and `.turn-live/llm-full-responses.jsonl`.

Verification:

- current broad harness tests still pass
- focused regression test: `tool_loop` strategy emits the same admitted/rejected
  evidence shape as the legacy path
- focused regression test: recorded strategy invocation can be consumed by the
  existing `record.rs` typed projection/replay tooling
- live dry-run/preflight can construct the default strategy

### Phase 3 — Add fixed `review_plan_fanout` strategy

Goal: introduce the first non-trivial composition without adding arbitrary graph
config yet.

Tasks:

1. Implement `ReviewStage: EditContext -> ReviewReport` using a read-only tool
   access profile and a registered review base prompt.
2. Implement `PlanStage: (EditContext, ReviewReport) -> EditPlan` using a
   planning read-only tool access profile and a registered planning prompt.
3. Implement `ImplementPlanStage: (EditContext, EditPlan) -> CandidatePatch`
   using the implementation edit access profile and an implementation prompt that
   includes the plan as typed input.
4. Implement `FanOut<ImplementPlanStage>` with bounded parallelism.
5. Reuse existing validation/admission for candidate outcomes.
6. Persist every agent-calling stage through the same recorded invocation helper
   so review, planning, and implementation are all replayable.

Verification:

- unit tests for stage input/output serialization
- strategy test with fake/model-stub stages proving review output is passed to
  plan and plan output is passed to implementation
- tool-access-profile test proving review/plan cannot call edit tools
- prompt-stack test proving prompt ids/digests are persisted in stage evidence
- failure test: one implementation fails, enough remaining candidates pass
- profile fixture for `review_plan_fanout`

### Phase 4 — Persist stage artifacts

Goal: make strategy runs auditable and useful as loop evidence.

Tasks:

1. Add strategy artifact directories under the parent node or message root.
2. Persist per-stage input/output references and summaries.
3. Link strategy artifacts from child-plan evidence and rejected attempts.
4. Add a small inspection command or extend existing summaries to show strategy
   stage outcomes.
5. Define a stable `strategy-run.json` envelope with strategy id, stage list,
   prompt refs, tool-access-profile refs, and child-plan linkage.
6. Define a stable `stage-run.json` envelope for each stage invocation.

Verification:

- artifacts survive process restart
- summary output can identify which stage rejected a candidate
- no large prompt/body blobs are duplicated unnecessarily when existing logs
  already contain them
- inspection can jump from strategy stage to `agent-turn-trace.json` and
  `llm-full-responses.jsonl` when the stage used an agent turn

### Phase 5 — Optional declarative graph registry

Goal: allow advanced configuration after fixed strategies are stable.

Tasks:

1. Register named stage kinds with runtime input/output type ids.
2. Parse a declarative stage graph from config.
3. Type-check graph adjacency before execution.
4. Execute the graph using type-erased stage objects only after validation.

Verification:

- invalid graph is rejected at profile load or preflight
- graph equivalent to `tool_loop` matches fixed `tool_loop` behavior
- graph equivalent to `review_plan_fanout` matches fixed strategy behavior

## Open questions

1. Should this live entirely in `ploke-eval`, or should generic stage/composition
   traits be promoted into `ploke-protocol` after the first implementation?
2. How much of the strategy output should be represented as Rust DTOs versus
   persisted JSON artifacts with typed handles?
3. Should strategy graphs be allowed to branch before we have a stable fixed
   `review_plan_fanout` strategy?
4. Which failures are allowed to become rejected evidence, and which must remain
   fatal authority violations?
5. Should benchmark feedback be a first-class input type, or part of
   `EditContext`?
6. Should prompt definitions live in code, profile files, or a versioned prompt
   registry under `crates/ploke-eval/docs`/resources?
7. Should named tool access profiles be global Prototype 1 policy,
   strategy-local policy, or both with strategy-local narrowing only?
8. Which record writer should be the single canonical API for agent-turn traces
   so live runs, replay probes, and edit strategies cannot drift?

## Near-term recommendation

Implement the smallest useful vertical slice:

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

Then wire it so the default strategy produces exactly the same behavior as the
current broad harness path. Only after that compatibility slice is verified
should we add `review_plan_fanout`.
