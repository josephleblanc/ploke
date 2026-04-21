# ploke-eval CLI audit from help text only

Date: 2026-04-21

Method:
- Primary source was `./target/debug/ploke-eval --help` plus `./target/debug/ploke-eval help <subcommand>`.
- No source inspection was used for the findings below.
- Where I infer workflow rather than quote explicit help text, I label it as inference.

## 1. Operator commands implied by help

### Single-run evals

The help text implies two distinct single-run paths:

1. Prepare and run initialization only:
   - `./target/debug/ploke-eval fetch-msb-repo --dataset-key <dataset-key>`
   - `./target/debug/ploke-eval prepare-msb-single --dataset-key <dataset-key> --instance <instance>`
   - `./target/debug/ploke-eval run-msb-single --instance <instance>`

2. Prepare and run one agentic benchmark turn:
   - `./target/debug/ploke-eval fetch-msb-repo --dataset-key <dataset-key>`
   - `./target/debug/ploke-eval prepare-msb-single --dataset-key <dataset-key> --instance <instance>`
   - `./target/debug/ploke-eval run-msb-agent-single --instance <instance>`

Model/provider overrides available at run time:
- `--use-default-model`
- `--model-id <MODEL_ID>`
- `--provider <PROVIDER>`
- For `run-msb-agent-single` specifically:
  - `--embedding-model-id <EMBEDDING_MODEL_ID>`
  - `--embedding-provider <PROVIDER>`

What the help explicitly says these do:
- `run-msb-single`: execute one prepared run through repo reset and initial artifact generation.
- `run-msb-agent-single`: extend the normal run with a single agentic turn, record prompt/tool/message lifecycle, and write a turn trace and summary beside run artifacts.

### Batch evals

The help text implies two batch paths:

1. Prepare and run initialization only for many instances:
   - `./target/debug/ploke-eval prepare-msb-batch --dataset-key <dataset-key> --all`
   - `./target/debug/ploke-eval run-msb-batch --batch-id <batch-id>`

2. Prepare and run one agentic turn per instance:
   - `./target/debug/ploke-eval prepare-msb-batch --dataset-key <dataset-key> --all`
   - `./target/debug/ploke-eval run-msb-agent-batch --batch-id <batch-id>`

Batch selection options on prepare:
- `--all`
- repeated `--instance <INSTANCE>`
- `--specific <SPECIFIC>`
- `--limit <LIMIT>`
- optional `--batch-id <BATCH_ID>`

Batch execution options:
- `--batch <PATH>` or `--batch-id <BATCH_ID>`
- `--stop-on-error`
- `--use-default-model`
- `--model-id <MODEL_ID>`
- `--provider <PROVIDER>`

What the help explicitly says batch execution writes:
- `batch-run-summary.json`
- `multi-swe-bench-submission.jsonl`

### Inspection and troubleshooting

Lightweight inspection:
- `./target/debug/ploke-eval doctor`
- `./target/debug/ploke-eval transcript`
- `./target/debug/ploke-eval conversations --instance <instance>`
- `./target/debug/ploke-eval replay-msb-batch --instance <instance> --batch <n>`

Structured inspection:
- `./target/debug/ploke-eval inspect conversations --instance <instance>`
- `./target/debug/ploke-eval inspect tool-calls --instance <instance>`
- `./target/debug/ploke-eval inspect db-snapshots --instance <instance>`
- `./target/debug/ploke-eval inspect failures --instance <instance>`
- `./target/debug/ploke-eval inspect config --instance <instance>`
- `./target/debug/ploke-eval inspect turn --instance <instance> <turn>`
- `./target/debug/ploke-eval inspect turn --instance <instance> <turn> --show messages`
- `./target/debug/ploke-eval inspect query --instance <instance> --turn <turn> --lookup <symbol>`

Notable defaults:
- `transcript` operates on the most recent completed run.
- `conversations` defaults to the most recent run's `record.json.gz` if neither `--record` nor `--instance` is given.
- `inspect` also defaults to the most recent completed run if `--record` and `--instance` are omitted.

### Model/provider setup

The help suggests this operator flow:

1. Refresh cached registry:
   - `./target/debug/ploke-eval model refresh`
2. Browse or search models:
   - `./target/debug/ploke-eval model list`
   - `./target/debug/ploke-eval model find <query>`
3. Set active model:
   - `./target/debug/ploke-eval model set <MODEL_ID>`
   - verify with `./target/debug/ploke-eval model current`
