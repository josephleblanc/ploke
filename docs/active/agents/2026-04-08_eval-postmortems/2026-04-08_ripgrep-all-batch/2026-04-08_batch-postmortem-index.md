# Ripgrep Batch Postmortem Index

- date: 2026-04-08
- task title: Ripgrep Batch Eval Postmortems
- task description: Track per-instance post-mortems for the sequential `ripgrep-all` batch eval and capture cross-run notes about gaps in tooling, artifacts, and report structure.
- related planning files:
  - [2026-04-08_postmortem-plan.md](../2026-04-08_postmortem-plan.md)

## Batch Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- dataset file: [/home/brasides/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl](/home/brasides/.ploke-eval/datasets/BurntSushi__ripgrep_dataset.jsonl)
- aggregate submission: [/home/brasides/.ploke-eval/batches/ripgrep-all/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/batches/ripgrep-all/multi-swe-bench-submission.jsonl)
- batch summary: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch-run-summary.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch-run-summary.json)
- selected model: `qwen/qwen3.6-plus`
- selected provider: `alibaba`

## Directory Convention

- One directory per instance under this batch directory.
- Each instance directory should contain one markdown report using the post-mortem template.
- Each report should link directly to the run artifacts used as evidence.

## Instance Status

- `BurntSushi__ripgrep-2626`: report completed at [2026-04-08_BurntSushi__ripgrep-2626.md](BurntSushi__ripgrep-2626/2026-04-08_BurntSushi__ripgrep-2626.md)
- `BurntSushi__ripgrep-2610`: report completed at [2026-04-08_BurntSushi__ripgrep-2610_postmortem.md](BurntSushi__ripgrep-2610/2026-04-08_BurntSushi__ripgrep-2610_postmortem.md)
- `BurntSushi__ripgrep-2610` batch-specific qwen/alibaba report completed at [2026-04-08_BurntSushi__ripgrep-2610_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2610/2026-04-08_BurntSushi__ripgrep-2610_qwen-qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-2209`: report completed at [2026-04-08_BurntSushi__ripgrep-2209_minimax-m2.5_postmortem.md](BurntSushi__ripgrep-2209/2026-04-08_BurntSushi__ripgrep-2209_minimax-m2.5_postmortem.md)
- `BurntSushi__ripgrep-2488`: report completed at [2026-04-08_BurntSushi__ripgrep-2488_qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2488/2026-04-08_BurntSushi__ripgrep-2488_qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-2295`: report completed at [2026-04-08_BurntSushi__ripgrep-2295_qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2295/2026-04-08_BurntSushi__ripgrep-2295_qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-1980`: report completed at [2026-04-08_BurntSushi__ripgrep-1980_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-1980/2026-04-08_BurntSushi__ripgrep-1980_qwen-qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-454`: report completed at [2026-04-08_BurntSushi__ripgrep-454_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-454/2026-04-08_BurntSushi__ripgrep-454_qwen-qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-1642`: report completed at [2026-04-08_BurntSushi__ripgrep-1642_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-1642/2026-04-08_BurntSushi__ripgrep-1642_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-1367`: report completed at [2026-04-08_BurntSushi__ripgrep-1367_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-1367/2026-04-08_BurntSushi__ripgrep-1367_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-1294`: report completed at [2026-04-08_BurntSushi__ripgrep-1294_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-1294/2026-04-08_BurntSushi__ripgrep-1294_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-954`: report completed at [2026-04-08_BurntSushi__ripgrep-954_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-954/2026-04-08_BurntSushi__ripgrep-954_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-727`: report completed at [2026-04-08_BurntSushi__ripgrep-727_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-727/2026-04-08_BurntSushi__ripgrep-727_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-723`: report completed at [2026-04-08_BurntSushi__ripgrep-723_qwen3.6plus-alibaba_indexing-timeout_postmortem.md](BurntSushi__ripgrep-723/2026-04-08_BurntSushi__ripgrep-723_qwen3.6plus-alibaba_indexing-timeout_postmortem.md)
- `BurntSushi__ripgrep-2626` batch-specific qwen/alibaba report completed at [2026-04-08_BurntSushi__ripgrep-2626_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2626/2026-04-08_BurntSushi__ripgrep-2626_qwen-qwen3.6-plus_alibaba_postmortem.md)

Batch-specific qwen/alibaba comparison reports:

- `BurntSushi__ripgrep-2209`: report completed at [2026-04-08_BurntSushi__ripgrep-2209_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2209/2026-04-08_BurntSushi__ripgrep-2209_qwen-qwen3.6-plus_alibaba_postmortem.md)
- `BurntSushi__ripgrep-2576`: report completed at [2026-04-08_BurntSushi__ripgrep-2576_qwen-qwen3.6-plus_alibaba_postmortem.md](BurntSushi__ripgrep-2576/2026-04-08_BurntSushi__ripgrep-2576_qwen-qwen3.6-plus_alibaba_postmortem.md)

Additional instances should be added here as the batch completes.

- `BurntSushi__ripgrep-2576`: report completed at [2026-04-08_BurntSushi__ripgrep-2576_postmortem.md](BurntSushi__ripgrep-2576/2026-04-08_BurntSushi__ripgrep-2576_postmortem.md)
