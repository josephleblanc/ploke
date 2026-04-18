# 2026-04-17 Packet P1: Path Recovery And Layout Hints

- task_id: `AUDIT-IMPL-P1`
- title: strengthen missing-file and repo-root/path recovery
- date: 2026-04-17
- owner_role: worker
- layer_workstream: `A4`
- related_hypothesis: many current trace failures are not deep reasoning
  failures but recoverable path/layout misses with weak tool guidance
- design_intent: reduce repeated root/path thrash by making the first failed
  path-oriented tool response more actionable
- scope:
  - improve recoverability for missing-file tool failures
  - improve guidance when a path is outside configured roots or likely has the
    wrong repo-relative anchor
  - consider method/function fallback hints where lookup misses clearly expose
    the mismatch
- non_goals:
  - do not relax path validation or silently reinterpret arbitrary paths
  - do not redesign the whole tool surface in one pass
  - do not weaken correctness guarantees around workspace roots
- owned_files:
  - to be assigned after permission; likely tool and/or error-surface files
    outside `crates/ploke-eval/`
- dependencies:
  - [blind-trace-sample-summary.md](./blind-trace-sample-summary.md)
  - sampled runs:
    - `clap-rs__clap-3700`
    - `clap-rs__clap-5873`
    - `BurntSushi__ripgrep-1980`
    - `sharkdp__bat-1518`
- acceptance_criteria:
  1. a missing-file failure can return at least one concrete next-step hint
     that is materially more specific than the current bare I/O error
  2. repeated root/path confusion cases from the sampled traces have a clearer
     first-failure recovery path
  3. any added hinting remains strict and does not silently mutate requested
     paths
- required_evidence:
  - before/after CLI trace or fixture evidence on at least two sampled failure
    shapes
  - file/line references for the changed recovery surface
  - explicit statement that validation strictness was preserved
- report_back_location:
  - this audit directory plus a bounded implementation report
- status: `ready`

## Motivation

The blind sample converged on path/layout recovery as the dominant issue:

- `clap-rs__clap-3700`
- `clap-rs__clap-5873`
- `BurntSushi__ripgrep-1980`
- `sharkdp__bat-1518`

These runs often recovered eventually, which makes this a high-leverage
recoverability packet rather than a pure model-strategy complaint.