4. Inspect provider options for that model:
   - `./target/debug/ploke-eval model providers`
   - or `./target/debug/ploke-eval model providers <MODEL_ID>`
5. Persist a default provider:
   - `./target/debug/ploke-eval model provider set <PROVIDER_SLUG>`
   - verify with `./target/debug/ploke-eval model provider current`
   - clear with `./target/debug/ploke-eval model provider clear`
6. Optionally pin a provider per run instead of persisting it:
   - `./target/debug/ploke-eval run-msb-agent-single --instance <instance> --provider <slug>`

## 2. Default locations implied by help

Top-level defaults under `PLOKE_EVAL_HOME`:
- `PLOKE_EVAL_HOME`: `~/.ploke-eval`
- datasets cache: `~/.ploke-eval/datasets`
- model registry: `~/.ploke-eval/models/registry.json`
- active model: `~/.ploke-eval/models/active-model.json`
- provider prefs: `~/.ploke-eval/models/provider-preferences.json`
- repo cache: `~/.ploke-eval/repos`
- run artifacts: `~/.ploke-eval/runs`
- batch artifacts: `~/.ploke-eval/batches`

Concrete default paths called out by help:
- Dataset JSONL cache:
  - `~/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl`
- Repo checkout:
  - `~/.ploke-eval/repos/<org>/<repo>`
  - example: `~/.ploke-eval/repos/BurntSushi/ripgrep`
- Single-run directory:
  - `~/.ploke-eval/runs/<instance>`
  - example: `~/.ploke-eval/runs/BurntSushi__ripgrep-2209`
- Default single-run manifest:
  - `~/.ploke-eval/runs/<instance>/run.json`
- Batch directory:
  - `~/.ploke-eval/batches/<batch-id>`
  - example: `~/.ploke-eval/batches/ripgrep-2209`
- Default batch manifest:
  - `~/.ploke-eval/batches/<batch-id>/batch.json`

Run-local artifacts explicitly named by help:
- `run.json`
- `repo-state.json`
- `execution-log.json`
- `indexing-status.json`
- `snapshot-status.json`
- `indexing-checkpoint.db`
- `indexing-failure.db`
- `record.json.gz` inferred from `conversations` and `inspect` help using that default path shape
- per-run config sandbox:
  - `~/.ploke-eval/runs/<instance>/config`
- `multi-swe-bench-submission.jsonl`
- `replay-batch-<nnn>.json`

Batch-local artifacts explicitly named by help:
- `batch.json`
- `batch-run-summary.json`
- `multi-swe-bench-submission.jsonl`

Campaign-related path explicitly named by help:
- campaign root:
  - `~/.ploke-eval/campaigns/<campaign>`
- campaign manifest:
  - `~/.ploke-eval/campaigns/<campaign>/campaign.json`

Submission JSONL location summary from help alone:
- A per-run `multi-swe-bench-submission.jsonl` exists under the run directory because the top-level help lists it as a key run artifact.
- A batch-level `multi-swe-bench-submission.jsonl` exists under the batch directory because `run-msb-batch` and `run-msb-agent-batch` say they write it.
- `campaign export-submissions` can write a submission JSONL, but its default output path is not stated in help. Only `--output <PATH>` is documented.

## 3. Practical workflow inferred from help text

This is the practical operator workflow that the help text appears to encourage.

### Basic path

1. Discover a dataset:
   - `list-msb-datasets`
2. Fetch the benchmark repo for that dataset:
   - `fetch-msb-repo --dataset-key <dataset-key>`
3. Prepare one run manifest or a batch manifest:
   - `prepare-msb-single` or `prepare-msb-batch`
4. Decide model/provider routing:
   - `model refresh`
   - `model list` or `model find`
   - `model set`
   - `model providers`
   - optionally `model provider set`
5. Execute:
   - `run-msb-single` or `run-msb-agent-single`
   - `run-msb-batch` or `run-msb-agent-batch`
6. Inspect what happened:
   - `doctor`, `transcript`, `conversations`, `inspect ...`
7. Produce submission artifacts:
   - per run or per batch, use the emitted `multi-swe-bench-submission.jsonl`
   - for campaign-scoped export, use `campaign export-submissions`

### What "prepare" appears to mean

From help alone, "prepare" appears to mean:
- locate a Multi-SWE-bench instance in dataset JSONL
- resolve the repo checkout from the repo cache
- normalize the instance into `run.json`
- stamp run limits like `--max-turns`, `--max-tool-calls`, and `--wall-clock-secs`
- for batch mode, also write `batch.json`

