# 2026-05-17: NsPatch Recovery Work Became Incoherent

## Trigger

The user interrupted an implementation turn for `non_semantic_patch` recovery
with: "Whatever you were doing it is becoming incoherent. Explain."

## User-visible failure

The agent resumed from a compacted handoff and continued changing code and tests
without first restating the live worktree boundary: which files were part of the
requested slice, which dirty files were pre-existing, which tests had already
failed, and which verification was still pending.

## Touched code surface

- `crates/ploke-tui/src/tools/validators.rs`
- `crates/ploke-tui/src/tools/ns_patch.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/tools/tool_tests/patches.rs`
- `crates/ploke-tui/src/llm/manager/semantics.rs`
- `crates/ploke-core/tool_text/non_semantic_patch.md`
- `crates/ploke-eval/src/cli.rs`

The live worktree also contained unrelated dirty files in `ploke-egui`,
`ploke-records`, and archaeology docs. Treating the full dirty stat as one
undifferentiated implementation state made the work look broader than the
intended slice.

## What the agent did

The agent carried forward a technically plausible implementation plan:
parser-backed unified diff validation, structured malformed-diff retry context,
single terminal failure event routing, semantic repair preflight coverage, and
eval reporting that distinguishes staged from applied patches.

The mistake was communication and control of the turn. The agent did not pause
after compaction to produce a compact state ledger before continuing with more
edits and tests, even though the repo was already dirty and the slice had a
specific "do not edit ploke-egui" boundary.

## Skipped docs / skills / instructions

- The agent did not apply the collaboration-incident workflow until the user
  explicitly objected.
- The agent did not front-load the AGENTS.md verification-surface rule when
  switching from implementation to status/explanation.
- The agent treated the prior compacted summary as enough operational context
  without first reconciling it against `git status` and the live diff.

## Why this was risky

The implementation surface is a tool-result lifecycle path where confusing
`staged` with `applied`, or duplicate failure events with dispatcher-owned
failure emission, is itself the bug class being fixed. A confusing worktree
story undermines the same invariant at the collaboration level: the user cannot
tell whether the agent is tightening the terminal event path or adding more
unbounded surface area.

## Concrete prevention rule

After an interruption, context compaction, or user concern during an active
implementation in a dirty worktree, stop before further edits and emit a short
state ledger:

- intended slice
- files intentionally touched by this turn
- dirty files known to be pre-existing or out of scope
- commands already run and their result
- commands still pending

Only resume implementation after that ledger is clear.

## Memory hypothesis

Prior memory about staged proposal lifecycle was useful for selecting the right
technical direction, but it also increased the risk of continuing from an
implicit story instead of verifying and presenting the live worktree boundary.
