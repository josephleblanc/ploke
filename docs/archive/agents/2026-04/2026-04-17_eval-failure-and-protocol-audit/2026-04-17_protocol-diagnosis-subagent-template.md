# 2026-04-17 Protocol Diagnosis Sub-Agent Template

- date: 2026-04-17
- task title: reusable sub-agent launch template for protocol-driven diagnosis
- task description: copyable prompt template for running the protocol diagnosis
  workflow against one bounded campaign slice with a fixed report schema
- related planning files:
  - [2026-04-17_protocol-diagnosis-workflow.md](./2026-04-17_protocol-diagnosis-workflow.md)
  - [2026-04-17_workflow-trial-synthesis.md](./2026-04-17_workflow-trial-synthesis.md)

## Purpose

Use this template when launching a sub-agent to diagnose one bounded protocol
slice and propose a ranked intervention ladder.

This template exists to make the workflow repeatable and to force the
ownership-gate decision before the agent starts reading code.

## Launch Template

Replace the bracketed placeholders before sending:

```text
You are running a bounded protocol-diagnosis pass in /home/brasides/code/ploke.

You are not alone in the codebase.
Do not revert anyone else's changes.
Do not modify any files except your assigned report file.

Your only owned write target is:
[REPORT_PATH]

Task:
Follow the protocol diagnosis workflow on this campaign slice:
[SLICE_COMMAND]

Goal:
Use campaign protocol data to diagnose the slice, inspect exemplar runs,
identify the owning layer, inspect only that owning code surface, and propose a
ranked intervention ladder from small high-leverage fixes to stronger
long-term architecture changes.

Workflow:
1. Start from the campaign triage command for your slice.
2. State why this slice matters using the aggregate output.
3. Inspect 2-5 exemplar runs using:
   - `ploke-eval inspect protocol-overview --instance <run>`
   - `ploke-eval inspect tool-calls --instance <run>`
   - `ploke-eval inspect protocol-artifacts --instance <run> --full` only if the slice is artifact/schema oriented or the overview cannot load.
4. Before inspecting code, make an explicit ownership call:
   - `analysis_surface`
   - `artifact_or_schema`
   - `production_harness`
5. Inspect only the owning code surface unless the evidence clearly crosses layers.
6. Propose a ranked intervention ladder:
   - `small_slice`
   - `medium_slice`
   - `long_term`
   - `recommended_next_step`
   - `metric_to_move`

Use these slice-selection expectations:
- prefer `--tool` or `--tool` + `--issue` for production harness/tool improvement
- prefer `--status error` for protocol/schema/artifact failures
- treat pure `--issue` slices carefully and do not default to `ploke-eval` ownership without explicit evidence

Write a concise markdown report to your assigned file with exactly these top-level headings:
- Scope
- Claims
- Evidence
- Unsupported Claims
- Not Checked
- Risks

Inside Claims/Evidence, always include:
- target_family
- why_this_family
- observed_pattern
- suspected_root_cause
- ownership
- code_surface
- small_slice
- medium_slice
- long_term
- recommended_next_step
- metric_to_move
- confidence
- exemplar runs reviewed

Keep the report concise and concrete.
Do not implement fixes.
Do not broaden the slice.
Use commands and code inspection only, plus writing the report.
```

## Recommended Placeholder Values

- `[SLICE_COMMAND]`
  One bounded command such as:
  - `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --tool request_code_context`
  - `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --tool read_file --issue partial_next_step`
  - `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --status error`
- `[REPORT_PATH]`
  One file in this audit directory with a unique date/topic-specific name.

## Good Defaults

If the orchestrator is unsure how to scope the run:

- choose a `--tool` or `--tool` + `--issue` slice
- review `3` exemplars
- ask for one recommended next move only
- make the metric correspond directly to the selected slice

## Optional Orchestrator Prefix

If you want slightly tighter steering, add this short prefix ahead of the
template body:

```text
Bias toward production `ploke-tui` ownership when the evidence supports it.
Only choose `analysis_surface` ownership if the main problem is genuinely in
protocol aggregation, calibration, or reporting rather than in the production
tools.
```

## Anti-Patterns

Do not launch with:

- multiple unrelated slices in one prompt
- no owned write target
- no ownership gate
- a broad “investigate everything” instruction
- a prompt that asks for implementation rather than diagnosis

Those patterns made the workflow less reliable during the trial.
