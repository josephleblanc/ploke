# 2026-05-25 Prototype 1 Same-File Semantic Edit Stale Anchor

## Summary

Prototype 1 eval runs can apply one edit to a file, then immediately stage a
follow-up semantic `apply_code_edit` against the same file using stale indexed
node metadata. The IO layer correctly rejects the follow-up write with
`Content changed for ...`, but the run can still end with a non-empty patch that
omits the intended behavioral change.

This is related to same-file stale-anchor bugs already observed in headless
`ns_patch` flows, but the concrete failure here is in the benchmark eval path:
`insert_rust_item` applies to `util.rs`, then canonical `apply_code_edit`
resolves `crate::util::Replacer::replace_all` from stale DB state and fails
during write.

## Affected Surface

- `crates/ploke-tui/src/tools/insert_rust_item.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-io/src/write.rs`
- Prototype 1 eval runs recorded through `crates/ploke-eval/src/runner.rs`

## Concrete Run

- Campaign: `p1-gemini35-flash-direct-fresh-20260524-163447`
- Instance: `BurntSushi__ripgrep-2209`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063`
- Target checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- Related run review:
  `docs/active/agents/run-reviews/2026-05-24-p1-gemini35-flash-direct-fresh-20260524-163447-eval.md`
- Related protocol/read-side bug:
  `docs/active/bugs/2026-05-24-prototype1-protocol-misses-hidden-apply-failure.md`

The final checkout had a real diff:

```text
crates/printer/src/standard.rs | 23 +++++++++++++++++
crates/printer/src/util.rs     | 57 ++++++++++++++++++++++++++++++++++++++++++
2 files changed, 80 insertions(+)
```

But the behavioral wiring was absent. Direct checkout search found:

```text
util.rs:86  .replace_with_captures_at(
util.rs:462 fn replace_with_captures_at_limited<M, F>(
```

So the new helper existed, but `Replacer::replace_all` still called the old
`replace_with_captures_at` path.

## Event Evidence

The run trace shows the stale-anchor sequence.

First, the model inserted a helper into `crates/printer/src/util.rs`:

```text
idx=364 REQUEST insert_rust_item ... file=crates/printer/src/util.rs
idx=375 COMPLETE insert_rust_item {"applied":1,"ok":true,...}
```

Then it attempted to update `crate::util::Replacer::replace_all` in the same
file:

```text
idx=379 REQUEST apply_code_edit canon=crate::util::Replacer::replace_all
idx=385 COMPLETE apply_code_edit {"ok":true,"staged":1,"applied":0,...}
idx=389 COMPLETE apply_code_edit {"applied":0,"ok":false,"results":[{"error":"Content changed for .../crates/printer/src/util.rs"}]}
```

The run `patch_artifact` recorded the same shape:

```json
{
  "edit_proposals": [
    { "call_id": "function-call-394ef3bc-d047-47b7-aed4-59009df6b52b", "status": "Failed" },
    { "call_id": "function-call-3f0d089d-a3c8-4fb5-87ea-6ae0e8a66be4", "status": "Applied" }
  ],
  "applied": true,
  "all_proposals_applied": false
}
```

## Root Cause

`ploke-io` is doing the right thing. Before a write, it reads the current file,
computes the actual tracking hash, and rejects the write if it differs from the
edit's expected hash:

```text
crates/ploke-io/src/write.rs:431
```

For canonical `apply_code_edit`, the expected hash comes from DB-resolved node
metadata:

```text
crates/ploke-tui/src/rag/tools.rs:910
```

In this run, the first edit changed `util.rs`. The follow-up canonical edit
then used stale metadata for `crate::util::Replacer::replace_all`, so its
`expected_file_hash` no longer matched the live file.

`approve_edits` has a post-apply rescan path, and there is test coverage that a
semantic approval should refresh file hashes before returning:

```text
crates/ploke-tui/src/rag/editing.rs:239
crates/ploke-tui/src/rag/tests/apply_code_edit_tests.rs:1061
```

The observed failure means at least one of these is still true in the live eval
path:

- `insert_rust_item` same-file applies do not produce a sufficiently visible
  DB refresh before the next canonical edit resolves;
- the next tool call can race ahead of post-apply rescan visibility;
- same-file semantic edits can remain staged/admissible even after an earlier
  applied edit invalidates their anchors.

## Relationship To `record.rs`

This is not the same bug as the protocol/read-side `record.rs` issue.

The `record.rs` problem hides the final apply failure by treating the staged
`ToolCallCompleted` as terminal. This bug is the underlying write failure: the
follow-up semantic edit really did fail because its anchor was stale.

Both need fixes:

1. `record.rs` should preserve the final settled lifecycle result.
2. The edit/apply pipeline should avoid or repair stale same-file semantic
   anchors after an earlier applied edit.

## Expected Behavior

After an edit applies to a file, a subsequent same-file semantic edit should
either:

1. resolve against refreshed node metadata and carry the new live file hash;
2. be delayed until post-apply rescan is complete and visible; or
3. be rejected as stale before staging/admission, with a model-visible repair
   instruction to reread or re-resolve the target.

The system should not allow a run to silently proceed with an unused helper and
a missing call-site update when the intended same-file follow-up edit failed.

## Fix Direction

- Add a regression that replays this sequence:
  `insert_rust_item` applied to a file, followed by canonical
  `apply_code_edit` against a node in that same file.
- Assert the second edit either resolves with the refreshed file hash or fails
  before staged success is reported.
- Ensure post-apply rescan visibility is a hard boundary for same-file
  canonical edits in eval/headless tool loops.
- Consider carrying touched-path/version state through the edit proposal
  lifecycle so stale same-file proposals can be rejected before apply.
