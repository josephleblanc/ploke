# Prototype 1 Eval Review: `p1-gemini35-flash-direct-fresh-20260524-163447`

## Verdict

The eval step completed mechanically and produced a non-empty benchmark patch, but the patch is probably benchmark-useless as submitted. The model added an unused helper and a weak passing test, but the intended behavioral edit to `Replacer::replace_all` failed to apply after a same-file content-change rejection. The trace then exposed a more serious harness issue: the recorded `ToolCompleted` event captured that failure, but the model-facing `MessageUpdated` tool messages for the same call only reported the earlier staged-success payload.

This is not an empty-patch failure. It is a false-positive patch/export failure: closure marks eval complete, the submission contains a diff, and the target checkout is modified, but the intended fix is absent from the final diff.

## Evidence Roots

- Campaign: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260524-163447`
- Run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063`
- Record: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063/record.json.gz`
- Target checkout: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- Trace audit command:

```bash
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063 \
  --markdown
```

## Closure And Protocol State

`closure status` reports one expected eval and one completed eval:

```json
"eval": {
  "expected_total": 1,
  "complete_total": 1,
  "failed_total": 0,
  "status": "complete"
}
```

Closure still reports protocol as missing for the three required procedures:

```json
"protocol": {
  "expected_total": 1,
  "full_total": 0,
  "missing_total": 1,
  "status": "missing"
}
```

Record-local `protocol status --record ... --format json` is more nuanced:

```json
{
  "tool_calls_total": 95,
  "protocol_eligible": true,
  "segmentation_present": true,
  "call_review_count": 0,
  "segment_review_count": 0,
  "next_step": { "kind": "tool_call_review", "index": 0 }
}
```

So segmentation exists, but follow-up call review and segment review have not run.

## Eval And Patch Output

`benchmark-patch-projection.json` says the patch export passed:

```json
"submission": {
  "byte_len": 2738,
  "line_count": 97
},
"check": {
  "status": "passed",
  "detail": "fix_patch exported from the recorded checkout cwd"
}
```

The target checkout has two modified files:

```text
 M crates/printer/src/standard.rs
 M crates/printer/src/util.rs
