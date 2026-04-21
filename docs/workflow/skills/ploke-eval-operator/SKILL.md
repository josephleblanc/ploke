---
name: ploke-eval-operator
description: Use this skill when you need to operate `ploke-eval` from the CLI without re-reading its source, especially to choose the right command path, treat the right artifacts as authoritative, avoid misleading batch assumptions, and recover a sane eval workflow.
---

# Ploke Eval Operator

Use this skill when the task is to run, inspect, export, or reason about evals through
the `ploke-eval` CLI.

## Boundaries

- Start from the CLI surface first: `ploke-eval --help` and `ploke-eval help <subcommand>`.
- Do not read `crates/ploke-eval/src/` unless the CLI surface fails to answer the operator
  question and the task explicitly requires code-level verification.
- Prefer one target family at a time for interactive work.
- Treat per-run artifacts as more trustworthy than batch aggregate artifacts.
- If the task is about a run trace, pair this skill with `cli-trace-review`.
- If the task is about a formal measured run, pair this skill with `run-protocol`.

## Default Operator Stance

- Default execution primitive: `run-msb-agent-single`
- Default setup path:
  1. `ploke-eval list-msb-datasets`
  2. `ploke-eval fetch-msb-repo --dataset-key <key>`
  3. `ploke-eval model current`
  4. `ploke-eval model providers [MODEL_ID]`
  5. `ploke-eval prepare-msb-single --dataset-key <key> --instance <instance-id>`
  6. `ploke-eval run-msb-agent-single --instance <instance-id> [--provider ...]`
- Default inspection path:
  - `ploke-eval conversations --instance <instance-id>`
  - `ploke-eval inspect failures --instance <instance-id>`
  - `ploke-eval inspect tool-calls --instance <instance-id>`
- Default export truth:
  - per-run `multi-swe-bench-submission.jsonl`
  - `campaign export-submissions` when working at campaign scope

## Artifact Truth Hierarchy

Use this order when deciding what to trust:

1. per-run artifacts in the chosen run directory
2. campaign-level export from `campaign export-submissions`
3. closure state for campaign progress
4. batch aggregate `multi-swe-bench-submission.jsonl`
5. terminal summary strings

Operational rule:
- Do not treat a batch aggregate JSONL as authoritative if the batch may have been rerun,
  interrupted, or overlapped with another live batch process.

## Command Groups That Matter

### Setup

- `list-msb-datasets`
- `fetch-msb-repo`
- `doctor`
- `model refresh`
- `model list`
- `model find`
- `model providers`
- `model set`
- `model provider current`
- `model provider set`

### Single-instance execution

- `prepare-msb-single`
- `run-msb-single`
- `run-msb-agent-single`

Use `run-msb-single` when you want repo reset/indexing/snapshot without the agent turn.
Use `run-msb-agent-single` for the normal eval path.

### Batch execution

- `prepare-msb-batch`
- `run-msb-batch`
- `run-msb-agent-batch`

Use batch commands only when batch orchestration is the point. They are not the safest path
for interactive validity-sensitive work.

### Inspection

- `transcript`
- `conversations`
- `inspect conversations`
- `inspect turn`
- `inspect tool-calls`
- `inspect failures`
- `inspect config`
- `inspect query`

### Campaign / registry / closure

- `campaign`
  Campaign manifests and campaign-scoped export.
- `registry`
  The local typed universe of target instances.
- `closure`
  Reduced campaign progress across registry, eval, and protocol coverage.

For measured work, campaign + closure is often the right operator layer even if top-level help
does not emphasize that enough.

## Model / Provider Semantics

There are four different knobs:

1. active model
2. persisted default provider for a model
3. per-run provider override
4. embedding override on specific commands

Practical rule:
- `model set` chooses the active model
- `model provider set` chooses the saved default provider for that model
- `--provider` pins one invocation only

If the task is about why a run used a provider, check those in that order.

## Minimal Workflow Answers

When asked “what should I run?” bias toward these answers:

- One instance:
  - `fetch-msb-repo`
  - `prepare-msb-single`
  - `run-msb-agent-single`
- One target family with stateful progress:
  - `campaign show`
  - `closure status`
  - `closure advance eval`
  - `campaign export-submissions`
- Debugging one run:
  - `conversations`
  - `inspect failures`
  - `inspect tool-calls`
  - `transcript`

## When The CLI Is Not Enough

Escalate beyond the CLI only if one of these is true:

- help text does not explain the relevant artifact location
- help text does not explain command precedence or state resolution
- observed artifacts contradict the command contract
- the task is to verify whether the command is misleading or buggy

If you escalate, say explicitly what the CLI failed to tell you.

## Output

When helping with eval operations, prefer this response shape:

- `goal`
- `recommended_commands`
- `artifact_paths`
- `what_to_trust`
- `what_not_to_trust`
- `open_risks`
- `next_step`

## Guardrails

- Do not recommend batch commands as the default safe path.
- Do not treat terminal success strings as proof of a valid patch.
- Do not assume the batch aggregate JSONL is complete.
- Do not assume `doctor` proves runtime validity; it is mostly setup health.
- If the user asks “where do the real outputs live?”, answer with concrete paths.
