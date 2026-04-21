# CLI Map And Workflows

Date: 2026-04-21

This file starts from the CLI surface, not the source tree.

## Top-level command groups

From `./target/debug/ploke-eval --help`, the top-level commands are:

- `prepare-single`
  Normalize one generic evaluation instance into a run manifest.
- `prepare-msb-single`
  Normalize one Multi-SWE-bench instance into a run manifest.
- `prepare-msb-batch`
  Normalize many Multi-SWE-bench instances into a batch manifest and per-instance run manifests.
- `run-msb-single`
  Execute one prepared run through repo reset and initial artifact generation.
- `run-msb-batch`
  Execute many prepared Multi-SWE-bench runs from one batch manifest.
- `run-msb-agent-single`
  Execute one prepared run and then run a single agentic benchmark turn.
- `run-msb-agent-batch`
  Execute many prepared runs and agentic benchmark turns from one batch manifest.
- `replay-msb-batch`
  Replay one specific embedding batch from a prepared run.
- `fetch-msb-repo`
  Clone or refresh a benchmark repo into `~/.ploke-eval/repos`.
- `list-msb-datasets`
  List built-in dataset registry entries.
- `doctor`
  Inspect the current eval setup and report likely configuration issues.
- `transcript`
  Print only assistant messages from the most recent completed run.
- `conversations`
  List all agent conversation turns from a run.
- `inspect`
  Inspect conversations, tool calls, DB snapshots, failures, config, turns, protocol artifacts, and protocol overview.
- `protocol`
  Run bounded review/adjudication protocols over eval artifacts.
- `model`
  Manage the cached OpenRouter model registry and active model selection.
- `campaign`
  Manage campaign manifests used by closure-driven operator workflows.
- `registry`
  Manage the local typed benchmark target registry.
- `closure`
  Track staged closure of registry, eval, and protocol coverage for a campaign.

## Commands We Actually Need

For ordinary eval work, the useful surface is much smaller.

### Setup

- `list-msb-datasets`
  Discover built-in dataset keys.
- `fetch-msb-repo --dataset-key <key>`
  Ensure the repo checkout exists in the eval cache.
- `doctor`
  Sanity check local eval setup.
- `model refresh`
  Refresh the cached OpenRouter model registry.
- `model list`
  See what models exist locally.
- `model providers [MODEL_ID]`
  See which OpenRouter providers are returned for a model.
- `model set <MODEL_ID>`
  Persist the active eval model.
- `model provider set <PROVIDER_SLUG>`
  Persist the default provider for the active or specified model.

### Single-instance execution

- `prepare-msb-single --dataset-key <key> --instance <instance-id>`
  Create `~/.ploke-eval/runs/<instance>/run.json`.
- `run-msb-single --instance <instance-id>`
  Run repo reset + indexing + snapshot generation.
- `run-msb-agent-single --instance <instance-id>`
  Run repo reset + indexing + one agentic issue turn.

### Batch execution

- `prepare-msb-batch --dataset-key <key> --all|--instance ...|--specific ... [--batch-id ...]`
  Create one `run.json` per selected instance plus one batch manifest.
- `run-msb-batch --batch-id <batch-id>`
  Run initialization-only for the batch.
- `run-msb-agent-batch --batch-id <batch-id>`
  Run one agentic issue turn per instance in the batch.

### Inspection

- `transcript`
  Quick view of assistant messages from the most recent completed run.
- `conversations --instance <instance-id>`
  Quick turn summary table for a run.
- `inspect conversations --instance <instance-id>`
- `inspect tool-calls --instance <instance-id>`
- `inspect failures --instance <instance-id>`
- `inspect config --instance <instance-id>`
- `inspect turn --instance <instance-id> <turn>`
- `inspect query --instance <instance-id> --turn <turn> --lookup <symbol>`

### Campaign / export layer

- `campaign list`
  See available campaigns and whether they have a manifest and closure state.
- `campaign init --campaign <campaign> ...`
  Create the campaign manifest.
- `campaign show --campaign <campaign>`
  Resolve the campaign configuration.
- `campaign validate --campaign <campaign>`
  Validate config against local state and provider routing.
- `closure status --campaign <campaign>`
  See current registry/eval/protocol coverage at the campaign level.
- `closure advance eval --campaign <campaign>`
  Advance missing eval work according to campaign config.
- `campaign export-submissions --campaign <campaign>`
  Export Multi-SWE-bench submission JSONL from completed runs in closure state.

## Default Locations

From help text and actual command output:

- Eval home:
  `~/.ploke-eval`
- Datasets cache:
  `~/.ploke-eval/datasets`
- Model registry:
  `~/.ploke-eval/models/registry.json`
