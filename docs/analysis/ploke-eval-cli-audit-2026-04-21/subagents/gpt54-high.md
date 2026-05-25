# ploke-eval CLI audit from help text alone

Date: 2026-04-21

Method: this audit uses `./target/debug/ploke-eval --help` and `./target/debug/ploke-eval help ...` output only. It does not assume source behavior beyond what the CLI help explicitly says.

## 1. Commands an operator appears to run

### Single-run evals

There are two visible single-run entry points:

1. Generic manifest preparation:

```bash
./target/debug/ploke-eval help prepare-single
./target/debug/ploke-eval prepare-single \
  --task-id <TASK_ID> \
  --repo <PATH> \
  --out-dir <PATH> \
  [--issue-title <TITLE>] \
  [--issue-file <PATH> | --issue-body <TEXT>] \
  [--base-sha <SHA>]
```

2. Multi-SWE-bench single-run flow:

```bash
./target/debug/ploke-eval fetch-msb-repo --dataset-key <DATASET_KEY>
./target/debug/ploke-eval prepare-msb-single --dataset-key <DATASET_KEY> --instance <INSTANCE>
./target/debug/ploke-eval run-msb-single --instance <INSTANCE>
```

If the operator wants the agentic turn as part of the run, the advertised command is:

```bash
./target/debug/ploke-eval run-msb-agent-single --instance <INSTANCE>
```

Per-run model/provider overrides exposed by help:

```bash
./target/debug/ploke-eval run-msb-single --instance <INSTANCE> --model-id <MODEL_ID> --provider <PROVIDER>
./target/debug/ploke-eval run-msb-agent-single --instance <INSTANCE> --model-id <MODEL_ID> --provider <PROVIDER>
./target/debug/ploke-eval run-msb-agent-single --instance <INSTANCE> --embedding-model-id <MODEL_ID> --embedding-provider <PROVIDER>
```

### Batch evals

Preparation:

```bash
./target/debug/ploke-eval prepare-msb-batch --dataset-key <DATASET_KEY> --all
./target/debug/ploke-eval prepare-msb-batch --dataset-key <DATASET_KEY> --specific <MATCH>
./target/debug/ploke-eval prepare-msb-batch --dataset-key <DATASET_KEY> --instance <INSTANCE>
```

Execution:

```bash
./target/debug/ploke-eval run-msb-batch --batch-id <BATCH_ID>
./target/debug/ploke-eval run-msb-agent-batch --batch-id <BATCH_ID>
```

The batch runners also accept explicit manifest paths:

```bash
./target/debug/ploke-eval run-msb-batch --batch ~/.ploke-eval/batches/<batch-id>/batch.json
./target/debug/ploke-eval run-msb-agent-batch --batch ~/.ploke-eval/batches/<batch-id>/batch.json
```

The help describes `run-msb-batch` as sequential prepared-run execution, and `run-msb-agent-batch` as the same plus one agentic benchmark turn per instance.

### Inspection and debugging

Top-level inspection commands:

```bash
./target/debug/ploke-eval doctor
./target/debug/ploke-eval transcript
./target/debug/ploke-eval conversations --instance <INSTANCE>
./target/debug/ploke-eval replay-msb-batch --instance <INSTANCE> --batch <N>
```

Run/turn inspection surface:

```bash
./target/debug/ploke-eval inspect conversations --instance <INSTANCE>
./target/debug/ploke-eval inspect tool-calls --instance <INSTANCE>
./target/debug/ploke-eval inspect db-snapshots --instance <INSTANCE>
./target/debug/ploke-eval inspect failures --instance <INSTANCE>
./target/debug/ploke-eval inspect config --instance <INSTANCE>
./target/debug/ploke-eval inspect turn --instance <INSTANCE> <TURN>
./target/debug/ploke-eval inspect turn --instance <INSTANCE> <TURN> --show messages
./target/debug/ploke-eval inspect query --instance <INSTANCE> --turn <TURN> --lookup <SYMBOL>
./target/debug/ploke-eval inspect protocol-artifacts --instance <INSTANCE>
./target/debug/ploke-eval inspect protocol-overview --instance <INSTANCE>
./target/debug/ploke-eval inspect protocol-overview --all-runs
./target/debug/ploke-eval inspect protocol-overview --campaign <CAMPAIGN>
```

Most `inspect` subcommands default to the most recent completed run if `--record` and `--instance` are omitted.

### Model and provider setup

The model-management workflow exposed by help is:

