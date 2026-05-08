# 2026-04-08 Setup Reliability Ripgrep Parse Handoff

- Date: 2026-04-08
- Task title: Ripgrep eval setup reliability and underlying parser failure diagnosis
- Task description: Make eval setup failures fail fast and visibly, ensure `ploke-tui` stays alive when setup parsing fails, and isolate the underlying parser bug affecting ripgrep historical repro runs.
- Related planning files:
  - `docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-plan.md`

## Current Status

We are working the setup-reliability prerequisite before evaluating the higher-level hypothesis about structural tools improving agent outcomes.

The historical repro for ripgrep setup failure is stable:

- Repro command:
  - `cargo test -p ploke-eval test_historical_ripgrep_setup_failure_reports_indexing_failed_and_status_artifact -- --ignored --nocapture`
- Historical run:
  - `BurntSushi__ripgrep-1642`
- Historical base SHA:
  - `ffd4c9c` for the replayed run checkout during the harness test

The harness side now fails fast with a typed indexing failure and writes `indexing-status.json` instead of degrading into a setup timeout. The TUI side now logs the setup failure as a handled warning and keeps the runtime alive.

## Work Completed

### 1. Setup failure no longer degrades into timeout

Implemented previously in:

- `crates/ploke-tui/src/app_state/handlers/indexing.rs`
- `crates/ploke-eval/src/runner.rs`
- `crates/ploke-eval/src/tests/replay.rs`

Behavior now:

- pre-index target-resolution or parse failures emit `IndexStatus::Failed`
- eval runner converts that into `PrepareError::IndexingFailed`
- runner persists `indexing-status.json`

Useful test commands:

- `cargo test -p ploke-tui parse_failure_emits_failed_index_status_before_indexer_runs -- --nocapture`
- `cargo test -p ploke-eval indexing_status_artifact -- --nocapture`
- `cargo test -p ploke-eval test_historical_ripgrep_setup_failure_reports_indexing_failed_and_status_artifact -- --ignored --nocapture`

### 2. TUI runtime now survives setup parse failure

Implemented in:

- `crates/ploke-tui/src/lib.rs`
- `crates/ploke-tui/src/event_bus/mod.rs`
- `crates/ploke-tui/src/app/events.rs`
- `crates/ploke-tui/src/app_state/handlers/indexing.rs`
- `crates/ploke-tui/tests/integration/indexing_freeze_app_loop.rs`

Behavior now:

- `IndexStatus::Failed` is surfaced as a warning containment path
- the warning explicitly states this is temporary and must be fixed before next release
- the app loop remains alive and continues to process input after the failure

Verification commands:

- `cargo fmt --all`
- `cargo test -p ploke-tui ssot_forwards_indexing_failed_once -- --nocapture`
- `cargo test -p ploke-tui parse_failure_emits_failed_index_status_before_indexer_runs -- --nocapture`
- `cargo test -p ploke-tui indexing_setup_failure_emits_warning_and_keeps_app_loop_alive -- --nocapture`

## What We Know About The Underlying Parse Bug

### Confirmed

1. The failing path is in workspace parsing, not in the indexing scheduler.

- `ploke-tui` resolves ripgrep as a workspace in `crates/ploke-tui/src/parser.rs`.
- Workspace parsing runs through `parse_workspace(...)` in `crates/ingest/syn_parser/src/lib.rs`.

2. The parser failure shape is:

- `SynParserError::MultipleErrors`
- containing a nested `PartialParsing`
- with `6 succeeded, 1 failed`

This is visible in:

- `~/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log`

Relevant log lines:

- around `291742`
- around `291823`
- around `291903`

3. The displayed label `"Parse failed for crate: .../crates/cli"` is probably misleading.

Why:

- for workspace targets, `ResolvedIndexTarget.focused_root` is set to the first sorted workspace member
- `format_parse_failure(...)` prints `focused_root`, not the actual child error source path
- later in the same log, the same failure gets surfaced as `.../ripgrep/globset`, which is inconsistent with the earlier `crates/cli` label

This means the current user-facing failure message is not reliably identifying the actual failing member or file.

### Ruled Out

The earlier suspicion that `crates/cli` might be failing because it is a nonstandard crate shape does not currently hold up.

Checked historical tree at commit `7099e174acbcbd940f57e4ab4913fee4040c826e`:

- `crates/cli/Cargo.toml` is a normal crate manifest
- `crates/cli/src/lib.rs` exists

