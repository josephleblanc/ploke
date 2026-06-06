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

## Current Status

Partially fixed in source. `stage_semantic_edit_proposal` now treats
`read_full_verified` failure as a terminal staging failure for semantic edits:
the tool emits `ToolCallFailed` and does not create a pending proposal when the
DB-derived `expected_file_hash` does not verify against the live file.

Regression coverage:

```text
cargo test -q -p ploke-tui apply_code_edit_rejects_stale_semantic_anchor_before_staging -- --nocapture
```

Historical replay probe:

```text
./target/debug/ploke-eval run replay turn-live \
  --run-dir /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063 \
  --workspace /tmp/ploke-ripgrep-replay-stale-anchor \
  --event-index 389 \
  --through-event \
  --tail stop \
  --max-attempts 1 \
  --timeout-secs 180 \
  --format json
```

The replay used a throwaway ripgrep worktree at the recorded base SHA. With the
fix applied, the historical provider prefix still applied the first
`insert_rust_item` to `crates/printer/src/util.rs`, but the follow-up
`apply_code_edit` no longer staged a bogus pending proposal. It emitted a
model-facing `ToolErrorWire` before staging:

```text
Cannot stage apply_code_edit for .../crates/printer/src/util.rs because the file version could not be verified: Content changed for ".../crates/printer/src/util.rs". Refresh or re-resolve the target before submitting another semantic edit.
```

Broader same-file composition remains a design risk: multiple same-file
proposals can still be staged before an earlier auto-confirmed proposal has
finished applying and refreshing the index. That should be handled as proposal
set/file-version planning, not by weakening the stale-anchor rejection added
here.

Follow-up fixed on 2026-05-25: the model-facing `request_code_context` path no
longer silently treats stale snippet IO as clean success. Lenient RAG context
assembly now records skipped snippet IO failures, and `request_code_context`
turns those into an explicit degraded-context note with refresh/re-resolve
steps. This addresses the related failure mode where a same-file edit leaves
retrieval stale and the model sees missing context without a stale-index signal.
It does not replace the same-file proposal planning work above.

Follow-up fixed on 2026-05-25: partial non-semantic mutation is no longer stored
as an ordinary `Failed` proposal. Mixed-result `ns_patch` batches now settle as
`PartiallyApplied`, remain terminal, force Prototype 1 headless refresh waiting,
and are recorded as mutation evidence without being treated as cleanly applied
candidate admission. The same-file fuzzy stale guard now keys off mutating
settled proposals (`Applied` or `PartiallyApplied`) rather than all failed/stale
proposals, so no-op failures do not poison later same-file staging.

Regression coverage:

```text
cargo test -p ploke-tui ns_patch_same_file_batch_partially_applies_then_fails_due_to_shared_stale_anchor -- --nocapture
cargo test -p ploke-tui ns_patch_allows_fuzzy_same_file_after_non_mutating_failed_proposal -- --nocapture
cargo test -p ploke-tui approve_edits_marks_mixed_result_ns_batch_as_partially_applied -- --nocapture
cargo test -p ploke-eval collect_patch_artifact_marks_partial_apply_as_applied_but_not_all_applied -- --nocapture
```

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

- already-stale canonical semantic anchors were allowed to stage because
  `stage_semantic_edit_proposal` substituted `"<unreadable or binary file>"`
  when `read_full_verified` rejected the expected file hash;
- the next tool call may still race ahead of post-apply rescan visibility if it
  stages before the earlier same-file proposal has actually applied;
- same-file semantic edit composition is still represented as independent
  proposals rather than a per-file versioned proposal set.

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

## Protocol-Blocked Recovery Chain (2026-06-06 campaign)

Campaign `p1-admissionfix-g35flash-p25flash-20260606-053302`, run
`run-1780751388442-structured-current-policy-c545d2b2`, protocol artifact
`1780751777945_tool_call_review_BurntSushi__ripgrep-2209.json`.

| Call | Tool | Outcome | Error / note |
|------|------|---------|--------------|
| 35 | `apply_code_edit` | applied | `applied:1, ok:true` on `crates/printer/src/util.rs` |
| 36 | `cargo test` | completed, tests failed | `status_reason: tests_failed_or_runtime` |
| 37 | `apply_code_edit` | failed | `Cannot stage apply_code_edit ... Content changed for ".../util.rs". Refresh or re-resolve the target before submitting another semantic edit.` |
| 38 | `code_item_lookup` | failed | `Internal compiler error: failed to read snippet: Content changed for ".../util.rs"` |

Protocol marked focal call `[36] cargo` as recoverability-blocked
(`no_clear_recovery`, UI: blocked) because the expected post-test repair path
(`apply_code_edit` / `code_item_lookup`) failed with stale-file/snippet errors
immediately after a successful same-file apply.

Instance trace:
`/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-053302/BurntSushi__ripgrep-2209/runs/run-1780751388442-structured-current-policy-c545d2b2/agent-turn-trace.json`

This is the same stale post-mutation contract as the 2026-05-24 repro, but
observed after an auto-applied semantic edit plus a legitimate failing test
verification step rather than only a staged-then-failed follow-up edit.

## Fix Direction

- Keep the new stale-anchor regression as fixed-contract coverage:
  `apply_code_edit_rejects_stale_semantic_anchor_before_staging`.
- Promote the manual historical replay probe above into automated replay
  coverage if this bug class recurs or the replay harness gets a stable
  artifact-comparison mode.
- Ensure post-apply rescan visibility or proposal-set planning is a hard
  boundary for same-file canonical edits in eval/headless tool loops.
- Carry touched-path/version state through the edit proposal lifecycle so
  stale same-file proposals can be composed, delayed, or rejected before apply.
