# Gemini 3.5 Flash Direct Eval Run Review

Date: 2026-05-24

Campaign: `p1-gemini35-flash-direct-profile-20260524-151353`

Instance: `BurntSushi__ripgrep-2209`

Run: `run-1779660936587-structured-current-policy-e89a791e`

## Verdict

This eval step is a real forward step. It ran after `14684163 Fix Google tool
call thought signature replay`, preserved all Google `thought_signature` fields
in the provider trace, recorded every provider-emitted tool call, produced an
applied two-file patch, and exported a non-empty Multi-SWE-Bench submission.

The patch is benchmark-plausible and touched the expected implementation file,
`crates/printer/src/util.rs`, plus a regression test in
`crates/printer/src/standard.rs`. The most important caveat is validation
quality: the meaningful checks were the `grep-printer` test runs, while the
final unqualified `cargo test` resolved to the unrelated focused
`crates/globset/Cargo.toml` manifest. The run has useful tool behavior, but it
also repeats known misleading success surfaces around staged patch results,
audit cargo `content_len = 0`, empty completed reads, and weak final validation.

## Evidence Roots

Repository:

```text
/home/brasides/code/ploke
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-profile-20260524-151353
```

Run root:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e
```

Record:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e/record.json.gz
```

Protocol artifact:

```text
/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e/1779661481557_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json
```

Patch/submission artifacts:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e/benchmark-patch-projection.json
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e/multi-swe-bench-submission.jsonl
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e/llm-full-responses.jsonl
```

## Setup And Closure State

The parent worktree was clean when checked, and its log shows this run was
created after the thought-signature fix:

```text
55aecc1b prototype1: initializing gen 0 parent node-06b31bdfd8547e36
14684163 Fix Google tool call thought signature replay
```

The campaign manifest selected `google/gemini-3.5-flash` through
`direct_google`. The eval budget was `max_turns = 40`, `max_tool_calls = 200`,
and `wall_clock_secs = 1800`.

`closure-state.json` shows:

- registry: `complete`, 1 expected, 1 mapped
- eval: `complete`, 1 complete, 0 failed, 0 missing
- protocol: `partial`
- `tool-call-intent-segments`: `complete`
- `tool-call-review`: `missing`
- `tool-call-segment-review`: `missing`

So the eval advanced into the baseline protocol path and produced the first
protocol artifact, but the protocol surface is not full.

## Patch And Submission

The record packaging phase and `benchmark-patch-projection.json` report:

- `submission_artifact_state`: non-empty, through the record packaging phase
- patch projection check: `passed`
- `msb_submission_path`: the run's `multi-swe-bench-submission.jsonl`
- submission `byte_len`: 3873
- submission `line_count`: 113
- submission `sha256`:
  `bcbfd1e756a15139e55e239c80a527da8c066869ac541ded0150d5115b89eca9`

The target checkout diff has exactly two modified files:

```text
crates/printer/src/standard.rs | 28 +++++++++++++++++++
crates/printer/src/util.rs     | 61 +++++++++++++++++++++++++++++-------------
```

The `agent-turn-summary.json` patch artifact reports two applied proposals:

- `crates/printer/src/util.rs`
- `crates/printer/src/standard.rs`

The expected file-change witness only required `crates/printer/src/util.rs`,
and it changed:

```text
before_sha256: 9e637c12ebe8a9afe08322d69de4aeab552ba3628505148deb1dc6bbbf59c62c
after_sha256:  843c68fa28a7356545e69472ae9ae608a7e3fab880e2f9edf047e585683b588b
```

The implementation patch replaces `replace_with_captures_at` in
`Replacer::replace_all` with an explicit `captures_at` loop. In multiline mode
it stops at `range.end` and copies trailing content only up to that logical end
point. The test patch adds `replacement_multi_line_look_around_leak`.

## Provider Trace And Tool Ledger

The trace audit command was:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown
```

It reported:

