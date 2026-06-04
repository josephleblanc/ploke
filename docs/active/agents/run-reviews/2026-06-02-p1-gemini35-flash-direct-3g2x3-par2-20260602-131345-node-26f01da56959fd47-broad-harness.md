# Prototype 1 broad-harness run review: parent slot `node-26f01da56959fd47`

Status: durable run review for the completed broad-harness parent slot. This review did not advance or mutate the campaign and did not edit source code. Evidence is from persisted artifacts plus read-only workspace/git checks.

## 1. Short verdict

Mechanical broad-harness attempt completion: yes, as a diagnostic bundle. The parent slot produced request JSON/MD, a materialized candidate workspace, a headless-TUI diagnostics JSON, a turn-live trace/summary, and recorded a terminal outcome.

Submitted/admitted broad-harness result: no. The request's `submitted_result_path` points to `prototype1/messages/edit-harness-result/node-26f01da56959fd47.json`, and that JSON is absent. The headless terminal outcome is `timed_out` after 900 seconds, and the source path explicitly refuses to publish a submitted broad-harness result on timeout.

Benchmark/descendant usefulness: not established. The candidate workspace is dirty with three changed files and includes several applied edits, but it timed out before a submitted result/admission artifact and before validation after the final `compilation_unit.rs` edit. Treat this as a timed-out, unsubmitted candidate workspace, not as an admitted child or benchmark-useful descendant.

High-signal caveats:

- `llm-full-responses.jsonl` for this turn-live bundle exists but is size 0, so provider-emitted tool calls cannot be audited from the raw provider ledger.
- The bundled `run_trace_audit.py` is not directly usable on this broad-harness turn-live directory because there is no `record.json`; it fails looking for `turn-live/record.json`.
- The headless diagnostics `attempts` array says the final `compilation_unit.rs` proposal was applied, and the workspace diff confirms the file changed, but the ordered `events` list ends at staged/proposal for that call with no final `applied=1` tool-completed event. This is a trace/projection mismatch reviewers should not smooth over.
- The request contract suggested `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`; the actual recorded validations were focused `ploke-core` cargo commands. That is useful for the changed crate but weaker than the requested surface checks.

## 2. Evidence roots

Campaign and Prototype 1:

- Project root for review docs: `/home/brasides/code/ploke`
- Campaign id: `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Prototype root: campaign root, `prototype1`
- Parent node: `node-26f01da56959fd47`
- Task: `BurntSushi__ripgrep-2209`
- Model route: `google/gemini-3.5-flash` via direct Google
- Run profile: `prototype1/run-profile.toml`
- Campaign manifest: `campaign.json`
- Closure state: `closure-state.json`
- Node record: `prototype1/nodes/node-26f01da56959fd47/node.json`
- Runner request: `prototype1/nodes/node-26f01da56959fd47/runner-request.json`

Reviewed broad-harness parent slot:

- Request JSON: `prototype1/messages/edit-harness-request/node-26f01da56959fd47.json`
- Request prompt: `prototype1/messages/edit-harness-request/node-26f01da56959fd47.md`
- Candidate workspace: `prototype1/workspaces/edit-harness/node-26f01da56959fd47`
- Headless diagnostics: `prototype1/messages/edit-harness-result/node-26f01da56959fd47.headless-tui.json`
- Turn-live dir: `prototype1/messages/edit-harness-result/node-26f01da56959fd47.turn-live`
- Turn-live trace: turn-live dir, `agent-turn-trace.json`
- Turn-live summary: turn-live dir, `agent-turn-summary.json`
- Turn-live provider sidecar: turn-live dir, `llm-full-responses.jsonl` (present, 0 bytes)
- Submitted result path named by the request: `prototype1/messages/edit-harness-result/node-26f01da56959fd47.json` (absent)

Candidate workspace verification:

- Candidate branch: `prototype1-broad-broad-harness-request-node-26f01da56959fd47`
- Candidate HEAD: `8f478c66e30224adf234439c8c6395bf521a6373`
- Read-only `git status --short`: three modified files:
  - `crates/ploke-core/src/compilation_unit.rs`
  - `crates/ploke-core/src/lib.rs`
  - `crates/ploke-core/src/workspace.rs`
- Read-only `git diff --stat`: 3 files changed, 35 insertions, 5 deletions.

## 3. Closure state

Campaign closure is mechanically complete for the baseline eval row, not for this broad-harness parent-slot submission:

- Registry: `complete` (`expected_total=1`, `mapped_total=1`, `missing_total=0`).
- Eval: `complete` (`expected_total=1`, `complete_total=1`, `failed_total=0`, `missing_total=0`).
- Protocol: `missing` (`expected_total=1`, `full_total=0`, `missing_total=1`). All required procedures are missing: `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review`.
- Node record still says `status: running` and names a `runner-result.json` path, but only `node.json` and `runner-request.json` exist in the node directory.

For this report, closure state is background context only. The reviewed broad-harness slot has its own terminal evidence: `headless-tui.json` says `timed_out`, and submitted result JSON is absent.

## 4. Exact execution path

The active broad-harness path is the Prototype 1 broad headless-TUI attempt path, not the baseline eval turn and not a completed child self-eval:

```text
loop prototype1-state
  -> CandidateGenerationConfig::BroadHarnessRequest
  -> publish_broad_harness_child_plan_request / publish_broad_edit_harness_request
  -> run_broad_headless_tui_attempt_with_options(slot)
       -> GitWorktreeBackend.prepare_broad_harness_workspace
       -> read published prompt
       -> tui_adapter::run_headless_with_model_capture_responses
       -> write_broad_headless_tui_diagnostics
       -> write_broad_headless_tui_turn_live_bundle
       -> finish_broad_headless_tui_attempt
            -> on Applied: write submitted_result_path JSON
            -> on TimedOut: error/refuse submitted result
