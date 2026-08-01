# Campaign Lifecycle

Campaigns are the stateful operator layer for measured work.

Default files:

```text
~/.ploke-eval/campaigns/<campaign>/campaign.json
~/.ploke-eval/campaigns/<campaign>/closure-state.json
```

## Typical flow

```bash
campaign=rust-ripgrep-smoke
cargo run -p ploke-eval -- campaign list
cargo run -p ploke-eval -- campaign init --campaign "$campaign" --from-registry
cargo run -p ploke-eval -- campaign show --campaign "$campaign"
cargo run -p ploke-eval -- campaign validate --campaign "$campaign"
```

Closure commands inspect planned campaign progress across eval/protocol work.
The example uses `--dry-run` so it is copy/paste safe; omit that flag when you
intend to execute eval advancement.

```bash
campaign=rust-ripgrep-smoke
cargo run -p ploke-eval -- closure status --campaign "$campaign"
cargo run -p ploke-eval -- closure advance eval --campaign "$campaign" --dry-run
```

## Export submissions

```bash
campaign=rust-ripgrep-smoke
cargo run -p ploke-eval -- campaign export-submissions --campaign "$campaign"
cargo run -p ploke-eval -- campaign export-submissions --campaign "$campaign" --nonempty-only
```

Default outputs:

```text
~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl
~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.nonempty.jsonl
```

The export reads completed closure rows and each run's per-run
`multi-swe-bench-submission.jsonl`. Prefer this over raw batch aggregate JSONL
when closure state is available.
