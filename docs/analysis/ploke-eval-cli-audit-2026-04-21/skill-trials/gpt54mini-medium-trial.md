# ploke-eval CLI audit

Sources used: [SKILL.md](/home/brasides/code/ploke/docs/workflow/skills/ploke-eval-operator/SKILL.md) and `./target/debug/ploke-eval --help` plus `help <subcommand>` output only.

## 1) Commands I would run

For one-instance eval:

```bash
ploke-eval fetch-msb-repo --dataset-key <key>
ploke-eval prepare-msb-single --dataset-key <key> --instance <instance>
ploke-eval run-msb-agent-single --instance <instance> [--provider <slug>]
```

For batch/family progress:

```bash
ploke-eval registry status
ploke-eval campaign show --campaign <campaign>
ploke-eval closure status --campaign <campaign>
ploke-eval closure advance eval --campaign <campaign>
```

For export:

```bash
ploke-eval campaign export-submissions --campaign <campaign> [--nonempty-only] [--output <path>]
```

## 2) Where outputs appear to live

Default root: `~/.ploke-eval` (`PLOKE_EVAL_HOME` overrides it).

Explicitly documented locations:

- datasets: `~/.ploke-eval/datasets`
- repos: `~/.ploke-eval/repos`
- runs: `~/.ploke-eval/runs`
- batches: `~/.ploke-eval/batches`
- campaigns: `~/.ploke-eval/campaigns`
- model registry files under `~/.ploke-eval/models/`

Run-level artifacts are documented under `~/.ploke-eval/runs/<instance>/`, including:

- `run.json`
- `repo-state.json`
- `execution-log.json`
- `indexing-status.json`
- `snapshot-status.json`
- `multi-swe-bench-submission.jsonl`

Batch preparation writes:

- `~/.ploke-eval/runs/<instance>/run.json`
- `~/.ploke-eval/batches/<batch-id>/batch.json`

`replay-msb-batch` writes `replay-batch-<nnn>.json` beside the run manifest.

## 3) What seems trustworthy vs untrustworthy

More trustworthy:

- per-run artifacts in the chosen run directory
- campaign-level `export-submissions`
- closure state for campaign progress

Less trustworthy:

- batch aggregate `multi-swe-bench-submission.jsonl`
- terminal summary strings

Important boundary from the skill: local `ploke-eval` artifacts are run telemetry, not the benchmark verdict. Official pass/fail comes from the external evaluator on the exported submission.

## 4) What `registry`, `campaign`, and `closure` seem to be

- `registry`: the local typed benchmark target universe; `status` prints the persisted registry, `recompute` rebuilds it from dataset sources.
- `campaign`: a manifest/config wrapper for a target family; it can `show`, `validate`, and `export-submissions` from completed runs in the campaign closure state.
- `closure`: reduced progress state for a campaign across registry, eval, and protocol coverage; `status` reports it, `recompute` rebuilds it, and `advance eval|protocol|all` produces missing artifacts.

## 5) What remains unclear or awkward

- `campaign export-submissions` accepts `--output`, but the help does not state the default destination when omitted.
- `closure advance` and `campaign show` are described only at a high level; the help does not spell out the exact files they read or write.
- The relationship between `closure`, `campaign`, and the batch manifests is still somewhat indirect; I can infer the workflow, but the CLI does not fully map the state transitions.
- `registry` and `closure` expose `status`/`recompute`, but their persisted locations are not called out as clearly as runs/batches/campaigns.
- The top-level help says `run-msb-agent-single` "extends the normal run", but the exact boundary between `run-msb-single` and `run-msb-agent-single` is still only partially explained in help text.

## Questions I would still ask

- What is the default output path for `campaign export-submissions` if `--output` is omitted?
- Is `multi-swe-bench-submission.jsonl` ever safe to treat as authoritative, or only as a local candidate artifact?
- What exact files define closure state on disk?
- Does `closure advance` only compute missing state, or can it also overwrite existing state?