```

Evidence for this path:

- `runner-request.json` records `loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`.
- `run-profile.toml` sets `[generation] source = "broad-harness-request"`, model `google/gemini-3.5-flash`, route source `direct-google`, and child fanout limits.
- The reviewed request has `request_id: broad-harness-request:node-26f01da56959fd47`, candidate workspace path under `prototype1/workspaces/edit-harness/node-26f01da56959fd47`, and `submitted_result_path` under `prototype1/messages/edit-harness-result/node-26f01da56959fd47.json`.
- Source path checked in `/home/brasides/code/ploke`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` lines 1532-1620 prepare the workspace, read the prompt, call `tui_adapter::run_headless_with_model_capture_responses`, write diagnostics/turn-live bundle, then call `finish_broad_headless_tui_attempt`.
- The same source file lines 1685-1748 writes submitted JSON only for `HeadlessTerminal::Applied`; `HeadlessTerminal::TimedOut` returns an error saying it is refusing to publish a submitted broad-harness result.
- The observed terminal in `node-26f01da56959fd47.headless-tui.json` is `{ "terminal": "timed_out", "secs": 900 }`, matching the source-side no-submission branch.

## 5. Eval and patch output

This slot did not produce an admitted benchmark patch or Multi-SWE-bench submission. It produced an unsubmitted candidate workspace diff.

Request contract and prompt:

- Prompt tells the model to modify the candidate checkout to improve `Prototype 1 descendant performance`, stay outside protected core, and use listed evidence roots outside the workspace.
- Contract validation commands requested:
  - `cargo check -p ploke-eval`
  - `cargo test -p ploke-eval edit_surface`
- Attempt timeout budget in the request: `turn_seconds=900`, `tool_seconds=180`.

Actual candidate diff, verified against the workspace:

- `crates/ploke-core/src/workspace.rs`: adds a global `CANONICALIZE_CACHE` behind `lazy_static`, adds `canonicalize_cached`, and changes `canonicalize_best_effort` to use the cache.
- `crates/ploke-core/src/lib.rs`: re-exports `canonicalize_cached`, imports it inside `mod ids`, and changes two `generate_resolved` paths from direct `.canonicalize()` calls to `canonicalize_cached(...)`.
- `crates/ploke-core/src/compilation_unit.rs`: changes `CompilationUnitKey::normalized_features` from `normalize_features(self.features.iter().cloned())` to `self.features.clone()`.

Recorded validations:

- The diagnostics JSON has six cargo validation records, all focused on `crates/ploke-core/Cargo.toml`, not `ploke-eval`:
  - initial `cargo check`: ok
  - initial `cargo test`: ok
  - after `workspace.rs`: `cargo check`: ok
  - after first `lib.rs` patch: `cargo check`: failed with two `E0425` errors for missing `canonicalize_cached` in scope
  - after import repair: `cargo check`: ok
  - after import repair: `cargo test`: ok, with 13 unit tests and doc tests in the captured tail
- No recorded validation appears after the final `compilation_unit.rs` change. The terminal timeout arrived after that proposal was staged/applied according to attempt projection, so the last workspace state is not validated by the recorded cargo commands.

## 6. Oracle/MBE state

There is no oracle/MBE result for this broad-harness parent-slot attempt.

- Run profile has `[execution.mbe] enabled = false` and selection oracle `mode = "record-only"`.
- The broad-harness result JSON that would package a submitted candidate is absent.
- No child admission, child eval, successor-ready, successor-completion, or benchmark patch projection exists for this slot.

Conclusion: the slot contains a candidate hypothesis, not benchmark evidence.

## 7. LLM and tool behavior

Provider ledger:

