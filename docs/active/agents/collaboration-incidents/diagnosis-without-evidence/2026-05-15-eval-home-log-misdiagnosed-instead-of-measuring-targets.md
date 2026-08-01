# 2026-05-15 Eval Home Log Misdiagnosed Instead of Measuring Targets

## Trigger

The user said the `prototype1-loop-runtime` eval-home size log was "getting way
too big" and wanted to understand what was happening with
`.codex/skills/prototype1-loop-runtime/references/eval-home-size-log.md`.

## User-Visible Failure

The agent answered by analyzing the markdown log file itself and proposing log
retention/deduplication policy before directly checking the actual storage
driver inside `~/.ploke-eval`. The user had to correct this and point out that
the real pressure was almost certainly accumulated `target/` directories.

## Touched Surface

- `.codex/skills/prototype1-loop-runtime/references/eval-home-size-log.md`
- `~/.ploke-eval` disk-usage triage during Prototype 1 campaign setup

## What the Agent Did

- Read the log file and reported that it was only `8K`.
- Framed the problem as semantic bloat in the append-only log.
- Gave cleanup-policy advice before measuring `target/` directory usage.

## Skipped Docs / Skills / Instructions

- Skipped the practical implication of the `prototype1-loop-runtime` guidance:
  the log is only a checkpoint, not the storage object that matters.
- Ignored the obvious next bounded observation step: measure `target/`
  directories under `~/.ploke-eval` before advising policy changes.

## Why This Was Risky

It pushed the conversation toward the wrong object. The log file was not the
source of disk pressure. Treating the symptom as a logging-policy problem wasted
time and forced the user to restate the likely root cause.

## Prevention Rule

When a user asks why eval-home footprint is growing, measure the actual largest
directories under `~/.ploke-eval` first. Do not pivot to log-file policy unless
bounded storage measurements show the log is materially relevant.