- provider responses: 62
- provider-emitted tool calls: 61
- recorded tool calls: 61
- missing recorded provider call IDs: 0
- extra recorded call IDs: 0
- finish reasons: 61 `tool_calls`, 1 `stop`
- final response content length: 2037
- final response usage: 117540 prompt tokens, 483 completion tokens, 118807 total tokens

Direct `llm-full-responses.jsonl` inspection confirmed all 61 provider tool
calls had IDs and all 61 carried `extra_content.google.thought_signature`. The
actual signature values are not copied here.

Recorded classifications from the trace audit:

- `completed`: 8
- `read_with_content`: 20
- `context_results`: 18
- `empty_completed_read`: 8
- `transport_failure`: 5
- `duplicate_request`: 2

The five failed tools were recoverable:

- four `code_item_lookup` failures for method/impl lookups
- one failed `insert_rust_item` for adding the regression test

The model recovered from the failed `insert_rust_item` by using
`non_semantic_patch` to add the test.

## Tool Use Quality

The tool sequence was semantically useful, but noisy.

Useful examples:

- The first useful localization started in `crates/printer/src/util.rs`, then
  searched `replace_with_captures_at` and read `crates/matcher/src/lib.rs`.
- `request_code_context` for `fn replacement_multi_line` returned the existing
  `standard::replacement_multi_line` test in `standard.rs`, which was relevant
  to the eventual regression test.
- After the failed `insert_rust_item`, the model searched `fn trim_ascii
  standard.rs`, found a nearby test anchor, and patched `standard.rs` directly.
- The final patch contents match the issue theme: multiline replacement should
  not process or copy the look-ahead region.

Noisy examples:

- Several `request_code_context` calls returned broadly related but low-value
  snippets, such as generic `Captures`, `RegexCaptures`, or `StandardMatcher`
  definitions, before the model narrowed to printer replacement tests.
- Eight completed `read_file` calls returned `ok:true`, `exists:true`, and
  `truncated:true` with empty `content`. Some requested valid line ranges. For
  example, `standard.rs` lines 900-1100 and 1400-1550 returned empty content
  even though the target file has thousands of lines and those ranges contain
  code.
- The trace had two duplicate cargo requests in the audit classification.

These did not prevent a patch, but they should be treated as information
quality failures rather than successful reads.

## Cargo Visibility And Verification

The model did see cargo output. The trace audit table reports `content_len = 0`
for cargo rows, but the recorded tool payloads and `MessageUpdated:tool` events
carry model-visible JSON and stdout/stderr tails.

Concrete cargo examples:

- Initial `cargo check` completed with `manifest_path` set to
  `crates/globset/Cargo.toml`. This was a weak setup check, not a check of the
  printer patch.
- `cargo test -p grep-printer` before adding the new test ran the printer crate
  test target and reported `94 passed`.
- `cargo test -p grep-printer` after adding the new test reported `95 passed`
  and included `standard::tests::replacement_multi_line_look_around_leak ... ok`.
- The final unqualified `cargo test` completed successfully but resolved to
  `crates/globset/Cargo.toml`, reporting `253 passed` plus doc tests for
  `globset`. That check is not meaningful validation for a patch in
  `crates/printer`.

The final assistant message claimed the `grep-printer` suite passed. That part
is supported by the focused printer test run. The implicit workspace-wide
confidence from the last `cargo test` is not supported because the manifest
resolution target was unrelated.

This is the same class of issue tracked in
`docs/active/bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`:
cargo output can be model-visible and repair-relevant while trace summaries
hide it, and final cargo checks can resolve to weak focused manifests.

## Protocol Review

The produced protocol artifact is useful but not sufficient as a semantic
success auditor.

The artifact reports:

- total calls: 61
- one turn with 61 tools
- failed tool count: 5
- patch proposed: true
- patch applied: false
- segments: 4
- labeled calls: 59
- uncovered calls: 2, indices 0 and 20

Segment labels:

- `locate_target`, calls 1-19
- `inspect_candidate`, calls 21-49
- `edit_attempt`, calls 50-51
- `edit_attempt`, calls 52-60