```bash
./target/debug/ploke-eval model refresh
./target/debug/ploke-eval model list
./target/debug/ploke-eval model find <QUERY>
./target/debug/ploke-eval model set <MODEL_ID>
./target/debug/ploke-eval model current
./target/debug/ploke-eval model providers [MODEL_ID]
./target/debug/ploke-eval model provider current [--model-id <MODEL_ID>]
./target/debug/ploke-eval model provider set <PROVIDER_SLUG> [--model-id <MODEL_ID>]
./target/debug/ploke-eval model provider clear [--model-id <MODEL_ID>]
```

From help alone, the intended operator sequence appears to be:

1. `model refresh`
2. `model list` or `model find`
3. `model set <MODEL_ID>`
4. `model providers`
5. `model provider set <PROVIDER_SLUG>`
6. Run with the persisted selection, or override with `--model-id` / `--provider`

## 2. Default locations

Top-level defaults shown by `--help`:

```text
PLOKE_EVAL_HOME    ~/.ploke-eval
datasets cache     ~/.ploke-eval/datasets
model registry     ~/.ploke-eval/models/registry.json
active model       ~/.ploke-eval/models/active-model.json
provider prefs     ~/.ploke-eval/models/provider-preferences.json
repo cache         ~/.ploke-eval/repos
run artifacts      ~/.ploke-eval/runs
batch artifacts    ~/.ploke-eval/batches
```

Default artifact locations called out explicitly:

```text
Dataset JSONL cache:
  ~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl

Repo checkout:
  ~/.ploke-eval/repos/BurntSushi/ripgrep

Run directory:
  ~/.ploke-eval/runs/BurntSushi__ripgrep-2209

Batch directory:
  ~/.ploke-eval/batches/ripgrep-2209
```

Per-run artifacts explicitly named by help:

```text
~/.ploke-eval/runs/<instance>/run.json
~/.ploke-eval/runs/<instance>/repo-state.json
~/.ploke-eval/runs/<instance>/execution-log.json
~/.ploke-eval/runs/<instance>/indexing-status.json
~/.ploke-eval/runs/<instance>/snapshot-status.json
~/.ploke-eval/runs/<instance>/indexing-checkpoint.db
~/.ploke-eval/runs/<instance>/indexing-failure.db
~/.ploke-eval/runs/<instance>/config/
~/.ploke-eval/runs/<instance>/multi-swe-bench-submission.jsonl
```

Batch artifacts explicitly named by help:

```text
~/.ploke-eval/batches/<batch-id>/batch.json
~/.ploke-eval/batches/<batch-id>/batch-run-summary.json
~/.ploke-eval/batches/<batch-id>/multi-swe-bench-submission.jsonl
```

Record locations inferred from inspection help:

```text
~/.ploke-eval/runs/<instance>/record.json.gz
```

That path is referenced repeatedly by `conversations` and `inspect`, but it is not listed in the run command’s artifact summary.

Campaign-related path surfaced by help:

```text
~/.ploke-eval/campaigns/<campaign>/campaign.json
```

For `campaign export-submissions --output <PATH>`, the default output path is not documented.

All of the above can be relocated by setting:

```bash
PLOKE_EVAL_HOME=/some/path
```

## 3. Practical workflow inferred from help text

The CLI help suggests a mostly Multi-SWE-bench-centered workflow:

1. Identify a dataset key:

```bash
./target/debug/ploke-eval list-msb-datasets
```

2. Fetch or refresh the benchmark repo:

```bash
./target/debug/ploke-eval fetch-msb-repo --dataset-key <DATASET_KEY>
```

3. Prepare either one instance or a batch:

```bash
./target/debug/ploke-eval prepare-msb-single --dataset-key <DATASET_KEY> --instance <INSTANCE>
./target/debug/ploke-eval prepare-msb-batch --dataset-key <DATASET_KEY> --all
```

4. Set up model/provider state if needed:

```bash
./target/debug/ploke-eval model refresh
./target/debug/ploke-eval model set <MODEL_ID>
./target/debug/ploke-eval model provider set <PROVIDER_SLUG>
```

5. Execute either the non-agentic path or the agentic path:

```bash
./target/debug/ploke-eval run-msb-single --instance <INSTANCE>
./target/debug/ploke-eval run-msb-agent-single --instance <INSTANCE>
./target/debug/ploke-eval run-msb-batch --batch-id <BATCH_ID>
./target/debug/ploke-eval run-msb-agent-batch --batch-id <BATCH_ID>
```

6. Inspect the resulting run or batch artifacts:

