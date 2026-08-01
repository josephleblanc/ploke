# ploke-eval CLI audit

Source of truth: `./target/debug/ploke-eval --help` and `./target/debug/ploke-eval help <subcommand>`.

## 1) What an operator should run

### Single-run evals

The MSB-shaped path is:
`fetch-msb-repo --dataset-key ...` -> `prepare-msb-single --dataset-key ... --instance ...` -> `run-msb-single --instance ...`

For agentic runs, swap in `run-msb-agent-single`.

The generic non-MSB path also exists:
`prepare-single --task-id ... --repo ... --out-dir ...`

That path is more manual and expects the operator to provide the repo checkout, task id, and issue text/body themselves.

### Batch evals

The MSB batch path is:
`prepare-msb-batch ...` -> `run-msb-batch ...` or `run-msb-agent-batch ...`

`prepare-msb-batch` selects instances with `--all`, repeated `--instance`, or `--specific`, then writes one run manifest per instance plus a batch manifest.

### Inspection

The main inspection entrypoint is `inspect`, with subcommands for:
`conversations`, `tool-calls`, `tool-overview`, `db-snapshots`, `failures`, `config`, `turn`, `query`, `protocol-artifacts`, and `protocol-overview`.

Two shortcuts sit outside `inspect`:
`transcript` prints assistant messages from the most recent completed run, and `conversations` at the top level lists turn summaries from a run.

`doctor` is the lightweight setup check.

### Model/provider setup

The model flow is:
`model refresh` -> `model list` / `model find` -> `model set <MODEL_ID>` -> `model provider set <PROVIDER_SLUG>`

`model providers` shows provider endpoints for the current active model unless a model id is passed.

`model provider current` and `model provider clear` manage the persisted default provider for the active or specified model.

Run commands can override this with `--model-id`, `--provider`, and, for agentic single runs, `--embedding-model-id` and `--embedding-provider`.

## 2) Default locations

Explicit defaults in the help:
- `PLOKE_EVAL_HOME`: `~/.ploke-eval`
- datasets cache: `~/.ploke-eval/datasets`
- model registry: `~/.ploke-eval/models/registry.json`
- active model: `~/.ploke-eval/models/active-model.json`
- provider prefs: `~/.ploke-eval/models/provider-preferences.json`
- repo cache: `~/.ploke-eval/repos`
- run artifacts root: `~/.ploke-eval/runs`
- batch artifacts root: `~/.ploke-eval/batches`
- repo checkout: `~/.ploke-eval/repos/<org>/<repo>`
- run directory: `~/.ploke-eval/runs/<instance>`
- batch directory: `~/.ploke-eval/batches/<batch-id>`
- run manifest: `~/.ploke-eval/runs/<instance>/run.json`
- batch manifest: `~/.ploke-eval/batches/<batch-id>/batch.json`
- record file: `~/.ploke-eval/runs/<instance>/record.json.gz`
- config sandbox: `~/.ploke-eval/runs/<instance>/config`
- campaign manifest: `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- campaign closure state: `~/.ploke-eval/campaigns/<campaign>/closure-state.json`

Submission JSONL:
- For single-run output, the help explicitly names `multi-swe-bench-submission.jsonl` as a key run artifact, so the default home is the run directory.
- For batch output, `run-msb-batch` and `run-msb-agent-batch` both say they write `multi-swe-bench-submission.jsonl` plus `batch-run-summary.json`; the help does not spell out the final directory in that sentence, but the batch root default points to `~/.ploke-eval/batches/<batch-id>/`.
- `campaign export-submissions` has `--output <PATH>` and no default output path in the help, so it is not a fixed default-location command.

## 3) Practical workflow implied by the help

The CLI reads like a staged pipeline:
1. Discover or refresh the model catalog, then choose a model and default provider.
2. Fetch the benchmark repo into the local cache.
3. Normalize one instance or a selection of instances into run manifests.
4. Execute the run(s), either as non-agentic initial runs or as agentic benchmark turns.
5. Inspect the resulting records, snapshots, tool calls, or transcripts.
6. If working in campaign mode, reconcile closure state and export submission JSONL.

That workflow is clearer for Multi-SWE-bench than for ad hoc evals. The generic `prepare-single` path exists, but the top-level help does not connect it to the MSB-specific commands.

## 4) Confusing, missing, or low-discoverability

- There are two overlapping prep flows (`prepare-single` vs `prepare-msb-single`) and two overlapping run flows (`run-msb-single` vs `run-msb-agent-single`), but the help does not explain when to choose the generic path over the MSB path.
- `inspect` is well-factored, but `transcript` and top-level `conversations` duplicate nearby inspection affordances without a clear “use this when you just want X” summary.
- Model/provider precedence is implied, not explicit: active model selection, persisted provider preference, and per-run overrides all exist, but the help does not spell out the resolution order in one place.
- Batch submission output is mentioned, but the exact directory for the batch-produced `multi-swe-bench-submission.jsonl` is not stated as directly as the run-directory artifact paths.
- `campaign`, `closure`, and `registry` look like an operator control plane, but the top-level help does not show a straight-line path from “run eval” to “export submissions.”
- `campaign export-submissions` requires `--campaign`, but the help does not advertise an output default, which makes the command feel incomplete until you already know the campaign directory layout.

