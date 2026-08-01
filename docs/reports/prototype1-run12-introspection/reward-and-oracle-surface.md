# Prototype 1 Run 12: Reward And Oracle Surface

## Scope

This report looks for existing score, report, visualization, and oracle-adjacent
code that can be reused after the successful bounded run
`p1-3gen-15nodes-run12`.

It does not inspect the run artifacts directly. The paired CLI and persisted
data reports cover that.

## Existing Reward Signal

The current Prototype 1 keep/reject decision is a proxy comparison over
operational run metrics.

Relevant code:

- `crates/ploke-eval/src/branch_evaluation.rs`
- `crates/ploke-eval/src/operational_metrics.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`

`evaluate_branch` compares baseline and treatment metrics and returns
`BranchDisposition::{Keep, Reject}`. Lower is better for:

- `partial_patch_failures`
- `same_file_patch_retry_count`
- `same_file_patch_max_streak`

Boolean preferences:

- prefer `aborted_repair_loop = false`
- prefer `convergence = true`
- prefer `oracle_eligible = true`

`OperationalRunMetrics` derives those fields from `record.json.gz`. The most
important limitation is explicit in the code: `convergence` is still a
pre-oracle proxy, and `oracle_eligible` only means the run converged and wrote a
nonempty submission artifact. It is not an external benchmark verdict.

## Existing Reporting / Visualization Code

There is already useful report rendering code for bars, issue counts, and
evidence reliability:

- `crates/ploke-eval/src/protocol_report.rs`
  - `progress_bar`
  - `segmented_status_bar`
  - issue/tool count charts
  - evidence reliability sections
- `crates/ploke-eval/src/protocol_triage_report.rs`
  - campaign-level evidence reliability
  - run status counts
  - call/segment review coverage bars
  - issue surface counts
- `crates/ploke-eval/src/cli.rs`
  - table/json output patterns through `InspectOutputFormat`
  - many `inspect` subcommands over `record.json.gz`, protocol artifacts, and
    tool reviews

This can be reused for a Prototype 1 lineage report. The shape should be a
small set of run-specific panels rather than another generic monitor:

```text
Lineage
  gen0 parent -> gen1 selected -> gen2 selected -> gen3 selected -> stop

Candidate surface
  generation | candidates | selected | frontier | completed | failed

Evaluation surface
  branch | disposition | reasons | baseline record | treatment record

Operational metrics
  tool calls | failed tool calls | patch attempts | patch failures | oracle eligible

Handoff surface
  predecessor | successor | ready ack | completion | active checkout commit
```

## Existing Oracle / Dataset Surface

The crate already has Multi-SWE-bench dataset and submission plumbing:

- `crates/ploke-eval/src/campaign.rs`
  - campaign manifests carry benchmark family and dataset sources
  - campaign resolution supports dataset labels and policy filters
- `crates/ploke-eval/src/target_registry.rs`
  - registry for Multi-SWE-bench Rust datasets
  - dataset/source/instance inventory
- `crates/ploke-eval/src/spec.rs`
  - dataset JSONL loading and instance lookup
- `crates/ploke-eval/src/runner.rs`
  - writes `record.json.gz`
  - writes `multi-swe-bench-submission.jsonl`
  - records turn summaries and full-response traces
- `crates/ploke-eval/src/record.rs`
  - `RunRecord` stores benchmark metadata, phases, tool calls, packaging, and
    outcome summary

The current missing piece is an external verifier/oracle adapter that consumes a
candidate submission and returns a grounded verdict. The internal signal already
has a field named `oracle_eligible`, but that is only a gate saying "this run
produced a candidate worth sending to an oracle."

## Editing Surface

The loop currently hillclimbs over a fixed text-file surface. That is useful for
proving the loop but too small for the intended tool-improvement engine.

The future surface should be:

```text
Runtime R
  -> bounded Surface(Artifact A)
  -> PatchAttempt
  -> derived Artifact A'
  -> hydrated/evaluated Runtime R'
```

The ploke editing engine already has relevant pieces:

- `crates/ploke-tui/src/rag/tools.rs`
  - edit proposal generation
  - patch staging
  - preview generation
- `crates/ploke-tui/src/tools/code_edit.rs`
  - code edit tool result shape
- `crates/ploke-tui/src/tools/ns_patch.rs`
  - namespace patch tool result shape
- `crates/ploke-tui/src/app_state/core.rs`
  - edit proposals, preview modes, diff/codeblock preview data

The Prototype 1 boundary should become a trait over `Surface(Artifact)` and
patch application, not a hard-coded `include_str!` text replacement.

## Recommended Next Report Command

Add a finite command, not a watcher:

```text
ploke-eval loop prototype1-report --campaign <id> lineage
```

Minimum output:

- stop reason and policy
- generation lineage
- candidate counts per generation
- selected branch per generation
- evaluation disposition/reasons
- baseline/treatment run record paths
- operational metrics table
- handoff/active-checkout commits

Useful options:

- `--format table|json`
- `--generation <n>`
- `--node-id <id>`
- `--include-stream-errors`
- `--include-record-paths`

## Main Gap

The loop now runs, but the generated evidence is not yet shaped around the
semantic spine. The next improvement should not be another broad record file.
It should be a reader that reconstructs:

```text
R(A) -> P
P(A) -> A'
A' -> R'
Eval(R') -> H
Policy(H...) -> selected successor
Crown handoff -> next Parent
```

Once that report exists, reward refinement and Oracle integration become much
less blind.