- `llm-full-responses.jsonl` exists but is 0 bytes. Do not infer provider response counts, raw finish reasons, or provider-emitted call IDs from this file.
- `agent-turn-trace.json` and `agent-turn-summary.json` are present and have matching top-level turn data, but they are framework traces, not raw provider responses.

Headless diagnostics counts:

- Attempts recorded: 6.
- Terminal: timed out after 900 seconds.
- Events recorded in `headless-tui.json`: 118.
- Event kinds: 55 `tool_request`, 57 `tool_completed`, 4 `proposal`, 2 `tool_failed`.
- Tool requests: 30 `read_file`, 7 `list_dir`, 6 `cargo`, 5 `request_code_context`, 3 `code_item_lookup`, 3 `non_semantic_patch`, 1 `apply_code_edit`.
- Prompt diagnostics: workspace loaded, focused root resolved to `crates/ploke-core`, BM25 ready with 6885 docs, context mode off, included RAG parts 0.

Tool behavior summary:

- The model did some useful localization around `ploke-core` canonicalization and compilation-unit feature normalization.
- It recovered from a compile failure caused by using `canonicalize_cached` inside a nested module without importing it.
- It also hit a real tool failure: `code_item_lookup` on an import failed with internal DB error `stored relation 'import' does not have field 'tracking_hash'`.
- A second `code_item_lookup` for `generate_resolved` failed semantically because the requested module path/kind did not match; the model recovered by reading line ranges directly.
- It overran the turn after making one more optimization edit and never published a submitted result.

## 8. Positive examples and adjudication candidates

Positive recovery chain worth preserving:

```text
read exact lib.rs line ranges around TypeId::generate_resolved and CanonId::generate_resolved
  -> non_semantic_patch replaces direct canonicalize() calls with canonicalize_cached
  -> cargo check fails with E0425 cannot find canonicalize_cached in this scope
  -> model reads ids module imports
  -> non_semantic_patch adds use crate::canonicalize_cached inside mod ids
  -> focused cargo check succeeds
  -> focused cargo test succeeds
```

Candidate adjudication signals:

- Compiler feedback was model-visible and materially used; the model did not repeat the same broken patch after `E0425`.
- The successful validations were focused on the changed crate, which is a meaningful but limited validation signal.
- Timeout after a later edit must reduce confidence: final workspace diff is not covered by the last cargo validation.
- Tool failure recovery was mixed: the model worked around `code_item_lookup` errors with reads, but the internal DB error should be treated as tool/runtime debt, not model fault.
- Request-contract adherence should be scored separately: the request asked for `ploke-eval` checks, while the model/harness ran focused `ploke-core` checks.

## 9. Trace reconstruction

Concrete chain from model/tool event to edit, validation, and timeout:

1. Events 74-80: model located `crate::workspace::canonicalize_best_effort` with `code_item_lookup`, then requested `apply_code_edit` on `workspace.rs`. The first tool completion was staged-only (`staged=1`, `applied=0`), then proposal `e75eb5c3-c258-562d-b4fd-6e3e43e5cd91` was recorded, and a later completion for the same call showed `applied=1` for `workspace.rs`.
2. Events 81-82: model ran `cargo check`; the tool returned `ok=true`, focused manifest `crates/ploke-core/Cargo.toml`, zero errors/warnings.
3. Events 83-86: two lookup attempts failed. The import lookup failed with internal DB schema error (`import` missing `tracking_hash`), and the `generate_resolved` lookup failed because no matching method was found under the requested module path/kind.
4. Events 87-95: model switched to direct reads of `lib.rs` exports and `generate_resolved` line ranges, then requested `non_semantic_patch` changing re-export/import sites and two `.canonicalize()` call sites. Proposal `f0e7bf62-1abb-5b81-be9e-512a7da40678` later applied.
5. Events 99-100: `cargo check` failed with two `E0425` errors: `cannot find function canonicalize_cached in this scope`, at lines 413 and 689 of `crates/ploke-core/src/lib.rs`.
6. Events 101-106: model read `lib.rs` imports inside `mod ids`, added `use crate::canonicalize_cached;`, and proposal `175ac429-66a0-545c-bfda-ce51e91dc91c` applied.
7. Events 107-110: `cargo check` and `cargo test` both succeeded, still focused on `crates/ploke-core/Cargo.toml`.
8. Events 111-117: model read `compilation_unit.rs` and requested a patch changing `normalized_features` to return `self.features.clone()`. The ordered events show only staged/proposal for call `function-call-341f3176-2ea7-4fbb-86a2-52ef3a55e143`; no final applied completion appears in the ordered event list. The `attempts` array nevertheless lists proposal `e0d2d371-a505-5a19-8989-51d79cc6fe78` as applied, and read-only git diff confirms the workspace contains that change.
9. Terminal: the overall headless attempt timed out after 900 seconds. Because terminal was `timed_out`, `finish_broad_headless_tui_attempt` refused to publish `submitted_result_path` JSON.

