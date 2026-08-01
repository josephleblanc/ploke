Short audit from the current `ploke-eval` help surface only.

1. One instance
```bash
./target/debug/ploke-eval fetch-msb-repo --dataset-key ripgrep
./target/debug/ploke-eval prepare-msb-single --dataset-key ripgrep --instance BurntSushi__ripgrep-2209
./target/debug/ploke-eval run-msb-agent-single --instance BurntSushi__ripgrep-2209
```

2. Campaign/family progress
```bash
./target/debug/ploke-eval campaign show --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval closure status --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval closure advance eval --campaign rust-baseline-grok4-xai
./target/debug/ploke-eval campaign export-submissions --campaign rust-baseline-grok4-xai
```

3. Yes.
The typed target registry lives at `~/.ploke-eval/registries/multi-swe-bench-rust.json`.
The closure state lives at `~/.ploke-eval/campaigns/<campaign>/closure-state.json`.
The default campaign export lives at `~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl`
or `.nonempty.jsonl` with `--nonempty-only`.

4. Trust hierarchy is clear.
Trust per-run artifacts first, then campaign export output, then closure state, then the raw batch aggregate `multi-swe-bench-submission.jsonl`, and least of all terminal summary strings.
Do not treat the batch aggregate as authoritative if the batch was rerun, interrupted, or overlapped.
Local `ploke-eval` artifacts are telemetry; the external Multi-SWE-bench evaluator is the final verdict.

Remaining ambiguities after the help patch:
- `campaign show` prints resolved config, but not every resolution rule or precedence edge.
- `closure status` is a reduced view, but the exact reduction logic is still not fully spelled out.
- `registry recompute` and `closure advance eval` are named clearly, but the boundary between registry inventory refresh and campaign progress refresh is still implicit.
- `run-msb-single` vs `run-msb-agent-single` is clearer than before, but the full cold-start operator path still spans multiple commands.