```

`git diff --stat` in the target checkout reports:

```text
crates/printer/src/standard.rs | 23 +++++++++++++++++
crates/printer/src/util.rs     | 57 ++++++++++++++++++++++++++++++++++++++++++
2 files changed, 80 insertions(+)
```

The final submitted patch adds:

- `replacement_multi_line_issue_2208` in `crates/printer/src/standard.rs`
- `replace_with_captures_at_limited` in `crates/printer/src/util.rs`

But the helper is unused. A direct search of the target checkout found only the existing call to `replace_with_captures_at` and the new helper definition:

```text
86:                .replace_with_captures_at(
462:fn replace_with_captures_at_limited<M, F>(
```

The existing `Replacer::replace_all` implementation still calls `matcher.replace_with_captures_at(...)`; it does not call the new limited helper. That means the intended behavioral fix did not land.

## Tool Trace Summary

The trace audit found ledger parity between provider-emitted and recorded tool calls:

```text
responses: 96
provider-emitted tool calls: 95
recorded tool calls: 95
missing recorded provider call ids: 0
finish reasons: {'tool_calls': 95, 'stop': 1}
recorded classifications:
  completed: 12
  context_results: 15
  read_with_content: 28
  duplicate_request: 11
  empty_completed_read: 27
  transport_failure: 2
```

This is mechanically healthy as a ledger, but not semantically healthy as a patch review.

## Cargo Visibility And Validation

Cargo output was model-visible for cargo tool calls. For example, the first failing `grep-printer` test call had a `MessageUpdated` tool payload with diagnostics:

```json
{
  "ok": false,
  "status_reason": "compile_failed",
  "command": "test",
  "scope": "workspace",
  "manifest_path": "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml",
  "summary": { "errors": 3, "warnings": 1 },
  "diagnostics": [
    "failed to resolve: use of undeclared type `RegexMatcher`",
    "failed to resolve: use of undeclared type `SearcherBuilder`",
    "cannot find function `printer_contents` in this scope"
  ]
}
```

The model used that output productively: immediately after the compile failure it started inspecting the end of `standard.rs` and later repaired the misplaced test.

The focused cargo checks were misleading. Several `cargo check` calls resolved to the unrelated `globset` manifest:

```text
command=check
scope=focused
manifest_path=/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml
```

The final assistant message claimed it would run the entire workspace test suite, but the final `cargo test` also resolved to focused `globset`:

```json
{
  "ok": true,
  "command": "test",
  "scope": "focused",
  "manifest_path": "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml"
}
```

There was a meaningful package-level validation before that:

```text
test standard::tests::replacement_multi_line_issue_2208 ... ok
test result: ok. 95 passed; 0 failed; 0 ignored
```

But this test did not catch the missing behavioral wiring because the final patch never changed `replace_all`.

## Same-File And Tool-Visibility Failures

The most important failure happened on the intended util edit.

The model first inserted the helper into `util.rs` successfully:

```json
{"applied":1,"ok":true,"results":[{"file_path":".../crates/printer/src/util.rs"}]}
```

It then requested an `apply_code_edit` to update `crate::util::Replacer::replace_all` to call the helper. The recorded terminal tool event says the apply failed:

```json
{
  "applied": 0,
  "ok": false,
  "results": [
    {
      "error": "Content changed for \"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs\"",
      "file_path": "/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs"
    }
  ]
}
```

However, for that same call id, the only model-facing `MessageUpdated` tool messages were:

```json
{"ok":true,"staged":1,"applied":0,"files":["crates/printer/src/util.rs"],"preview_mode":"codeblock","auto_confirmed":true}
```

There was no model-facing tool message for the final apply failure. The model continued as if the behavioral edit had landed. This directly affected the final output.

A later `non_semantic_patch` failure on `standard.rs` was different. It was model-visible:

```json
{
  "ok": false,
  "tool": "non_semantic_patch",
  "message": "failed to patch ... patch matched only fuzzily after an earlier settled proposal touched the same file; refresh the file and submit a diff against the current content"
}
```

The model recovered from that visible failure by rereading nearby ranges and applying a smaller cleanup patch. That later failure caused churn, but it did not prevent final patch export.

### Post-Fix Historical Replay Probe

After fixing semantic staging to reject unverifiable file versions before
creating a pending proposal, I replayed the historical provider prefix through
the same breakpoint against a throwaway ripgrep worktree at the recorded base
SHA:

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

The replay installed the historical prefix through response index 45 and
re-executed tool behavior against current code. The first same-file
`insert_rust_item` still applied to `crates/printer/src/util.rs`. The follow-up
`apply_code_edit` for `crate::util::Replacer::replace_all` did not stage a
stale proposal. It emitted a model-facing tool error before staging:

```text
Cannot stage apply_code_edit for .../crates/printer/src/util.rs because the file version could not be verified: Content changed for ".../crates/printer/src/util.rs". Refresh or re-resolve the target before submitting another semantic edit.
```

That replay is not a new benchmark success: it intentionally stops at the old
failure breakpoint and leaves only the helper insertion dirty in the throwaway
worktree. It is useful evidence that this specific stale-anchor path now fails
at admission instead of producing a staged-success message followed by a hidden
apply failure.

## Empty Successful Reads

The trace had 27 `empty_completed_read` calls. Several were not harmless EOF reads. Direct checkout verification shows requested ranges existed.

Example: `read_file` returned `ok:true`, `truncated:true`, and empty `content` for `crates/matcher/src/lib.rs` lines `910..936`. The target file has 1322 lines, and direct `sed -n '910,936p'` returns the `replace_with_captures_at` signature and body.

Example: `read_file` returned empty content for `crates/printer/src/standard.rs` around `3600..3650`. The target file has 3701 lines, and direct `sed -n '3600,3650p'` returns real test code near `regression_after_context_with_match`.

The model adapted by trying nearby ranges and larger `max_bytes`, but the empty successful reads made the recovery phase much longer and noisier.

## Positive Examples

- Initial RAG/context retrieval was useful. The first `request_code_context` for `find_iter_at_in_context` returned the exact printer utility and related matcher context, and the model used it to localize the issue.
- Cargo diagnostics were visible and materially used. The failed `grep-printer` package test surfaced missing imports/out-of-module placement, and the model pivoted to inspect and repair `standard.rs`.
- The model recovered from the visible fuzzy same-file patch rejection. After `non_semantic_patch` failed on `standard.rs`, it reread the current file around the affected area and applied a smaller successful patch.
- The run produced an exportable patch and preserved provider/recorded tool-call parity.

## Adjudication Candidates

These should be considered for future LLM-adjudication fields or reviewer prompts:

- Did the final patch actually wire newly added helper code into the behavior under test?
- Did any final `ToolCompleted` failure lack a corresponding model-facing tool message?
- Did the final validation target the files or package actually changed, or did it resolve to an unrelated focused manifest?
- Did a passing test exercise the changed behavior, or merely compile/pass alongside an unused helper?
- How many `ok:true` reads returned empty content for line ranges that exist in the target checkout?
- Did the model recover from a visible tool failure with a targeted alternate action?

## Action Items

Blocker before trusting this loop result as a semantic benchmark success:

1. Fix or file the tool-message lifecycle bug where a final apply failure can be recorded in `ToolCompleted` but hidden from the model-facing `MessageUpdated` tool message.
2. Add a validation/adjudication check for unused helper additions and missing behavioral wiring.

Non-blocking but high-value:

1. Track cargo `scope` and `manifest_path` in adjudication. A final validation that resolves to `crates/globset/Cargo.toml` should not count as whole-workspace validation.
2. Track `ok:true` empty reads on existing line ranges as semantic tool failures.
3. Promote cargo-error recovery into positive-example adjudication: this run contains a compact chain of visible diagnostics, targeted inspection, and successful test repair.
