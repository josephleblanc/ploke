---
name: ploke-eval-operator
description: Use this skill when operating or changing `ploke-eval` CLI workflows, campaigns, closure state, active `select` context, prototype loop commands, or eval artifact paths. Inspect the existing CLI and persisted operator context before proposing new flags, shell variables, config files, or long commands.
---

# Ploke Eval Operator

Use this skill when the task is to run, inspect, export, or reason about evals through
the `ploke-eval` CLI.

## Boundaries

- Start from the CLI surface first: `ploke-eval --help` and `ploke-eval help <subcommand>`.
- Check active operator context early with `ploke-eval select status`.
- Do not read `crates/ploke-eval/src/` unless the CLI surface fails to answer the operator
  question and the task explicitly requires code-level verification.
- Prefer one target family at a time for interactive work.
- Treat per-run artifacts as more trustworthy than batch aggregate artifacts.
- If the task is about a run trace, pair this skill with `cli-trace-review`.
- If the task is about a formal measured run, pair this skill with `run-protocol`.

## Default Operator Stance

- Default execution primitive: `run single agent`
- Default state resolution order:
  1. active `select` context
  2. campaign manifest
  3. closure/prototype scheduler state
  4. explicit CLI arguments
- Default setup path:
  1. `ploke-eval registry status`
  2. `ploke-eval registry show --dataset <family>`
  3. `ploke-eval run repo fetch --dataset-key <key>`
  4. `ploke-eval model current`
  5. `ploke-eval model providers [MODEL_ID]`
  6. `ploke-eval run prepare instance --dataset-key <key> --instance <instance-id>`
  7. `ploke-eval run single agent --instance <instance-id> [--provider ...]`
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

## Execution Debug Tracing

- `ploke-eval --debug-tools` is a simple on/off flag.
- It enables the shared cross-crate tracing target `ploke_core::EXECUTION_DEBUG_TARGET`.
- Use it when you need one execution-debug stream that follows an eval/tool path across crates.
- Expect the detailed output in `~/.ploke-eval/logs`, not primarily on stderr.
- Keep permanent callsites on that target sparse and sanity-oriented; remove temporary ones after diagnosis.

## Command Groups That Matter

### Active selection

- `select status`
- `select campaign <campaign>`
- `select batch <batch>`
- `select instance <instance-id>`
- `select attempt <n>`
- `select unset <scope>`
- `select clear`

Use `select` as the persisted operator context. Before adding flags, shell variables,
or a new config file, ask whether the command can resolve the value from active
selection.

Practical rule:
- If a command needs `--campaign`, first check whether `select campaign` should supply it.
- If a command needs `--instance`, first check whether `select instance` should supply it.
- If a command needs both and they conflict, report the selection warning and do not
  silently choose one.

### Setup

- `registry status`
- `registry show --dataset <family>`
- `run datasets list`
- `run repo fetch`
- `doctor`
- `model refresh`
- `model list`
- `model find`
- `model providers`
- `model set`
- `model provider current`
- `model provider set`

### Single-instance execution

- `run prepare instance`
- `run single setup`
- `run single agent`

Use `run single setup` when you want repo reset/indexing/snapshot without the agent turn.
Use `run single agent` for the normal eval path.

### Batch execution

- `run prepare batch`
- `run batch setup`
- `run batch agent`

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

### Prototype loop

Before proposing or running a prototype loop command, inspect:

- `ploke-eval loop --help`
- `ploke-eval loop <prototype-command> --help`
- `ploke-eval select status`
- `ploke-eval campaign show --campaign <campaign>`
- `ploke-eval campaign validate --campaign <campaign>`

Expected prototype state locations:

- `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- `~/.ploke-eval/campaigns/<campaign>/closure-state.json`
- `~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json`
- `~/.ploke-eval/campaigns/<campaign>/prototype1/branches.json`
- `<repo-root>/.ploke/prototype1/parent_identity.json`

Do not make humans pass IDs already present in campaign/prototype state unless the
CLI genuinely lacks a resolver. Prefer command shapes like:

```bash
ploke-eval select campaign <campaign>
ploke-eval select instance <instance-id>
ploke-eval loop prototype1-state --repo-root .
```

For prototype branch/runtime work, keep `--repo-root` explicit until the command has
strong guardrails; repo checkout selection is operationally risky and should not be
silently inferred from global state.

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
  - `registry show --dataset <family>`
  - `run repo fetch`
  - `run prepare instance`
  - `run single agent`
- One target family with stateful progress:
  - `select campaign <campaign>`
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