So the current evidence does not support "bin-only or malformed `crates/cli` layout" as the root cause.

### Unknown

We still do not know:

- which exact workspace member produced the nested failure
- which exact source file produced the nested `SynParserError::Syn`
- whether the real issue is:
  - syntax handling
  - module-tree construction
  - target selection / root-file selection
  - another parser invariant failure

## Most Important Insight

The current top-level error reporting is masking the true failure source.

The actual parser bug may be in some specific workspace member and file, but the message shown to the harness is built from `resolved.focused_root`, which is only "the first member" heuristic and not the nested failing file.

That makes the next best step diagnostic, not speculative parser changes.

## Recommended Next Step

Add a replay-oriented diagnostic artifact that flattens nested parser errors from:

- `SynParserError::MultipleErrors`
- `SynParserError::PartialParsing`

Specifically capture:

- child error kind
- `diagnostic_source_path`
- `diagnostic_detail`
- diagnostic emission site
- diagnostic backtrace when present

Then rerun the historical ripgrep replay once and identify the actual failing source file before attempting a parser fix.

## Concrete Next Implementation Slice

1. Extend the setup-failure artifact path to persist nested parser diagnostics.

Suggested locations to inspect:

- `crates/ploke-eval/src/runner.rs`
- `crates/ploke-tui/src/utils/parse_errors.rs`
- `crates/ingest/syn_parser/src/error.rs`

2. If needed, add a helper that recursively flattens `SynParserError`.

Useful fields already exist on the error type in:

- `crates/ingest/syn_parser/src/error.rs`

3. Update the historical replay test to assert the presence of the nested failing source path once it is available.

4. Only after that, write a targeted parser repro test for the true failing file/member.

## Key File References

### Setup containment and logging

- `crates/ploke-tui/src/lib.rs`
- `crates/ploke-tui/src/event_bus/mod.rs`
- `crates/ploke-tui/src/app/events.rs`
- `crates/ploke-tui/src/app_state/handlers/indexing.rs`
- `crates/ploke-tui/tests/integration/indexing_freeze_app_loop.rs`

### Eval harness failure typing and replay

- `crates/ploke-eval/src/runner.rs`
- `crates/ploke-eval/src/tests/replay.rs`
- `crates/ploke-eval/src/spec.rs`

### Parser resolution and parse entrypoints

- `crates/ploke-tui/src/parser.rs`
- `crates/ploke-tui/src/utils/parse_errors.rs`
- `crates/ingest/syn_parser/src/lib.rs`
- `crates/ingest/syn_parser/src/error.rs`
- `crates/ingest/syn_parser/src/discovery/workspace.rs`
- `crates/ingest/syn_parser/src/discovery/single_crate.rs`

### Historical artifacts and logs

- `~/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log`
- `~/.ploke-eval/runs/BurntSushi__ripgrep-1642/run.json`

## Relevant Evidence From Current Investigation

### Resolution behavior

In `crates/ploke-tui/src/parser.rs`:

- workspace resolution uses the first workspace member as `focused_root`
- this is not guaranteed to be the failing member

### Error formatting behavior

In `crates/ploke-tui/src/utils/parse_errors.rs`:

- the displayed `"Parse failed for crate: ..."` message is built from `target_dir`
- for workspace failures that `target_dir` is currently `resolved.focused_root`

### Workspace parsing behavior

In `crates/ingest/syn_parser/src/lib.rs`:

- member parse results are partitioned into successes and errors
- any errors cause `SynParserError::MultipleErrors(errors)`

### Workspace member ordering

In `crates/ingest/syn_parser/src/discovery/workspace.rs`:

- workspace members are sorted
- therefore `focused_root` is a stable ordering artifact, not proof of actual blame

## Cautions For The Next Session

- Do not assume `crates/cli` is the actual failing member just because it appears in the top-level message.
- Do not loosen parser correctness or silently skip broken members without explicit approval.
- Prefer extracting real nested diagnostics before making parser changes.
- The TUI containment path is intentionally temporary. It is not the final fix.

## Good Resume Prompt

If resuming after compaction, a good next prompt is:

"Continue from `docs/active/agents/2026-04-08_setup-reliability-ripgrep-parse-handoff.md`. Add nested parser diagnostic extraction for the historical ripgrep setup replay so we can identify the actual failing member/file behind the misleading `focused_root` label, then rerun the replay and report the concrete failing source path."
