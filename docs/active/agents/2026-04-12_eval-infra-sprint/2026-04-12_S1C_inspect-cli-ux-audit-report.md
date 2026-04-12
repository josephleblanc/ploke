# S1C Report Back

- date: 2026-04-12
- worker: Codex
- status: reported

## implemented

- Added a small, safe help-text improvement in `crates/ploke-eval/src/cli.rs` so `inspect --help` now shows bootstrap-oriented examples for `inspect turn --show db-state`, `inspect query --lookup`, and `inspect conversations`.
- Tightened `inspect turn` help text so the `--show` description includes `tool-result` as a supported value.

## claims

- [1] The inspect-oriented CLI can answer the core bootstrap questions without implementation spelunking: run history, tool-call inventory, DB snapshot timestamps, explicit turn state, and symbol lookup at a turn timestamp.
- [2] The surface is reasonably discoverable once a user reaches `inspect`, but the best bootstrap path is still split across `inspect conversations`, `inspect turn --show db-state`, and `inspect query --lookup`, so the help text needs to do some of the teaching.
- [3] There is one misleading UX hole: empty `inspect turn --show messages` output still says `No messages available (placeholder implementation).`, which makes the surface feel unfinished even when the command is otherwise valid.
- [4] `transcript` is useful as a quick “most recent run” context dump, but it is not a general inspect surface because it has no run selector or query narrowing.

## evidence

- `crates/ploke-eval/src/cli.rs:1142-1162` now includes `inspect` bootstrap examples in `after_help`.
- `crates/ploke-eval/src/cli.rs:1262-1286` now lists `tool-result` in the `--show` description.
- Sample commands against `BurntSushi__ripgrep-2209` all worked cleanly:
  - `cargo run -p ploke-eval -- inspect conversations --instance BurntSushi__ripgrep-2209`
  - `cargo run -p ploke-eval -- inspect tool-calls --instance BurntSushi__ripgrep-2209`
  - `cargo run -p ploke-eval -- inspect db-snapshots --instance BurntSushi__ripgrep-2209`
  - `cargo run -p ploke-eval -- inspect failures --instance BurntSushi__ripgrep-2209`
  - `cargo run -p ploke-eval -- inspect turn --instance BurntSushi__ripgrep-2209 --turn 1 --show db-state`
  - `cargo run -p ploke-eval -- inspect query --instance BurntSushi__ripgrep-2209 --turn 1 --lookup GlobSet`
- Observed outputs were concrete and usable:
  - `conversations` reported 1 turn, 15 tool calls, and outcome `content`.
  - `turn --show db-state` printed the DB timestamp and the next-step hint for `inspect query`.
  - `query --lookup GlobSet` returned JSON for the `GlobSet` struct without requiring code inspection.
  - `tool-calls`, `db-snapshots`, and `failures` all produced direct summaries from the run record.
  - `doctor` reported a healthy local eval environment, which is helpful context but not run inspection.

## unsupported_claims

- I did not verify every `inspect` JSON output variant.
- I did not audit the entire `model` or `doctor` surface beyond the sampled bootstrap commands.
- I did not establish whether the CLI can answer arbitrary natural-language questions without Cozo syntax or prior command knowledge.

## not_checked

- `inspect turn --show tool-result`
- `inspect query` with raw Cozo text instead of `--lookup`
- `transcript` on older runs or with intentionally missing history
- Any tests that assert the help text itself

## risks

- The current inspect surface still leaks a little implementation language through the empty-message placeholder and through the need to know which subcommand family to use.
- `transcript` is context-rich but not searchable, so it can still fail as a quick bootstrap path when someone needs a specific turn or artifact.
- `inspect query` is powerful, but it still assumes the user knows either Cozo or the `--lookup` shortcut.

## next_step

- Open a small follow-up packet for inspect-CLI polish: replace the empty-message placeholder, add one or two more command-choice examples for the common bootstrap questions, and add a lightweight help/output test if that stays within `crates/ploke-eval/`.
