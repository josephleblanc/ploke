# Single Run

Use this for one Multi-SWE-bench instance.

Example instance used in existing docs:

```text
BurntSushi__ripgrep-2209
```

## 1. Fetch or refresh the repo checkout

```bash
cargo run -p ploke-eval -- run repo fetch --dataset-key ripgrep
```

Expected state:

```text
~/.ploke-eval/repos/BurntSushi/ripgrep
```

## 2. Prepare the run manifest

```bash
cargo run -p ploke-eval -- run prepare instance \
  --dataset-key ripgrep \
  --instance BurntSushi__ripgrep-2209
```

Expected state:

```text
~/.ploke-eval/instances/BurntSushi__ripgrep-2209/run.json
```

## 3. Run setup-only or the agent path

Setup-only:

```bash
cargo run -p ploke-eval -- run single setup \
  --instance BurntSushi__ripgrep-2209
```

Agent path (live provider/model API):

```bash
cargo run -p ploke-eval -- run single agent \
  --instance BurntSushi__ripgrep-2209
```

Pin an OpenRouter provider when needed; this is also a live provider/model API run:

```bash
cargo run -p ploke-eval -- run single agent \
  --instance BurntSushi__ripgrep-2209 \
  --provider chutes
```

## Inspect the result

```bash
cargo run -p ploke-eval -- transcript
```

The last completed run is recorded in:

```text
~/.ploke-eval/last-run.json
```

## State touched

| Step | Safety | State touched |
| --- | --- | --- |
| repo fetch | `FS` + `NET` | repo checkout and remote refs |
| prepare instance | `FS` | run manifest and normalized task metadata |
| single setup | `FS` + maybe `NET` | repo reset, indexing, DB snapshot, setup logs |
| single agent | `FS` + `NET` + `LIVE-API` | setup artifacts plus agent turn, patch/submission artifacts |