```bash
./target/debug/ploke-eval transcript
./target/debug/ploke-eval conversations --instance <INSTANCE>
./target/debug/ploke-eval inspect turn --instance <INSTANCE> <TURN>
./target/debug/ploke-eval inspect tool-calls --instance <INSTANCE>
./target/debug/ploke-eval inspect query --instance <INSTANCE> --turn <TURN> --lookup <SYMBOL>
```

7. Export benchmark submission artifacts:

```bash
./target/debug/ploke-eval campaign export-submissions --campaign <CAMPAIGN>
```

The top-level help also states a narrower “current end-to-end path”:

1. fetch a benchmark repo
2. prepare a run manifest from Multi-SWE-bench
3. run one prepared instance

That reads as the officially endorsed minimum path. Agentic runs, batch workflows, campaign export, protocol review, and closure workflows all exist in the command tree, but they feel like adjacent or newer layers on top of that core.

The help further implies that `run-msb-single` performs repository reset, indexing, DB snapshotting through the normal SaveDb path, and artifact writing. `run-msb-agent-single` then extends that with one real issue turn and turn-trace output. In other words, the operator-visible workflow appears to be:

1. prepare immutable input manifests
2. execute a run that materializes repo/index/db artifacts
3. optionally execute one benchmark turn
4. inspect run history and tool traces
5. export submission JSONL for external evaluation

## 4. Confusing, missing, or low-discoverability aspects of help

### High-confidence help issues

1. `inspect` help includes examples for `inspect proto`, but the listed subcommands are `protocol-artifacts` and `protocol-overview`. From help alone, `proto` looks undocumented or stale.

2. `transcript` only targets “the most recent completed run” and exposes no `--instance` or `--record`. That makes it much less usable than `conversations` and `inspect` when an operator needs a specific run.

3. `prepare-single` exists, but the rest of the quick start and execution surface is strongly `msb`-named. From help alone, it is not clear what command should execute a manifest produced by `prepare-single`, or whether `run-msb-single` is valid for it.

4. `record.json.gz` is central to `conversations` and `inspect`, but run help does not list it among the default output artifacts. An operator could finish a run and still not know which file inspection commands are reading.

5. `prepare-msb-batch --batch-id` is optional, but the default batch-id derivation is not explained. The examples imply names such as `ripgrep-all` and `ripgrep-2209`, but that rule is not documented as a rule.

6. `campaign export-submissions` offers `--output <PATH>`, but if `--output` is omitted the destination is not documented.

### Workflow clarity gaps

1. The top-level help says the “current end-to-end path” is fetch -> prepare manifest -> run one instance, yet the CLI also exposes batch runs, agentic runs, replay, campaigns, registry, closure, and protocol workflows. The help does not distinguish “core supported path” from “advanced/operator workflows”.

2. The model help implies an OpenRouter-backed workflow, but the run help only says “use the default model instead of the persisted active model selection” without defining what “default model” means.

3. `doctor` is too terse. It says it reports likely configuration issues, but not what it checks: model state, provider state, repo cache, dataset cache, auth, or filesystem layout.

4. `list-msb-datasets` is discoverable, but the help text does not link it into the fetch/prepare workflow. An operator can infer it belongs there, but the CLI does not say so directly.

5. `run-msb-agent-single` exposes embedding model/provider overrides, but `run-msb-agent-batch` does not. From help alone, it is unclear whether batch agent runs intentionally lack those overrides or whether they are available elsewhere.

6. `conversations` and `inspect conversations` appear overlapping. The help does not explain why both exist or when to prefer one over the other.

7. `replay-msb-batch` uses the word “batch” for embedding replay inside one prepared run, while other commands use “batch” for many-instance eval execution. The help description makes the distinction eventually, but the name is easy to misread.

### Command-prefix inconsistency

Examples are inconsistent about invocation style:

- top-level and most subcommand help uses `cargo run -p ploke-eval -- ...`
- some model help examples use `ploke-eval ...`

That is not fatal, but it weakens copy-paste clarity. A single convention would improve operator guidance.

## Bottom line

From help alone, the safest operator interpretation is:

1. Multi-SWE-bench is the primary documented workflow.
2. The normal path is fetch repo -> prepare manifest(s) -> run -> inspect -> export submission.
3. Default state lives under `~/.ploke-eval`, especially `repos/`, `runs/`, `batches/`, and `models/`.
4. Submission JSONL artifacts are explicitly documented for per-run and per-batch outputs, but campaign export defaults are not.
5. The CLI exposes a larger operator surface than the quick-start narrative explains, so experienced users can probably do much more than the help currently teaches cleanly.