- Active model:
  `~/.ploke-eval/models/active-model.json`
- Provider prefs:
  `~/.ploke-eval/models/provider-preferences.json`
- Repo cache:
  `~/.ploke-eval/repos/<org>/<repo>`
- Runs root:
  `~/.ploke-eval/runs`
- Batch root:
  `~/.ploke-eval/batches`
- Campaign root:
  `~/.ploke-eval/campaigns/<campaign>`
- Registry root:
  `~/.ploke-eval/registries`

Useful concrete artifacts:

- Single-instance manifest:
  `~/.ploke-eval/runs/<instance>/run.json`
- Per-run artifacts live under the instance root and nested run directories.
- Per-run submission artifact:
  `multi-swe-bench-submission.jsonl`
- Batch manifest:
  `~/.ploke-eval/batches/<batch-id>/batch.json`
- Batch summary artifact:
  `batch-run-summary.json`
- Batch aggregate submission artifact:
  `multi-swe-bench-submission.jsonl`
- Campaign manifest:
  `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- Campaign closure state:
  `~/.ploke-eval/campaigns/<campaign>/closure-state.json`
- Target registry:
  `~/.ploke-eval/registries/multi-swe-bench-rust.json`

## Practical Operator Workflows

### Workflow 1: Single target, trustworthy path

Use this when validating harness behavior or collecting one measured run.

1. `ploke-eval list-msb-datasets`
2. `ploke-eval fetch-msb-repo --dataset-key <key>`
3. `ploke-eval model current`
4. `ploke-eval model providers`
5. `ploke-eval prepare-msb-single --dataset-key <key> --instance <instance-id>`
6. `ploke-eval run-msb-agent-single --instance <instance-id> [--provider ...]`
7. `ploke-eval conversations --instance <instance-id>`
8. `ploke-eval inspect failures --instance <instance-id>`
9. Read the run’s `multi-swe-bench-submission.jsonl`

This is the cleanest operational path because each run gets its own nested run directory.

### Workflow 2: Batch by hand

Use only if you explicitly want batch orchestration and you are prepared to inspect per-run artifacts.

1. `ploke-eval prepare-msb-batch --dataset-key <key> --all --batch-id <batch-id>`
2. `ploke-eval run-msb-agent-batch --batch-id <batch-id>`
3. Inspect:
   - batch summary
   - per-instance run directories
   - per-run `multi-swe-bench-submission.jsonl`

Operational warning:
- From real usage, the batch aggregate file is not the safest source of truth.
- The more trustworthy export surface is the per-run submission file set, or campaign export from closure state.

### Workflow 3: Campaign-driven measured work

This is the likely right long-term operator entrypoint.

1. `ploke-eval campaign init --campaign <campaign> --from-registry ...`
2. `ploke-eval campaign show --campaign <campaign>`
3. `ploke-eval campaign validate --campaign <campaign>`
4. `ploke-eval closure status --campaign <campaign>`
5. `ploke-eval closure advance eval --campaign <campaign>`
6. `ploke-eval closure status --campaign <campaign>`
7. `ploke-eval campaign export-submissions --campaign <campaign>`

This layer exists to make eval work stateful and resumable at the campaign level instead of manually juggling dataset files, batch ids, and ad hoc export logic.

## What Are `registry`, `campaign`, and `closure`?

### `registry`

Operator meaning:
- the universe of target instances we know about
- recomputed from dataset sources
- persisted so later workflows do not have to rediscover the world every time

Concrete example:
- `registry status` currently shows one persisted Rust registry with 239 entries across 10 datasets

Why it exists:
- to make the dataset universe explicit
- to give campaign workflows a stable catalog of instances
- to avoid rebuilding target lists ad hoc from scattered dataset JSONLs

### `campaign`

Operator meaning:
- a named configuration for a measurement effort
- includes dataset sources, model, provider, runs root, batches root, required procedures, and policies

Why it exists:
- to turn “what are we evaluating, with what model/provider, and where do artifacts live?” into a persisted manifest
- to support resumable operator workflows and campaign-scoped exports

### `closure`

Operator meaning:
- a reduced state machine over a campaign
- answers: for each instance, do we have registry coverage, eval coverage, and protocol coverage?

Why it exists:
- to separate “the universe of targets” from “what work has been completed”
- to support incremental advancement instead of rerunning everything blindly
- to support export from completed runs rather than trusting one batch file

Concrete example:
- `closure status --campaign rust-baseline-grok4-xai` reports registry completeness, eval success/failure, protocol completeness, and specific failing instances

That is why `closure` may be the real operator entrypoint for serious measured work, even though it is much less discoverable from the top-level help than the raw prepare/run commands.