### What "run" appears to mean

From help alone:
- non-agent runs reset the repo, index it with ploke, save a DB snapshot, and write run telemetry/artifacts
- agent runs do all of the above plus exactly one benchmark issue turn through the real app/state path
- batch runs are sequential reuse of prepared per-instance manifests

### What "inspection" appears to be for

The CLI appears to separate:
- simple operator views:
  - `transcript`
  - `conversations`
- structured forensic views:
  - `inspect turn`
  - `inspect tool-calls`
  - `inspect db-snapshots`
  - `inspect query`
- setup diagnosis:
  - `doctor`
- embed-path debugging:
  - `replay-msb-batch`

## 4. Confusing, missing, or low-discoverability areas in help

These are documentation/discoverability issues visible from help alone, not claims about implementation defects.

### High-impact ambiguity

1. The single-run quick start is internally odd.
   - Top-level quick start says:
     - `run-msb-single`
     - then `model providers moonshotai/kimi-k2`
     - then `run-msb-agent-single --provider chutes`
   - That suggests model/provider setup happens after `run-msb-single`, even though model selection is also a run input.
   - An operator can infer this is just an example sequence, but the intended ordering is not stated cleanly.

2. The difference between `run-msb-single` and `run-msb-agent-single` is not obvious from names alone.
   - The top-level descriptions help, but an operator still has to inspect nested help to realize one is initialization only and the other includes the benchmark turn.

3. Submission JSONL destinations are only partially discoverable.
   - Run-level and batch-level submission files are mentioned.
   - `campaign export-submissions` does not state its default output path when `--output` is omitted.
   - If a default exists, help does not reveal it.

4. `record.json.gz` is important to inspection, but top-level help does not list it among key run artifacts.
   - It is discoverable only indirectly through `conversations` and `inspect`.

### Missing operator guidance

1. There is no single "happy path" for batch mode that includes model setup, execution, and inspection together.
   - The batch example stops at `run-msb-agent-batch`.

2. There is no explicit "minimal commands to change model/provider safely" sequence.
   - The necessary commands exist, but the operator has to assemble the flow from multiple help pages.

3. `doctor` has no sub-help explaining what it checks.
   - The verb is promising, but discoverability is low because the help only repeats the one-line description.

4. `transcript` has no selector flags.
   - Help says "most recent completed run" only.
   - If it supports targeting a specific run, that is not discoverable here.
   - If it truly does not support it, that is a practical limitation an operator will hit quickly.

5. `list-msb-datasets` tells the operator nothing about its output shape.
   - There is no example and no hint whether it prints keys only, metadata, or paths.

6. The help does not clearly say whether `prepare-msb-single --dataset-key ...` auto-resolves the dataset JSONL from cache or requires `fetch-msb-repo` first.
   - The basic flow strongly implies fetch first.
   - The prepare help says it reads the dataset JSONL and repo checkout, but it does not explicitly state which prerequisites it will or will not create.

### Low-discoverability naming or wording

1. `prepare-single` exists at top level but is not featured anywhere in quick start.
   - From help alone, its relationship to `prepare-msb-single` is unclear.

2. `replay-msb-batch` is really an embedding-batch replay/debugging command.
   - The name is easy to confuse with rerunning an eval batch.
   - The top-level replay notes help, but only after reading them closely.

3. `inspect` help includes example aliases that do not match the listed command names.
   - It shows:
     - `inspect proto --instance ...`
     - `inspect proto --all-runs`
   - But the listed subcommands are `protocol-artifacts` and `protocol-overview`.
   - From help alone, that looks inconsistent.

4. `campaign`, `registry`, and `closure` are presented at top level, but the eval quick start does not explain when an operator should stay in the run/batch flow versus move to campaign/closure workflows.

5. The benchmark boundary note is useful but easy to miss.
   - It clarifies that local artifacts are telemetry and official pass/fail requires the external Multi-SWE-bench evaluator.
   - This is operationally important and arguably belongs nearer the run/batch command helps too.

## Bottom line

From help alone, the CLI presents a credible operator story:
- fetch repo
- prepare run or batch manifest
- optionally configure model/provider
- run non-agent or single-turn agent evals
- inspect artifacts and conversations
- export submission JSONL for external evaluation

The main gaps are not missing commands but discoverability:
- ambiguous sequencing around model/provider setup
- incomplete disclosure of artifact locations, especially exported submissions
- important inspection artifacts discoverable only indirectly
- a few naming/example inconsistencies that make the operator stop and re-interpret the help.
