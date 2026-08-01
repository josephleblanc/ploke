# ploke-eval CLI audit

Source of truth used: `./target/debug/ploke-eval --help` and `./target/debug/ploke-eval help <subcommand>`.

## What an operator should run

### Single-run evals
- `./target/debug/ploke-eval fetch-msb-repo --dataset-key <dataset>`
- `./target/debug/ploke-eval prepare-msb-single --dataset-key <dataset> --instance <instance>`
- `./target/debug/ploke-eval run-msb-single --instance <instance>`
- `./target/debug/ploke-eval run-msb-agent-single --instance <instance>`

Useful single-run inspection:
- `./target/debug/ploke-eval transcript`
- `./target/debug/ploke-eval conversations --instance <instance>`
- `./target/debug/ploke-eval inspect turn --instance <instance> 1`
- `./target/debug/ploke-eval inspect query --instance <instance> --turn 1 --lookup <symbol>`

### Batch evals
- `./target/debug/ploke-eval prepare-msb-batch --dataset-key <dataset> --batch-id <batch-id> ...`
- `./target/debug/ploke-eval run-msb-batch --batch-id <batch-id>`
- `./target/debug/ploke-eval run-msb-agent-batch --batch-id <batch-id>`

The batch prep command can select instances via `--all`, `--instance`, `--specific`, and `--limit`.

### Inspection
- `./target/debug/ploke-eval inspect conversations --instance <instance>`
- `./target/debug/ploke-eval inspect tool-calls --instance <instance>`
- `./target/debug/ploke-eval inspect db-snapshots --instance <instance>`
- `./target/debug/ploke-eval inspect failures --instance <instance>`
- `./target/debug/ploke-eval inspect config --instance <instance>`
- `./target/debug/ploke-eval inspect turn --instance <instance> <turn>`
- `./target/debug/ploke-eval inspect query --instance <instance> --turn <turn> <cozo-query>`
- `./target/debug/ploke-eval inspect protocol-artifacts --instance <instance>`
- `./target/debug/ploke-eval inspect protocol-overview --instance <instance>`

### Model / provider setup
- `./target/debug/ploke-eval model refresh`
- `./target/debug/ploke-eval model list`
- `./target/debug/ploke-eval model find <query>`
- `./target/debug/ploke-eval model providers [model-id]`
- `./target/debug/ploke-eval model set <model-id>`
- `./target/debug/ploke-eval model current`
- `./target/debug/ploke-eval model provider current`
- `./target/debug/ploke-eval model provider set <provider-slug>`
- `./target/debug/ploke-eval model provider clear`
- One-off run pinning: `--model-id`, `--provider`, and for agent runs `--embedding-model-id`, `--embedding-provider`

## Default locations

- Root: `PLOKE_EVAL_HOME`, default `~/.ploke-eval`
- Dataset cache: `~/.ploke-eval/datasets`
- Model registry: `~/.ploke-eval/models/registry.json`
- Active model: `~/.ploke-eval/models/active-model.json`
- Provider prefs: `~/.ploke-eval/models/provider-preferences.json`
- Repo cache: `~/.ploke-eval/repos`
- Runs root: `~/.ploke-eval/runs`
- Batch root: `~/.ploke-eval/batches`

Concrete default paths called out by help:
- Repo checkout: `~/.ploke-eval/repos/<org>/<repo>`
- Run manifest / run dir: `~/.ploke-eval/runs/<instance>/run.json` and `~/.ploke-eval/runs/<instance>/`
- Batch manifest: `~/.ploke-eval/batches/<batch-id>/batch.json`
- Dataset JSONL cache example: `~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl`
- Run artifacts: `run.json`, `repo-state.json`, `execution-log.json`, `indexing-status.json`, `snapshot-status.json`, `indexing-checkpoint.db`, `indexing-failure.db`, `multi-swe-bench-submission.jsonl`
- Per-run config sandbox: `~/.ploke-eval/runs/<instance>/config`

The help text also says batch runs write `batch-run-summary.json` and `multi-swe-bench-submission.jsonl`, but it does not spell out a separate batch-output subdirectory beyond the batch root and batch-id directory.

## Practical workflow implied by the help

1. Pick or inspect the active model and provider.
2. Fetch the benchmark repo into the local repo cache.
3. Prepare either one instance or a batch into run manifests.
4. Execute the prepared run(s).
5. Inspect turns, tool calls, DB snapshots, or the assistant transcript.
6. Treat `multi-swe-bench-submission.jsonl` as the exported candidate patch artifact; the CLI explicitly says the local artifacts are telemetry, not the final benchmark verdict.

The docs imply two operator modes:
- single-run: fetch -> prepare-msb-single -> run-msb-single / run-msb-agent-single
- batch: prepare-msb-batch -> run-msb-batch / run-msb-agent-batch

`prepare-single` exists as a generic lower-level escape hatch, but the help surface frames the MSB-specific commands as the normal path.

## Confusing, missing, or low-discoverability

- The top-level help’s “Bootstrap questions” examples use `inspect proto`, but the actual inspect subcommands are `protocol-artifacts` and `protocol-overview`. That looks stale and is easy to miss.
- `inspect` says the default target is the “most recent completed run,” while `conversations` says it defaults to the most recent run’s `record.json.gz`. The default-target wording is inconsistent.
- `model`, `model providers`, `model provider`, `model set`, and the run-level `--provider` flags overlap conceptually. The help does not clearly separate active model, persisted default provider, and per-run provider pinning.
- `run-msb-agent-single` adds embedding-model/provider flags, but the relationship between those and the main model/provider flags is not explained in the help.
- The batch submission JSONL path is only partially specified. The filename is clear, but the exact default output location for batch-produced submission JSONL is not stated as directly as the run directory layout.
- `prepare-single` and `prepare-msb-single` coexist without a strong explanation of when to use which.
