# Broad-harness child-attempt run review: node-26f01da56959fd47-r2

Verdict: `node-26f01da56959fd47-r2` is mechanically inspectable and did apply one workspace edit, but it is not yet a usable descendant/benchmark result. The headless TUI artifact records a staged-then-applied patch to `proc_macros/syn_parser/syn_parser_macros/src/lib.rs`, followed by mixed validation and a 900s harness timeout. The request-declared `submitted_result_path` JSON is absent, so there is no broad-harness submitted result to admit, score, or compare as a successor.

Review scope: this is a read-only review of the completed broad-harness retry/child self-eval slot `node-26f01da56959fd47-r2`. I did not advance the campaign, run Prototype 1 step/observe commands, or edit source code. I wrote only this report.

## Evidence roots

- Project root: `/home/brasides/code/ploke`
- Campaign id: `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Active worktree/repo root: `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Parent node: `node-26f01da56959fd47`
- Slot reviewed: `node-26f01da56959fd47-r2`
- Task: `BurntSushi__ripgrep-2209`
- Model route: `google/gemini-3.5-flash` via direct Google (`campaign.json`, `run-profile.toml`, and `agent-turn-trace.json` all identify this route/model)
- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r2.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r2.md`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness/node-26f01da56959fd47-r2`
- Headless result JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r2.headless-tui.json`
- Turn-live trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r2.turn-live/agent-turn-trace.json` and `agent-turn-summary.json`
- Turn-live raw provider response log: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r2.turn-live/llm-full-responses.jsonl` exists but has size 0; do not use it as provider-call evidence.

## Exact execution path

This slot is a Prototype 1 broad-harness headless-TUI child attempt, not the baseline Multi-SWE-bench eval turn itself:

```text
prototype1-state parent node
  -> publish broad-harness request slot node-26f01da56959fd47-r2
  -> materialize candidate workspace for that slot at target commit 8f478c66e30224adf234439c8c6395bf521a6373
  -> tui_adapter headless turn for google/gemini-3.5-flash
  -> record tool events, staged proposal, applied proposal, cargo validations
  -> headless attempt terminal record reports timed_out at 900s
