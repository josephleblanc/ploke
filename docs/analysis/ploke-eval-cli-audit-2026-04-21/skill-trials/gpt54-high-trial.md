# ploke-eval CLI audit from skill + help only

Scope: this note is based only on `docs/workflow/skills/ploke-eval-operator/SKILL.md` and `./target/debug/ploke-eval` help output.

## 1. Commands I would run

### One-instance eval

The skill is explicit that the default execution primitive is `run-msb-agent-single`, and the top-level help gives the same shape in its quick-start example.

Recommended path:

```bash
./target/debug/ploke-eval list-msb-datasets
./target/debug/ploke-eval fetch-msb-repo --dataset-key ripgrep
./target/debug/ploke-eval model current
./target/debug/ploke-eval model providers
./target/debug/ploke-eval prepare-msb-single --dataset-key ripgrep --instance BurntSushi__ripgrep-2209
./target/debug/ploke-eval run-msb-agent-single --instance BurntSushi__ripgrep-2209 --provider chutes
```

If I wanted the non-agent setup/artifact pass first, I would run:

```bash
./target/debug/ploke-eval run-msb-single --instance BurntSushi__ripgrep-2209
```

### Batch / family progress

The skill says to prefer campaign + closure for stateful family progress, not raw batch commands. The commands I would actually use are:

```bash
./target/debug/ploke-eval campaign show --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval closure status --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval closure advance eval --campaign rust-baseline-grok4-xai --dry-run
./target/debug/ploke-eval closure advance eval --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval closure status --campaign rust-baseline-grok4-xai
```

If I specifically needed raw batch prep/run instead of closure-driven progress:

```bash
./target/debug/ploke-eval prepare-msb-batch --dataset-key ripgrep --specific 2209
./target/debug/ploke-eval run-msb-agent-batch --batch-id ripgrep-2209
```

### Export

For campaign-scope export:

```bash
./target/debug/ploke-eval campaign export-submissions --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval campaign export-submissions --campaign rust-baseline-grok4-xai --nonempty-only
```

## 2. Where outputs appear to live

From `ploke-eval --help`:

- Eval home: `~/.ploke-eval`
- Dataset cache: `~/.ploke-eval/datasets`
- Repo cache: `~/.ploke-eval/repos/<org>/<repo>`
- Per-run artifacts: `~/.ploke-eval/runs/<instance>`
- Batch artifacts: `~/.ploke-eval/batches/<batch-id>`
- Campaign manifest: `~/.ploke-eval/campaigns/<campaign>/campaign.json`
- Closure state: apparently `~/.ploke-eval/campaigns/<campaign>/closure-state.json` because `campaign init --from-closure-state` names that path directly

Per-run outputs explicitly surfaced by help:

- `run.json`
- `repo-state.json`
- `execution-log.json`
- `indexing-status.json`
- `snapshot-status.json`
- `multi-swe-bench-submission.jsonl`
- `indexing-checkpoint.db`
- `indexing-failure.db`
- `config/` sandbox under the run directory

Batch outputs explicitly surfaced by help:

- `~/.ploke-eval/batches/<batch-id>/batch.json`
- `batch-run-summary.json`
- `multi-swe-bench-submission.jsonl`

What is not clearly surfaced:

- the persisted file path for the typed `registry`
- the default output path used by `campaign export-submissions` when `--output` is omitted

## 3. Trustworthy vs untrustworthy artifacts

The skill gives the clearest trust ordering:

1. per-run artifacts in the chosen run directory
2. campaign-level export from `campaign export-submissions`
3. closure state for campaign progress
4. batch aggregate `multi-swe-bench-submission.jsonl`
5. terminal summary strings

So the trustworthy things seem to be:

- the chosen run directory and its per-run artifacts
- the per-run `multi-swe-bench-submission.jsonl`
- `campaign export-submissions` when working at campaign scope
- `closure status` / closure state for progress accounting

The less trustworthy or explicitly non-authoritative things seem to be:

- batch aggregate `multi-swe-bench-submission.jsonl`, especially if batches were rerun, interrupted, or overlapped
- terminal success/summary text
- local telemetry as proof of benchmark success

The top-level help is also explicit about benchmark truth boundaries: local `ploke-eval` artifacts are telemetry, and official pass/fail only comes from the external Multi-SWE-bench evaluator on the exported submission.

## 4. What `registry`, `campaign`, and `closure` seem to be

- `registry`: the persisted local typed benchmark target registry, recomputed from dataset sources. It looks like the local universe of known targets/rows.
- `campaign`: a named operator manifest under `~/.ploke-eval/campaigns/<campaign>/campaign.json`. It appears to bind datasets, model/provider overrides, required procedures, and run/batch roots into one reusable scope.
- `closure`: a reduced campaign state that tracks staged coverage across registry, eval, and protocol work. It has `recompute`, `status`, and `advance`, and `campaign export-submissions` exports from completed runs in this closure state.

My working interpretation is: `registry` defines what exists, `campaign` defines what this effort intends to cover and how, and `closure` records how much of that intent has actually been completed.

## 5. What remains unclear or awkward

- `campaign export-submissions` does not say where it writes by default when `--output` is omitted.
- `registry` is described as persisted, but the CLI help does not reveal the actual file path.
- `closure status` and `closure recompute` describe state, but the concrete artifact path is only indirectly discoverable through `campaign init --from-closure-state`.
- The CLI exposes both raw batch commands and closure-driven campaign workflows; the help does not strongly explain when to choose one over the other, while the skill does.
- `run-msb-agent-single` says it writes a turn trace and summary beside run artifacts, but it does not name the files.
- The help makes clear that local submission JSONL is only a candidate patch artifact, but it does not say whether campaign export is just a concatenation of per-run JSONL or performs additional filtering/selection beyond closure state membership.

## Questions I would still ask

- What exact file paths back the persisted target `registry`?
- What exact path does `campaign export-submissions` write to by default?
- What are the exact filenames for the turn trace and summary produced by `run-msb-agent-single`?
- When should an operator prefer raw batch commands over `closure advance eval`, beyond "batch orchestration is the point"?