Last point where the model had enough information to act:

- It had enough information to repair the `canonicalize_cached` compile failure after event 100 and did act successfully through event 110.
- It had enough information to stop with a validated candidate after event 110, but instead made another `compilation_unit.rs` optimization edit. That final edit was not followed by recorded validation or submitted-result publication before timeout.

## 10. Protocol review and protocol blind spots

Protocol is missing for the campaign eval row and there is no protocol artifact set for this broad-harness slot.

Observed record-surface status:

Present:

- Request JSON and prompt MD.
- Candidate workspace.
- Headless diagnostics JSON.
- Turn-live trace and summary.
- Zero-byte provider sidecar path.

Record absent:

- Submitted result JSON at the request's `submitted_result_path`.
- Node-level `runner-result.json`.
- Protocol artifacts under the parent node path checked for this review.
- `record.json` in the turn-live directory; the bundled trace-audit script failed because it expects that file.

Record present, manual join needed:

- Workspace diff had to be manually joined to headless attempt/proposal IDs and event ordering.
- Validation records had to be manually joined to the edits they did and did not cover.
- The absence of submitted JSON had to be manually joined back to source behavior for `TimedOut` terminal outcomes.

Protocol/read-side blind spots:

- A staged edit can look like progress unless the later applied/failed/timeout lifecycle is joined.
- The `attempts` array and ordered events disagree for the final `compilation_unit.rs` apply lifecycle; playback needs to expose this mismatch directly.
- Focused cargo checks are not automatically equivalent to the request's validation contract.
- A dirty candidate workspace is not equivalent to a submitted/admitted child.

## 11. What is working

- Request publication and candidate workspace materialization worked for the parent slot.
- The headless TUI produced durable diagnostics and a turn-live bundle even though it timed out.
- The edit tool lifecycle captured several staged-to-applied proposal transitions.
- The model used compile feedback to repair a broken import scope and obtained focused `ploke-core` check/test success before the final timeout.
- The timeout branch avoided overclaiming by refusing to write the submitted result JSON.
- The candidate workspace is inspectable and its diff is easy to verify with read-only git commands.

## 12. What is not working yet

- No submitted broad-harness result exists for the parent slot.
- No child admission or descendant eval usefulness can be claimed from this slot.
- Raw provider response recording is absent for this turn-live bundle (`llm-full-responses.jsonl` is 0 bytes).
- The broad-harness turn-live bundle lacks `record.json`, so the standard trace audit cannot run without a broad-harness-specific path.
- The final edited workspace state was not validated after the `compilation_unit.rs` change.
- The request's validation contract and actual focused cargo validation diverged.
- The final `compilation_unit.rs` lifecycle is suspicious: projected as applied in `attempts` and present in workspace diff, but missing a final applied tool-completed event in the ordered event list.
- `code_item_lookup` has at least one internal DB/schema failure (`import.tracking_hash`) that can derail exact lookup during broad-harness attempts.

## 13. Action items

Blocker-level before treating this slot as an admitted descendant candidate:

1. Do not admit or score this parent slot until a submitted result JSON exists. Observed gap: `submitted_result_path` is absent and terminal was `timed_out`; source code intentionally refuses submission on timeout.
2. Add or expose a broad-harness timeout finalization record that explicitly states whether dirty workspace changes are discarded, preserved for inspection only, or eligible for retry. Observed gap: workspace is dirty and partially validated, but no submitted/admitted artifact exists.
3. Repair broad-harness trace lifecycle projection for final edits. Observed gap: final `compilation_unit.rs` change is present in workspace and in `attempts`, but the ordered event list ends at staged/proposal without an applied completion.

Non-blocking but important observability/tooling fixes:

4. Add broad-harness support to `run_trace_audit.py` or produce a compatible `record.json` for turn-live bundles. Observed gap: current audit fails on missing `turn-live/record.json`.
5. Persist raw provider responses for broad-harness attempts. Observed gap: provider sidecar exists but is 0 bytes, preventing provider-vs-recorded lifecycle comparison.
6. Surface validation-contract coverage. Observed gap: request asked for `ploke-eval` checks while actual validations were focused `ploke-core` checks; playback should label this as partial/contract-mismatch rather than generic cargo success.
7. Fix or classify the `code_item_lookup` import schema error. Observed gap: lookup on an import produced internal DB error `stored relation 'import' does not have field 'tracking_hash'`.
8. If this candidate idea is retried manually, require validation after all edits and consider a stronger workspace/root check. Observed gap: last cargo test occurred before the final `normalized_features` edit.