```

Evidence for this path:

- The request has `request_id: broad-harness-request:node-26f01da56959fd47:r2`, `parent_node_id: node-26f01da56959fd47`, target artifact commit `8f478c66e30224adf234439c8c6395bf521a6373`, and the r2 candidate workspace path.
- The prompt tells the model to modify the candidate checkout to improve `Prototype 1 descendant performance` while staying outside protected core.
- `prompt_diagnostics` in the headless result says the workspace was loaded at the r2 candidate root, with focused root `proc_macros/syn_parser/syn_parser_macros`, BM25 ready with 6885 docs, and context mode `Off`.
- The headless result contains 139 event entries: 68 `tool_request`, 67 `tool_completed`, 2 `tool_failed`, 1 `proposal`, and 1 `turn`.
- The workspace git state proves one uncommitted applied edit against the target commit: `git status --short` reports `M proc_macros/syn_parser/syn_parser_macros/src/lib.rs`; `git diff --stat` reports 1 file changed, 20 insertions, 78 deletions.

## Closure state

Campaign-level closure is mechanically complete for the baseline eval, but not for this broad-harness slot as a descendant result.

- `closure-state.json` reports registry complete: expected 1, mapped 1, missing 0.
- Eval is complete: expected 1, complete 1, failed 0, missing 0.
- Protocol is missing: expected 1, full 0, missing 1. All required procedures are missing: `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review`.
- The parent node record still has stale/live-state tension: `node.json` says `status: running`, while this r2 attempt has a completed headless artifact. This review only classifies the r2 slot artifact, not the whole parent node lifecycle.

Record labels for the slot:

- broad-harness request JSON/MD: present.
- candidate workspace: present, manual join needed to inspect git state.
- headless TUI result and turn-live trace/summary: present, manual join needed.
- `submitted_result_path` JSON: record absent. The request declares `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r2.json`, but that file is absent; the result directory only had `node-26f01da56959fd47-r2.headless-tui.json` for this slot.
- raw provider responses: file present but empty (`llm-full-responses.jsonl` size 0), so provider-call replay is unavailable for this child attempt.
- protocol artifacts for this node: absent.

## Eval and patch output

Patch output exists only as an applied workspace diff and headless proposal/apply evidence, not as a submitted descendant result.

- `patch_artifact` in the turn-live JSON reports `edit_proposals: []`, `create_proposals: []`, `applied: true`, `all_proposals_applied: true`, `expected_file_changes: []`, `any_expected_file_changed: false`, and `all_expected_files_changed: false`. This is internally weak as a patch summary because the proposal lists are empty even though raw events show one proposal and one applied file.
- Headless `attempts` records three attempt outcomes within turn 1: a failed read of parent `runner-result.json`, a failed `code_item_lookup`, and an applied proposal `d4c1a94b-da89-5c86-bae9-d86a7334f67d` touching `proc_macros/syn_parser/syn_parser_macros/src/lib.rs`.
- Git verification in the candidate workspace shows HEAD `8f478c66e30224adf234439c8c6395bf521a6373` and a single modified file.
- The diff rewrites `generate_node_info_derive` to build token iterators (`other_fields`, `other_field_vis`, `other_field_name`, `other_field_type`) and emit one combined `quote!` block, deleting the previous `Vec<quote!>` accumulation for generated fields/constructor fields. It leaves `Type` imported from `syn` even though the rewritten file no longer uses it, which matches the post-edit warning count.

Validation recorded by the headless result:

- Before the edit, focused `cargo check` and `cargo test` for `proc_macros/syn_parser/syn_parser_macros/Cargo.toml` both passed with 0 warnings.
- After the edit, focused `cargo check` passed with 1 warning and focused `cargo test` passed with 2 warnings.
- `cargo check -p syn_parser` at workspace scope passed with 1 warning.
- `cargo test -p syn_parser` failed with exit code 101 after 18.823s. The failing test was `tests::parse_workspace_config_handles_non_standard_root_without_duplicate_crate_roots`; stdout reported a `Discovery(ManifestRead)` failure for missing `tests/fixture_github_clones/corpus/serde/Cargo.toml`.
- I verified that path in the candidate workspace: `tests/fixture_github_clones`, `tests/fixture_github_clones/corpus`, and `tests/fixture_github_clones/corpus/serde/Cargo.toml` are absent. That makes the broad `cargo test -p syn_parser` failure at least partly fixture/environment-dependent, but it still means the child did not finish with a clean full package validation.

## Oracle/MBE state

No child oracle/MBE result exists for this r2 broad-harness attempt.

- The campaign run profile has `[selection.oracle] mode = "record-only"` and `[execution.mbe] enabled = false`.
- The r2 slot did not produce a submitted broad-harness result JSON, child admission, successor node, benchmark patch projection, Multi-SWE-bench submission, or oracle report.
- Therefore the edit may be a plausible local performance refactor, but there is no benchmark-usefulness evidence for descendants.

## LLM and tool behavior

- Selected model in the trace: `google/gemini-3.5-flash`.
- `final_assistant_message` is null in the turn-live trace.
- `terminal_record.outcome` is `completed`, with summary text beginning `[success] code=TOOL_EXECUTION_FAILED kind=tool_execution ...` and attempts `69`.
- The headless result separately reports `terminal: timed_out`, `secs: 900`. The durable conclusion should prefer the more specific combined state: the turn produced tool/proposal/validation records and a `TurnFinished` event, but the overall headless attempt timed out before a clean final result/submission.
- Tool failures were not all terminal:
  - Failed read: the model tried to read parent `runner-result.json`, which was absent.
  - Failed exact lookup: `code_item_lookup` claimed no function named `generate_node_info_derive` existed in `proc_macros/syn_parser/syn_parser_macros/src/lib.rs` with `module_path: crate`, `node_kind: function`.
- The model recovered from the exact lookup failure by reading the target file region directly and then issuing a `non_semantic_patch`.
- The model did not recover from the final broad validation failure before timeout. After `cargo test -p syn_parser` failed, it read `proc_macros/syn_parser/ploke-test-macros/src/lib.rs` in two chunks; no second patch or successful validation followed.

## Positive examples and adjudication candidates

Positive trace signal:

- Failed exact symbol lookup did not stop the attempt. The next concrete action was a direct `read_file` on `proc_macros/syn_parser/syn_parser_macros/src/lib.rs` lines 40-170, which exposed the function body, followed by a targeted patch to that same function. This is a useful `tool failure -> alternate context retrieval -> targeted edit` chain.

Adjudication candidates:

- Distinguish focused validation success from package validation success. The edit passed focused macro-crate checks but did not pass `cargo test -p syn_parser`.
- Treat empty raw provider logs as an observability failure for provider-led adjudication; only framework event traces are available for this child.
- Score code-item lookup failures semantically. This lookup failed on an item that the file visibly contained, so a future adjudicator should classify it as tool/index mismatch rather than model hallucination.
- Mark validation failures caused by absent external fixture checkouts separately from compile/test regressions, but do not promote the attempt unless the harness records a clean accepted validation policy.

## Trace reconstruction

Concrete chain from model/tool event to edit, validation, and timeout:

```text
prompt_diagnostics: workspace loaded at r2 candidate root; focused root is syn_parser_macros; context mode Off
  -> model explores focused crate with list_dir and initial cargo check/test
  -> focused cargo check/test pass before any edit
  -> model reads syn_parser_macros/src/lib.rs and requests related context
  -> model later calls code_item_lookup for generate_node_info_derive
  -> code_item_lookup fails, saying no such function exists
  -> model reads syn_parser_macros/src/lib.rs lines 40-170 instead
  -> read_file shows #[proc_macro_derive(GenerateNodeInfo)] pub fn generate_node_info_derive(...)
  -> model emits non_semantic_patch for that function
  -> tool_completed reports staged=1, applied=0
  -> proposal event d4c1a94b-da89-5c86-bae9-d86a7334f67d is recorded for lib.rs
  -> later tool_completed reports applied=1, partial=false, with a new file hash
  -> focused cargo check/test pass after edit, with warnings
  -> cargo check -p syn_parser passes
  -> cargo test -p syn_parser fails on missing tests/fixture_github_clones/corpus/serde/Cargo.toml
  -> model reads ploke-test-macros source twice but makes no further edit
  -> headless result terminal is timed_out at 900s; submitted_result JSON is absent
