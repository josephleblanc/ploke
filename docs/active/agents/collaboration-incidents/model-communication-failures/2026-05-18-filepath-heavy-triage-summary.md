# Filepath-Heavy Triage Summary

## Trigger

During Prototype 1 harness failure triage, the user pointed out that the
diagnosis paragraph was hard to read because more than half of the visible words
were long absolute file paths.

## User-Visible Failure

The answer had the correct basic causal chain, but it embedded repeated absolute
paths inline:

- `tui_adapter.rs` was cited three times with full worktree prefixes.
- `cli_facing.rs` was cited with the same long prefix.
- The reasoning sentence became a wall of path text instead of a readable
diagnosis.

## Touched Code Surface

No code was changed by the faulty answer. The affected surface was the
operator-facing triage report for a Prototype 1 broad headless-TUI failure.

## What The Agent Did

The agent used file references as inline prose instead of separating the causal
explanation from supporting evidence. That made the conclusion harder to scan
even though the evidence itself was relevant.

## Skipped Docs / Skills / Instructions

The response followed verification-surface honesty but failed the practical
communication requirement to make the verdict readable. The missing discipline
was evidence formatting, not evidence gathering.

## Why This Was Risky

Prototype 1 triage often involves long campaign paths. If the answer lets those
paths dominate the sentence, the operator has to parse formatting noise before
they can see the actual failure mode and next action. That increases the chance
that a useful diagnosis is ignored or misread.

## Prevention Rule

When citing long campaign/worktree paths in a triage answer, keep the reasoning
sentence path-light. Use short labels such as `tui_adapter.rs:620` in prose, and
put full clickable paths in a compact evidence list only when the user needs to
jump to the file.

## Memory Hypothesis

No memory update is needed. This is a local response-formatting failure that the
collaboration incident ledger can capture.
