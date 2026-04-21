# `ploke-eval` CLI audit from `ploke-eval-operator` skill + help surface

Scope: this report uses only [`docs/workflow/skills/ploke-eval-operator/SKILL.md`](/home/brasides/code/ploke/docs/workflow/skills/ploke-eval-operator/SKILL.md) and `./target/debug/ploke-eval --help` plus `help` on relevant subcommands.

## 1. Commands I would run

One-instance eval:

```bash
./target/debug/ploke-eval list-msb-datasets
./target/debug/ploke-eval fetch-msb-repo --dataset-key <dataset-key>
./target/debug/ploke-eval model current
./target/debug/ploke-eval model providers [MODEL_ID]
./target/debug/ploke-eval prepare-msb-single --dataset-key <dataset-key> --instance <instance-id>
./target/debug/ploke-eval run-msb-agent-single --instance <instance-id> [--provider <slug>]
```

Why: the skill calls `run-msb-agent-single` the "default execution primitive" and gives exactly this setup path. The CLI help for `run-msb-agent-single` says it executes a prepared run and then "writes a turn trace and summary beside the run artifacts."

Batch / family progress:

```bash
./target/debug/ploke-eval campaign show --campaign <campaign>
./target/debug/ploke-eval closure status --campaign <campaign>
./target/debug/ploke-eval closure advance eval
./target/debug/ploke-eval closure advance protocol
```

Why: the skill's "Minimal Workflow Answers" says to prefer `campaign show`, `closure status`, `closure advance eval`, and `campaign export-submissions` for "one target family with stateful progress." The raw batch path also exists:

```bash
./target/debug/ploke-eval prepare-msb-batch --dataset-key <dataset-key> --specific <selector>
./target/debug/ploke-eval run-msb-agent-batch --batch-id <batch-id>
```

But the skill explicitly says batch commands are not the safest default for "interactive validity-sensitive work."

Export:

```bash
./target/debug/ploke-eval campaign export-submissions --campaign <campaign>
./target/debug/ploke-eval campaign export-submissions --campaign <campaign> --nonempty-only
```

Why: both the skill and top-level help point to `campaign export-submissions` as the campaign-scope export path.

## 2. Where outputs appear to live

Top-level defaults from `--help`:

- `~/.ploke-eval/datasets`
- `~/.ploke-eval/repos`
- `~/.ploke-eval/runs`
- `~/.ploke-eval/batches`
- `~/.ploke-eval/campaigns/<campaign>/campaign.json`

Concrete run / batch outputs named in help:

- Per-run manifest: `~/.ploke-eval/runs/<instance>/run.json`
- Per-run directory: `~/.ploke-eval/runs/<instance>`
- Batch manifest: `~/.ploke-eval/batches/<batch-id>/batch.json`
- Batch directory: `~/.ploke-eval/batches/<batch-id>`
- Key run artifacts: `run.json`, `repo-state.json`, `execution-log.json`, `indexing-status.json`, `snapshot-status.json`, `multi-swe-bench-submission.jsonl`
- Batch artifacts named by help: `batch-run-summary.json`, `multi-swe-bench-submission.jsonl`
- Replay output: `replay-batch-<nnn>.json` beside the run manifest
- Campaign closure seed path implied by `campaign init --from-closure-state`: `~/.ploke-eval/campaigns/<campaign>/closure-state.json`

What is still not explicit from help:

- The persisted file path for the typed target `registry`
- The default output path for `campaign export-submissions` when `--output` is omitted
- The exact path of closure recompute/status output, beyond the implied `closure-state.json`

## 3. Trustworthy vs untrustworthy artifacts

Most trustworthy, based on the skill:

1. Per-run artifacts in the selected run directory
2. Campaign-level export from `campaign export-submissions`
3. Closure state for campaign progress

Less trustworthy or explicitly warned against:

1. Batch aggregate `multi-swe-bench-submission.jsonl`
2. Terminal summary strings

Important nuance from top-level help:

- `multi-swe-bench-submission.jsonl` is only a "candidate patch artifact" for the official evaluator.
- Local `ploke-eval` artifacts are telemetry, not the benchmark verdict.
- Official pass/fail comes from the external evaluator on the exported submission.

My read: per-run artifacts look trustworthy for local inspection and provenance; campaign export looks trustworthy for packaging completed runs; batch aggregate JSONL looks unsafe as a source of truth if batches were rerun, interrupted, or overlapped; terminal success text should not be treated as proof of validity.

## 4. What `registry`, `campaign`, and `closure` seem to be

`registry`:

- "the local typed benchmark target registry"
- recomputed from dataset sources
- appears to be the persisted local universe of benchmark targets, but the CLI does not expose where it lives on disk

`campaign`:

- a named manifest under `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- seems to define the operator scope for datasets, model/provider overrides, required procedures, and run/batch roots
- also owns campaign-scoped export via `campaign export-submissions`

`closure`:

- "reduced campaign closure state" for a campaign
- tracks staged completion across "registry, eval, and protocol coverage"
- seems to be the progress/state reduction layer derived from existing datasets and artifacts, with `advance` producing missing eval or protocol artifacts

My shorthand interpretation: `registry` is the target inventory, `campaign` is the intended workset/config, and `closure` is the reduced progress state over that workset.

## 5. What remains unclear or awkward

- `closure advance eval` help does not show required flags like `--campaign`; from the surfaced help, it is not obvious how campaign context is supplied at the leaf command.
- `closure status` and `closure recompute` clearly take `--campaign`, but `closure advance` help does not mirror that context.
- The CLI names both `registry` and model `registry`; top-level help gives a file path only for the model registry, which is easy to confuse with the typed benchmark target registry.
- The default output path for `campaign export-submissions` is not stated.
- The exact on-disk location of the persisted typed target registry is not stated.
- The exact on-disk location of closure state is only indirectly exposed through `campaign init --from-closure-state`.
- The skill says "answer with concrete paths" when asked where real outputs live, but the help surface only fully supports that for runs, batches, repos, datasets, and campaign manifests.
- There are two batch stories: raw `prepare-msb-batch` / `run-msb-agent-batch` and campaign/closure progress. The skill resolves the ambiguity, but the top-level CLI help does not strongly explain when to prefer one over the other.

## Questions I would still ask

- What exact command form is intended for `closure advance eval` on a specific campaign?
- Where is the persisted typed target registry stored?
- When `campaign export-submissions` omits `--output`, what file path does it write?
- Is `closure-state.json` always the authoritative closure artifact path, or only the seed path used by `campaign init --from-closure-state`?
