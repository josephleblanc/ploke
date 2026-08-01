# Semantic Edit Can Be Marked Applied With Zero Writes

## Summary

`apply_semantic_edit` could present and persist a semantic edit as `Applied` even when zero writes landed (`applied == 0`). This is a status regression in the semantic approval path, not a final patch-capture bug by itself.

## Code References

- Semantic apply path: [crates/ploke-tui/src/rag/editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:270)
- Write result counting: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:294)
- Success payload derived from `applied > 0`: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:311)
- Proposal status branch now gated on `applied > 0`: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:318)
- UI payload status now switches between `applied` and `failed`: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:335)
- Follow-up message now distinguishes `No edits were applied`: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:368)
- Rescan now only happens when at least one edit lands: [editing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/editing.rs:378)

## Focused Test Shape

- Repro test: [crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs:735)
- Test shape:
  - stage a semantic `apply_code_edit` proposal
  - mutate the target file after staging
  - call `approve_edits`
  - assert the proposal ends as `EditProposalStatus::Failed(_)`, not `Applied`

This targets the stale-content case where semantic staging succeeded, but approval-time writes no longer match the file contents.

## Why This Polluted Eval Patch Artifacts / Summaries

`ploke-eval` snapshots proposal statuses into patch artifacts and was treating any `Applied` proposal label as evidence that a patch had been applied:

- Proposal snapshotting: [crates/ploke-eval/src/runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:755)
- `applied` derived from proposal status strings: [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:801)

That meant a semantic proposal could be recorded as applied in eval metadata even when the actual write count was zero and the final repo diff stayed empty. This inflated patch-artifact summaries and confused empty-final-patch diagnosis.

## Current Worktree State

The worktree appears to contain a fix already.

- `apply_semantic_edit` now sets `EditProposalStatus::Applied` only when `applied > 0`
- zero-write approval now becomes `EditProposalStatus::Failed("No semantic edits were applied")`
- the focused regression test exists in the worktree and covers this exact seam
