# 2026-04-13 Inspect CLI Comparison Synthesis

- date: 2026-04-13
- task title: CLI-only vs non-CLI inspection comparison for `BurntSushi__ripgrep-1294`
- task description: preserve the first bounded comparison wave after landing `inspect turn --show loop`, including the mismatch between CLI-only and direct-artifact narratives and the resulting metric/UX follow-up targets
- related planning files: `docs/active/plans/evals/eval-design.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-13_inspect-cli-ux-orchestration-note.md`, `docs/active/agents/2026-04-13_inspect-turns-and-loop-ux-note.md`

## Scope

This note records a three-way comparison over `BurntSushi__ripgrep-1294`:

- CLI-only inspection using `ploke-eval inspect`
- non-CLI direct artifact inspection
- metrics-focused CLI audit against `eval-design.md`

The goal was not to adjudicate the run completely, but to identify what the CLI
surfaces well, what it undersurfaces, and whether the CLI materially improves
diagnostic work.

## High-Signal Findings

### 1. The CLI is now materially useful for narrative reconstruction

The CLI-only report used:

- `inspect turns`
- `inspect config`
- `inspect turn 1 --show loop`
- `inspect tool-calls`
- targeted `--show tool-result --index N`

That was enough to reconstruct a coherent tool-interaction story and identify:

- repeated missing-file `read_file` failures
- one `non_semantic_patch` format failure
- recovery into a staged single-file patch

This supports the current command ladder direction:

- compact selection surface
- turn summary
- loop view
- full single-call drilldown only when needed

### 2. Direct artifact inspection still exposes important details the CLI does not surface well

The non-CLI report found:

- exact prompt construction and RAG composition
- prompt/context token estimates
- exact patch expectation hashes
- run-directory timestamp skew and mixed-history risk

The CLI does not currently provide strong first-class access to:

- token usage or token estimates
- prompt/RAG summary fields
- clear run-history linearity / stale-artifact warnings
- exact patch expectation state beyond indirect drilldown

### 3. The comparison surfaced a real record-selection or mixed-history mismatch

The CLI-only and non-CLI reports did not converge on the same semantic story for
`BurntSushi__ripgrep-1294`.

CLI-only saw:

- one completed turn
- 31 tool calls
- 5 failed tool calls
- recovered patch attempt

Non-CLI saw:

- one aborted turn
- no tool calls
- no patch attempt
- completed setup artifacts plus older parse-failure history in the same directory

This is a first-class finding. At least one of these must be true:

- the CLI default selection is reading a different record layer than the direct artifact pass
- the run directory contains mixed-history artifacts that make direct inspection ambiguous
- the current inspect surfaces collapse or abstract over multiple record sources in a way that is not obvious to users

Before treating either path as canonical for this run, this discrepancy should be resolved explicitly.

## Metrics Assessment

Against `eval-design.md`, the current CLI is strongest on:

- historical lookup/replay-style answerability
- chronological tool-flow reconstruction
- per-call failure inspection

It is weaker on first-class outcome and health metrics:

- `solve_rate`
  - inferable, but not summarized clearly
- `wall_time`
  - reconstructible from timestamps, not surfaced directly
- `token_cost`
  - not surfaced in the CLI
- tool-call failure rate
  - manually reconstructible, not aggregated
- recovery success rate
  - inferable narratively, not aggregated
- attribution coverage / failure labeling
  - not surfaced at the inspect-summary layer
- provider/runtime health
  - provenance exists in some places, but metric-layer visibility is weak

## Recommended Follow-Ups

1. Resolve the `1294` CLI-vs-artifact mismatch before leaning too hard on this run as a showcase for inspect correctness.
2. Add a small inspect-summary surface for core metrics:
   - tokens
   - wall time
   - tool failure count/rate
   - recovery count/rate
   - turn outcome vs tool-failure distinction
3. Consider a prompt/context summary view or compact token/context section on `inspect config` or `inspect turn`.
4. Consider stale/mixed-history signaling when a run directory contains artifacts with conflicting timestamps or phases.

## Resume Point

If resuming this lane:

1. read this note
2. compare it against the active `inspect` CLI output on `BurntSushi__ripgrep-1294`
3. decide whether the next packet should be:
   - record-selection/mixed-history clarification, or
   - first-class metric surfacing in `inspect`