The protocol correctly recognized the broad shape of the run: locate target,
inspect candidate, patch `util.rs`, then add a test in `standard.rs`. Its blind
spots are important:

- It did not distinguish the meaningful `grep-printer` validation from the
  weak final `globset` validation.
- It inherited a misleading `patch_applied: false` signal even though the
  persisted patch artifact says both edit proposals were `Applied` and the
  checkout diff contains the patch. The likely cause is the tool-level
  `non_semantic_patch` result shape: `ok:true`, `staged:1`, `applied:0`.
- It did not flag empty successful reads as information failures.
- It has not run the required `tool-call-review` or `tool-call-segment-review`
  procedures yet.

## Misleading Success Signals

Do not read these signals as complete semantic success:

- `closure-state.json` says eval is complete, but that only means the eval
  artifact path completed and packaged a patch.
- `non_semantic_patch` returned `ok:true` while its immediate tool payload said
  `staged:1`, `applied:0`; the later patch artifact is the authority for
  actual applied proposals.
- `run_trace_audit.py` reports cargo rows with `content_len = 0`; the model did
  see cargo payloads in the record and message stream.
- The final `cargo test` success was against `globset`, not `grep-printer` or
  the workspace.
- The intent-segmentation protocol artifact is partial protocol coverage, not a
  correctness judgment.

I did not find `TOOL_EXECUTION_FAILED`, `ContentMismatch`, or `ploke_io` hash
mismatch text in the persisted run root, record, campaign directory, or
protocol artifact. If the late `TOOL_EXECUTION_FAILED` warning came from an
operator/stderr stream outside these persisted artifacts, it should be tied to
the existing replay-boundary/shutdown-noise note in
`docs/active/plans/self-improvement-loop/historical-replay-probe-workflow.md`
rather than filed as a duplicate. If stale snippet/hash mismatch logs appeared
outside the persisted artifacts, the existing active bug is
`docs/active/bugs/2026-05-24-request-code-context-silent-stale-snippet-skip.md`.

## Adjudication Fields To Promote

Future LLM adjudication should explicitly score these fields:

1. Provider tool-call preservation:
   count provider tool calls, recorded calls, missing call IDs, and provider
   metadata preservation such as Google thought signatures, without exposing
   signature values.

2. Patch materiality:
   whether a non-empty submission exists, whether the expected files changed,
   and whether the final checkout diff is limited to plausible benchmark files.

3. Tool-output learning chain:
   capture `tool output -> model interpretation -> follow-up action -> later
   result`. In this run, the clearest chain is failed `insert_rust_item` ->
   search for a test anchor -> `non_semantic_patch` of `standard.rs` -> passing
   `grep-printer` test with the new regression test name.

4. Cargo validation strength:
   record the requested cargo command, resolved manifest path, changed files,
   and whether the command is target-relevant. A final successful cargo command
   against an unrelated focused manifest should be weak validation.

5. Cargo visibility:
   separate audit summary `content_len` from model-visible tool message content.
   The adjudicator should inspect the actual tool payload or message event, not
   only the audit table.

6. Empty successful reads:
   treat `read_file ok:true` with empty content for valid line ranges as an
   information failure, not a successful evidence read.

7. Staged versus applied edit semantics:
   distinguish tool-level staging results from the later patch artifact that
   records applied proposals and changed files.

8. Protocol coverage:
   distinguish intent segmentation from full tool-call and segment review. A
   partial protocol state should not be scored as semantic validation.

## Action Items

- Do not advance the loop based on this review alone; this report is only the
  eval-step review requested here.
- Treat the thought-signature replay blocker as cleared for this run's eval
  ledger: 61 provider tool calls, 61 recorded tool calls, zero missing call
  IDs, and 61 preserved Google thought signatures.
- Keep the cargo/audit ambiguity tied to
  `2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`.
- Keep stale snippet/hash-mismatch concerns tied to
  `2026-05-24-request-code-context-silent-stale-snippet-skip.md` unless a fresh
  persisted artifact shows a distinct failure mode.
- Promote the adjudication fields above before treating future protocol output
  as enough to explain semantic progress.
