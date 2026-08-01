# Run Lifecycle

A prepared run moves through four common stages.

## 1. Repo checkout

```bash
cargo run -p ploke-eval -- run repo fetch --dataset-key ripgrep
```

Writes or refreshes:

```text
~/.ploke-eval/repos/<owner>/<repo>
```

## 2. Manifest preparation

```bash
cargo run -p ploke-eval -- run prepare instance \
  --dataset-key ripgrep \
  --instance BurntSushi__ripgrep-2209
```

Writes:

```text
~/.ploke-eval/instances/<instance-id>/run.json
```

Source excerpt for the default instance/attempt layout:

```rust,ignore
{{#include ../../src/layout.rs:ploke_eval_instances_dir}}
{{#include ../../src/run_registry.rs:ploke_eval_attempt_runs_dir}}
```

## 3. Setup

```bash
cargo run -p ploke-eval -- run single setup --instance BurntSushi__ripgrep-2209
```

Common outputs:

```text
repo-state.json
execution-log.json
indexing-status.json
snapshot-status.json
config/
```

## 4. Agent turn

This is a live provider/model API run.

```bash
cargo run -p ploke-eval -- run single agent --instance BurntSushi__ripgrep-2209
```

Common outputs include agent telemetry plus:

```text
multi-swe-bench-submission.jsonl
```

Official benchmark pass/fail comes from the external Multi-SWE-bench evaluator
running on exported submission JSONL, not from local `ploke-eval` telemetry.

## Inspection

```bash
cargo run -p ploke-eval -- transcript
cargo run -p ploke-eval -- inspect --help
```