```

Suspicious-result verification:

- The failed `code_item_lookup` is suspicious because the item exists. I verified the candidate file directly: line 39 has `#[proc_macro_derive(GenerateNodeInfo)]` and line 40 has `pub fn generate_node_info_derive(input: TokenStream) -> TokenStream {`.
- The failed `cargo test -p syn_parser` references a missing serde fixture checkout. I verified the exact path is absent in the candidate workspace. That supports classifying the broad package test failure as unresolved/fixture-dependent rather than a clear compile regression from the macro edit.
- The applied patch is real: git diff in the candidate workspace shows one modified file and no submitted result JSON exists to package that diff as an admitted child.

Last point where the model had enough information to act: after the `cargo test -p syn_parser` failure, the model had a concrete failing test name and missing manifest path. It could have either limited validation claims to focused checks, investigated fixture provisioning, or repaired/avoided the failing fixture path. Instead it inspected a different proc-macro crate and timed out without another edit or final submission.

## Protocol review and protocol blind spots

Protocol did not judge this r2 slot.

- Campaign closure reports protocol missing for the baseline eval procedures.
- No node-scoped protocol artifacts were found under `prototype1/nodes/node-26f01da56959fd47` for this broad-harness attempt.
- The raw provider response ledger for the child is empty, so any future protocol/read-side reconstruction has to use framework event traces, not provider-emitted call IDs and finish reasons.

Blind spots exposed by this slot:

- Mechanical `TurnFinished outcome: completed` can coexist with headless `terminal: timed_out`, `TOOL_EXECUTION_FAILED` summary text, applied workspace edits, and absent submitted result JSON.
- `patch_artifact` says applied/all applied but has empty proposal lists and empty expected-file-change fields; reviewers must inspect raw events and git diff.
- Validation summaries capture exit code and tails, but the child did not turn the failure into a clear final status such as `accepted_with_fixture_gap`, `rejected_validation_failed`, or `timed_out_after_apply`.

## What is working

- Slot-scoped request and workspace paths are durable and independently inspectable.
- Prompt diagnostics prove the model saw a loaded candidate workspace, focused root, context mode, and BM25 state.
- The raw headless event stream preserves enough lifecycle detail to reconstruct staged versus applied edit status.
- Focused and workspace cargo validation calls are captured with command, manifest path, exit code, warning/error counts, and output tails.
- The candidate workspace git state provides an external check on whether a patch actually touched disk.

## What is not working yet

- No `submitted_result_path` JSON was written for r2, despite an applied patch in the workspace.
- The broad-harness terminal state is ambiguous without manual interpretation: completed turn event, `TOOL_EXECUTION_FAILED` summary, and 900s timeout all coexist.
- Provider-call evidence for the child is missing because `llm-full-responses.jsonl` is empty.
- The exact lookup tool failed on a function that exists in the target file.
- Full package test validation failed and was not resolved before timeout.
- There is no child admission, successor node, benchmark run, oracle/MBE output, or protocol judgment for this r2 edit.

## Action items

1. Persist a submitted/final slot result even for timeout-after-apply cases. Observed gap: r2 has a request-declared `submitted_result_path`, applied proposal evidence, and a dirty workspace, but no result JSON. The final record should explicitly say whether the applied diff is admissible, rejected, or quarantined.

2. Add a single authoritative broad-harness terminal classification. Observed gap: `TurnFinished outcome=completed`, summary `TOOL_EXECUTION_FAILED`, and headless `terminal=timed_out` require manual reconciliation. A normalized status such as `timed_out_after_apply_with_failed_validation` would prevent over-crediting mechanical completion.

3. Preserve raw provider responses for broad-harness child attempts. Observed gap: `llm-full-responses.jsonl` exists but is size 0, so provider-emitted call IDs, finish reasons, and model text cannot be audited for this child.

4. Fix or annotate `code_item_lookup` for proc-macro functions. Observed gap: lookup reported no `generate_node_info_derive`, while direct file verification shows the function at lines 39-40. This should be classified as a tool/index lookup miss and either repaired or surfaced as a recoverable exact-lookup limitation.

5. Preflight external fixture-dependent package tests or label them separately. Observed gap: `cargo test -p syn_parser` failed because `tests/fixture_github_clones/corpus/serde/Cargo.toml` was absent. The harness should distinguish missing fixture setup from edit-induced test regressions, while still preventing a dirty attempt from being promoted as fully validated.

6. Keep r2 out of descendant benchmark scoring until admission artifacts exist. Observed gap: there is no submitted result, child node, benchmark patch projection, Multi-SWE-bench submission, oracle/MBE report, or protocol review for this slot.
