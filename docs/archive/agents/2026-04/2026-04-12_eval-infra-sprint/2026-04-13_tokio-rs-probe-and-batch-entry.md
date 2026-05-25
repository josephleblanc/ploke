- date: 2026-04-13
- mode: surgical implementation note
- control_plane: [2026-04-12_eval-infra-sprint-control-plane.md](./2026-04-12_eval-infra-sprint-control-plane.md)
- workstream: `A2` / `A3` / `A4` / `H0`
- scope: select `tokio-rs` as the second Rust repo family, run one bounded probe, and leave the next batch move explicit

## Why This Exists

We needed a narrow second-target expansion record after the ripgrep rollup so the next batch could launch from a reviewed probe rather than from registry guesses.

## Selection

`tokio-rs` was chosen as the next Rust repo family because it is medium-small and coherent:

- the upstream harness exposes five visible instances under `github-project/multi_swe_bench/harness/repos/rust/tokio_rs/`
- the visible family is:
  - `tokio-rs__bytes-*`
  - `tokio-rs__tokio-*`
  - `tokio-rs__tracing-*`
  - `tokio-rs__tokio_6229_to_4434-*`
  - `tokio-rs__tracing_2442_to_853-*`
- compared with `clap_rs` and `sharkdp`, it presented lower immediate toolchain churn and a cleaner first expansion surface

## Probe Setup

The reviewed probe used:

- dataset file: [tokio-rs__tokio_dataset.jsonl](/home/brasides/.ploke-eval/datasets/tokio-rs__tokio_dataset.jsonl)
- repo checkout: [~/.ploke-eval/repos/tokio-rs/tokio](/home/brasides/.ploke-eval/repos/tokio-rs/tokio)
- instance: `tokio-rs__tokio-6618`

Commands executed:

```bash
./target/debug/ploke-eval prepare-msb-single \
  --dataset /home/brasides/.ploke-eval/datasets/tokio-rs__tokio_dataset.jsonl \
  --instance tokio-rs__tokio-6618 \
  --repo-cache /home/brasides/.ploke-eval/repos

./target/debug/ploke-eval run-msb-agent-single --instance tokio-rs__tokio-6618
```

## Probe Outcome

The probe completed successfully and produced a full artifact set under [tokio-rs__tokio-6618](/home/brasides/.ploke-eval/runs/tokio-rs__tokio-6618).

High-signal results:

- `inspect turns` reports:
  - turns: `1`
  - tools: `5`
  - failed: `0`
  - outcome: `completed`
  - wall time: `145.100s`
  - token usage from raw sidecar: `prompt:47676 completion:18057 total:65733`
- `inspect tool-calls` shows five successful tool calls:
  - `read_file`
  - `list_dir`
  - `read_file`
  - `list_dir`
  - `read_file`
- the run wrote:
  - `execution-log.json`
  - `record.json.gz`
  - `agent-turn-summary.json`
  - `agent-turn-trace.json`
  - `llm-full-responses.jsonl`
  - `multi-swe-bench-submission.jsonl`
  - `final-snapshot.db`

## Interpretation

The probe did not reveal a hard parser, modeling, or runner blocker.

What it did reveal is a broader operational profile than ripgrep:

- indexing completed successfully but produced a larger workspace footprint (`387` docs staged in BM25 finalization)
- legacy parse mode skipped many non-primary targets across several crates during indexing
- the run still remained interpretable enough to treat `tokio-rs` as a fair next batch target

So the registry move is:

- keep `tokio-rs__tokio` at `watch`
- set `run_policy` to `default_run`
- treat the remaining uncertainty as scaling/breadth watchfulness, not a batch blocker

## Next Move

The next safe move is:

1. keep `tokio-rs__tokio` in the target-capability registry at `watch` / `default_run`
2. prepare a fresh `tokio-rs` batch id over the visible `tokio-rs` instance family
3. run that batch
4. only tighten to `subset_only` if follow-on runs show repeated scaling or coverage instability
