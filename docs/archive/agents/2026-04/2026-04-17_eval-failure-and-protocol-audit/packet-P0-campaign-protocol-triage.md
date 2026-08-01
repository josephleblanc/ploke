# 2026-04-17 Packet P0: Campaign Protocol Triage Surface

- task_id: `AUDIT-IMPL-P0`
- title: build a campaign-scoped protocol triage surface for tool/harness improvement
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A4`
- related_hypothesis: the current protocol data is rich enough to guide
  production `ploke` / `ploke-tui` tool improvements, but `ploke-eval` does not
  yet compress that data into a usable operator decision surface
- design_intent: turn existing machine-readable protocol and closure outputs
  into a compact, campaign-aware CLI surface that helps an operator identify the
  biggest tool-friction problems, inspect representative exemplars, and leave
  with an evidence-backed hypothesis to test in production
- scope:
  - define the operator happy path from campaign data to next implementation
    hypothesis
  - add a campaign-scoped aggregate surface for protocol outcomes and issue
    families
  - group runs by actionable problem families rather than only printing one row
    per run
  - include representative exemplars and follow-up command hints at each stage
  - keep the output visual and fast to parse: bars, ranked lists, compact tables
- non_goals:
  - do not redesign `ploke-protocol` procedures in this slice
  - do not change `ploke-tui` tools yet
  - do not require the operator to read large JSON blobs or chain many `jq`
    queries for common triage flows
- owned_files:
  - `crates/ploke-eval/src/cli.rs`
  - supporting `ploke-eval` aggregate/report modules as needed
- dependencies:
  - [blind-trace-sample-summary.md](./blind-trace-sample-summary.md)
  - [packet-P3-protocol-calibration.md](./packet-P3-protocol-calibration.md)
  - `closure status --campaign ... --format json`
  - `inspect protocol-overview --all-runs --format json`
- report_back_location:
  - this audit directory plus a bounded implementation report
- status: `implemented_self_checked`
- implementation_report:
  [2026-04-17_protocol-triage-implementation-report.md](./2026-04-17_protocol-triage-implementation-report.md)

## Problem Statement

The current `inspect protocol-overview --all-runs` surface aggregates the
existence of run summaries, but not the meaning of the protocol outputs. It
does not answer the real operator question:

> what is the biggest tool/harness problem, where does it show up, and what
> should I inspect next to turn that into a fix?

The result is a dead zone between raw artifacts and actionable production work.
The system has the data, but the operator still has to reconstruct the
middle-layer analysis by hand.

## Operator Happy Path

The intended operator flow is:

1. start at a campaign triage dashboard
2. identify dominant problem families
3. choose one family and inspect representative exemplars
4. inspect the local trace shape around the family
5. leave with a concrete hypothesis and likely fix surface

That flow should feel like:

`campaign summary -> dominant issue family -> exemplar runs -> trace pattern -> fix hypothesis -> implementation target`

## Desired CLI Experience

### Step 1: campaign triage

One command should show a campaign-scoped protocol triage summary with:

- protocol-eligible vs full vs partial vs failed vs ineligible counts
- issue kinds ranked by frequency
- issue tools ranked by frequency
- top failure families ranked by affected runs
- a short “next moves” footer

The footer should include follow-up hints such as:

- `if you want exemplar runs for the top issue family, try ...`
- `if you want only artifact/schema failures, try ...`
- `if you want completed runs with the highest issue density, try ...`

### Step 2: family drilldown

The operator should then be able to ask for one issue family or one
tool-friction family and get:

- count of affected runs
- count of affected calls
- common segment labels / statuses nearby
- representative runs
- the top tools implicated in that family
- a short “next moves” footer

Examples:

- `search_thrash`
- `partial_next_step`
- `read_file`
- `code_item_lookup`
- `read_file + partial_next_step`

### Step 3: exemplar drilldown

For a chosen exemplar run, the operator should get:

- run-level protocol overview
- only the relevant segments / calls for the chosen family where possible
- the actual tool-call sequence around those calls
- a short “next moves” footer pointing to:
  - `inspect tool-calls`
  - `inspect protocol-artifacts`
  - related family views

### Step 4: hypothesis handoff

The operator should be able to stop after a small number of exemplars with:

- suspected problem family
- why it matters
- representative runs
- likely owning surface
  - `ploke-eval` aggregate/report plumbing
  - `ploke-protocol` artifact/schema compatibility
  - `ploke-tui` tool behavior / tool descriptions / recovery affordances
- what metric should move after the fix

## Required Output Shapes

At minimum, the campaign triage surface should expose:

- campaign evidence reliability
- campaign issue surface
- campaign tool-friction surface
- top problem families
- representative exemplars
- next-step commands

### Campaign evidence reliability

Compact bars/tables for:

- reviewed calls / total calls
- usable segment reviews / total segments
- mismatch / missing / failed / ineligible counts
- artifact/schema failure counts

### Campaign issue surface

Compact bars/tables for:

- issue kinds ranked by count
- issue tools ranked by count
- optionally issue kind × tool counts if still compact

### Problem families

Families should distinguish at least:

- artifact/schema failures
- missing review coverage
- high-frequency issue families on completed runs
- ineligible runs

The surface must separate “analysis plumbing is broken” from “the harness/tool
behavior is bad”.

## Suggested Command Model

Exact naming is flexible, but the operator model should look roughly like:

```text
ploke-eval inspect protocol-overview --campaign <id>
ploke-eval inspect protocol-overview --campaign <id> --family <family>
ploke-eval inspect protocol-overview --campaign <id> --tool <tool>
ploke-eval inspect protocol-overview --campaign <id> --issue <kind>
ploke-eval inspect protocol-overview --campaign <id> --examples
```

If campaign-aware `inspect` is not the right entrypoint, add a new campaign
subcommand instead. The important part is the operator happy path, not the
specific verb.

## Why This Matters For Production `ploke`

The goal is not to admire evals. The goal is to improve the user-facing
application.

This surface should help the operator conclude things like:

- there are no recovery affordances after failed searches
- a tool description is misleading the model
- a tool result shape does not provide enough guidance for the next step
- the model churns around a specific tool because no follow-up suggestion exists

If the CLI cannot efficiently support those conclusions, then the eval framework
is not yet paying off for tool/harness improvement.

## Acceptance Criteria

1. one campaign-scoped command produces a compact protocol triage dashboard
   without requiring the operator to read large JSON blobs or hand-roll `jq`
   pipelines
2. the dashboard distinguishes:
   - artifact/schema failures
   - missing review coverage
   - completed-but-high-friction runs
   - ineligible runs
3. the dashboard includes ranked issue kinds and ranked issue tools
4. the dashboard includes representative exemplar runs for at least the top
   problem families
5. each major step includes short “if you want X next, try Y” follow-up hints
6. the operator can go from campaign data to a plausible tool/harness fix
   hypothesis in a bounded number of commands

## Suggested Implementation Slices

### Slice 1: campaign triage dashboard

Build the compact campaign-level summary surface first.

### Slice 2: family grouping

Add grouping by issue family and tool-friction family with exemplar selection.

### Slice 3: exemplar-aware drilldown

Connect family views to per-run drilldown and follow-up command hints.

### Slice 4: polish and command hints

Tune ranking, visual shape, and bottom-of-step suggestions for next commands.

## Evidence To Show When Reporting Back

- before/after examples of the operator flow for:
  - finding the top current protocol problem family
  - finding the top completed-run tool-friction family
  - drilling into one exemplar and identifying a plausible fix hypothesis
- exact command list for the happy path after implementation
- note whether the final command shape lives under `inspect protocol-overview`
  or a new campaign-scoped command family
