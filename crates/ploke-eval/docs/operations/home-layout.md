# Home Layout

By default, `ploke-eval` stores state under:

```text
~/.ploke-eval/
  datasets/   downloaded benchmark JSONL files
  repos/      benchmark repo checkouts
  models/     model registry, active model, provider preferences
  instances/  per-instance manifests and nested run artifacts
  batches/    batch manifests, summaries, aggregate exports
  campaigns/  campaign manifests, closure state, Prototype 1 state
```

Override the root with:

```bash
PLOKE_EVAL_HOME=/tmp/ploke-eval-scratch cargo run -p ploke-eval -- doctor
```

## Trust order

When artifacts disagree, use this practical trust order for normal benchmark
work:

```text
per-run artifacts > campaign export-submissions > closure state > batch aggregate JSONL
```

For Prototype 1 handoff authority, use the narrower Prototype 1 rules:
sealed History and typed transition evidence outrank scheduler/debug
projections. See [Operator Map](../prototype1/operator-map.md).
