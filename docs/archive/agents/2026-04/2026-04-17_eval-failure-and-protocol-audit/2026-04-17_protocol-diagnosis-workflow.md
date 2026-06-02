# 2026-04-17 Protocol Diagnosis Workflow

- date: 2026-04-17
- task title: protocol-driven diagnosis workflow for production harness improvement
- task description: repeatable workflow for turning campaign protocol data into
  bounded diagnosis, exemplar review, ownership identification, and a ranked
  intervention ladder for `ploke` / `ploke-tui` improvement work
- related planning files:
  - [packet-P0-campaign-protocol-triage.md](./packet-P0-campaign-protocol-triage.md)
  - [2026-04-17_workflow-trial-synthesis.md](./2026-04-17_workflow-trial-synthesis.md)
  - [2026-04-17_protocol-triage-implementation-report.md](./2026-04-17_protocol-triage-implementation-report.md)

## Purpose

This workflow exists to answer:

> what is the next worthwhile fix in `ploke`, why does it matter, and what is
> the smallest useful slice versus the stronger long-term architecture move?

It is designed to be:

- repeatable by a human operator
- repeatable by sub-agents
- compact enough to avoid JSON/JQ spelunking
- explicit enough to prevent drift into the wrong code layer

## Happy Path

The intended flow is:

`campaign triage -> bounded slice -> exemplar review -> ownership gate -> owning code surface -> intervention ladder`

## Slice Selection Rules

Choose one bounded slice before reviewing exemplars.

Preferred slice types:

- `--tool`
  Use when the goal is production-tool improvement.
- `--tool` + `--issue`
  Use when the goal is diagnosing one concrete behavior within one tool family.
- `--status`
  Use when the goal is unblocking interpretation, artifact loading, or schema
  compatibility.
- `--issue`
  Use only when the goal is cross-tool behavior analysis or analysis-surface
  calibration.

Rule of thumb:

- prefer `--tool` or `--tool` + `--issue` for `ploke-tui` improvement work
- prefer `--status error` for artifact/schema failure work
- use pure `--issue` slices carefully, because they tend to drift toward
  `ploke-eval` / protocol-analysis code unless ownership is made explicit

## Commands

Start from the campaign dashboard:

```bash
ploke-eval inspect protocol-overview --campaign <campaign>
```

Then choose a bounded slice:

```bash
ploke-eval inspect protocol-overview --campaign <campaign> --tool <tool>
ploke-eval inspect protocol-overview --campaign <campaign> --issue <issue>
ploke-eval inspect protocol-overview --campaign <campaign> --tool <tool> --issue <issue>
ploke-eval inspect protocol-overview --campaign <campaign> --status <status>
```

Inspect exemplars:

```bash
ploke-eval inspect protocol-overview --instance <run>
ploke-eval inspect tool-calls --instance <run>
ploke-eval inspect protocol-artifacts --instance <run> --full
```

Use `protocol-artifacts` only when the slice is artifact/schema oriented or the
run-level overview cannot load.

## Sub-Agent Workflow

### Step 1: campaign triage

- run the campaign dashboard
- choose one bounded slice
- state why this slice matters in terms of:
  - affected runs
  - affected calls
  - likely payoff

### Step 2: exemplar review

- inspect `2-5` exemplar runs
- prefer the top exemplars suggested by the campaign slice
- identify repeated trace patterns, not just one-off mistakes

### Step 3: ownership gate

Before inspecting code, the agent must answer:

- is this mainly an `analysis_surface` problem?
- is this mainly an `artifact_or_schema` problem?
- is this mainly a `production_harness` problem?

This step is mandatory.

Without it, issue-only slices tend to drift into the nearest visible
`ploke-eval` surface even when the real problem is in `ploke-tui`.

### Step 4: owning code surface

Inspect only the layer chosen at the ownership gate.

Typical ownership targets:

- `analysis_surface`
  - `crates/ploke-eval/`
  - aggregate/reporting/classification logic
- `artifact_or_schema`
  - producer/consumer boundaries
  - persisted artifact schema and compatibility surfaces
- `production_harness`
  - `crates/ploke-tui/`
  - tool definitions
  - tool descriptions
  - result formatting
  - recovery affordances
  - tool-selection contracts

Inspect multiple layers only when the evidence explicitly crosses layers.

### Step 5: intervention ladder

Every report must propose:

- `small_slice`
  Minimal localized change with large expected payoff.
- `medium_slice`
  Stronger behavioral improvement without major architecture change.
- `long_term`
  Structural or architectural improvement with larger refactor/design cost.
- `recommended_next_step`
  The best current move, given leverage and implementation cost.
- `metric_to_move`
  The measurement expected to change if the intervention works.

### Step 6: bounded report

Every report must stay concise and use the fixed schema below.

## Required Report Schema

Use these top-level headings:

- `Scope`
- `Claims`
- `Evidence`
- `Unsupported Claims`
- `Not Checked`
- `Risks`

Inside `Claims` / `Evidence`, always include:

- `target_family`
- `why_this_family`
- `observed_pattern`
- `suspected_root_cause`
- `ownership`
- `code_surface`
- `small_slice`
- `medium_slice`
- `long_term`
- `recommended_next_step`
- `metric_to_move`
- `confidence`
- `exemplar runs reviewed`

## Orchestrator Workflow

The orchestrator uses the same workflow, but with two extra responsibilities:

1. compare reports for convergence
   - do different sub-agents pick the same owning layer?
   - do they identify the same root-cause shape?
   - do their `small_slice` recommendations converge?
2. convert the winning recommendation into an implementation target
   - packet
   - likely files/modules
   - expected metric movement
   - verification plan

The orchestrator should not collapse distinct ownership surfaces into one
implementation packet unless the evidence actually supports that merge.

## When The Workflow Is Working

The workflow is successful when it gets from campaign data to:

- one bounded problem family
- a small set of exemplar runs
- one explicit owning layer
- one concrete next implementation move
- one metric worth watching after the fix

If the output still requires reading large JSON blobs or manually reconstructing
the problem family from many unrelated runs, the workflow has failed.

## Failure Modes To Watch

- choosing a slice that is too broad
- reviewing too many exemplars
- skipping the ownership gate
- reading `ploke-eval` because it is nearby, even when the fix belongs in
  `ploke-tui`
- proposing only long-term architecture work with no practical next move
- naming a metric that does not actually correspond to the slice being fixed

## Practical Defaults

If unsure:

- start with `--tool` or `--tool` + `--issue`
- review `3` exemplars
- force the ownership gate before code inspection
- prefer the smallest credible intervention that could materially move the
  selected slice

This default is the most reliable path for production harness/tool improvement
work.
