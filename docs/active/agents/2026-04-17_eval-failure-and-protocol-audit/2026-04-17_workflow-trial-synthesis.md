# 2026-04-17 Workflow Trial Synthesis

- date: 2026-04-17
- purpose: stress-test the proposed protocol-driven diagnosis workflow with
  parallel sub-agents before formalizing it as an explicit operating workflow
- campaign: `rust-baseline-grok4-xai`

## Trial Setup

Six sub-agents were assigned the same basic workflow over different slices:

- `--issue search_thrash`
- `--tool request_code_context`
- `--tool read_file`
- `--issue partial_next_step`
- `--status error`
- `--tool read_file --issue partial_next_step`

Each agent had to:

1. start from campaign triage
2. choose exemplar runs
3. inspect traces
4. inspect the implicated code surface
5. propose `small_slice`, `medium_slice`, and `long_term` interventions
6. recommend one next move and metric

## What Worked

- The workflow is runnable and repeatable for sub-agents without extra
  handholding.
- Every completed report produced a concrete intervention ladder rather than
  only observations.
- Tool-focused slices were the strongest:
  - `request_code_context`
  - `read_file`
  - `read_file + partial_next_step`
- The combined slice was especially useful because it constrained the exemplar
  set tightly enough to keep the diagnosis concrete.
- The `status=error` branch also worked well because the ownership surface was
  obvious: schema/artifact compatibility rather than production tool behavior.

## What Broke

- Issue-only slices (`search_thrash`, `partial_next_step`) drifted toward
  `ploke-eval` / protocol classification and reporting rather than the
  production `ploke-tui` harness.
- This means the workflow is currently under-specified at the point where the
  agent should decide whether the problem is:
  - analysis-surface / aggregation
  - protocol-artifact compatibility
  - production tool / tool-description / recovery-affordance behavior
- Without that explicit branch, agents may inspect the wrong codebase layer and
  still produce a plausible-looking report.

## Main Refinement

The workflow needs an explicit ownership gate after exemplar review and before
code inspection:

1. identify the slice
2. inspect exemplars
3. answer: **what layer actually owns this problem?**
   - `analysis_surface`
   - `artifact_or_schema`
   - `production_harness`
4. only then inspect code in the owning layer
5. build the intervention ladder inside that owning layer

This keeps issue families from defaulting to the nearest visible `ploke-eval`
surface when the real question is about `ploke-tui` tool behavior.

## Refined Workflow Shape

### Phase 1: triage

- start from `ploke-eval inspect protocol-overview --campaign <id>`
- choose one bounded slice:
  - `--tool`
  - `--issue`
  - `--tool` + `--issue`
  - `--status`

### Phase 2: exemplars

- inspect 2-5 exemplar runs
- review trace shape and local evidence

### Phase 3: ownership gate

Answer explicitly:

- is this mainly an `analysis_surface` problem?
- is this mainly an `artifact_or_schema` problem?
- is this mainly a `production_harness` problem?

Only after that answer should the agent inspect code.

### Phase 4: code-surface inspection

- inspect only the owning layer
- avoid reading multiple layers unless the evidence actually crosses them

### Phase 5: intervention ladder

Require:

- `small_slice`
- `medium_slice`
- `long_term`
- `recommended_next_step`
- `metric_to_move`

## Recommended Usage Rules

- Prefer tool slices and combined tool+issue slices when the goal is improving
  production tools.
- Use issue-only slices when the goal is calibrating analysis or understanding
  cross-tool behavior.
- Use status slices when the goal is unblocking protocol interpretation.
- Require each agent to state the owning layer explicitly before naming the
  code surface.

## Practical Conclusion

The workflow is viable, but the next formal version should not be the original
6-step flow unchanged.

It should become:

`campaign triage -> bounded slice -> exemplar review -> ownership gate -> owning code surface -> intervention ladder`

That extra ownership gate is the main change forced by the trial.
