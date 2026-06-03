# Prototype 1 run review: baseline/eval turn for `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`

Status: durable run review for the completed parent baseline/eval turn. The live campaign was not advanced by this review. Protocol is still missing/pending and is not counted as successful.

## 1. Short verdict

Mechanical eval completion: yes.

Benchmark-useful outcome: likely useful but not oracle-proven. The run produced a non-empty Multi-SWE-bench patch for `BurntSushi__ripgrep-2209`, changed the expected production file `crates/printer/src/util.rs`, added a regression test in `crates/printer/src/standard.rs`, and ended with a recorded successful `cargo test` that covered changed files. The patch projection passed.

Caveats:

- Protocol did not run or did not persist artifacts: closure says protocol status `missing`, with all three required procedures missing.
- No MBE/oracle verdict was found for this baseline/eval turn; run profile has MBE disabled and oracle in record-only mode.
- `fmt_check_observed=false`; do not claim `cargo fmt` or `rustfmt` passed.
- The final benchmark patch compiles according to recorded validation, but the checkout diff shows a formatting/style regression in `util.rs`: the original doc comment on `replace_all` was removed and `pub fn replace_all` is over-indented. That is not known to break behavior, but it matters because formatting was not checked.
- One late `cargo check` call was model-described as workspace-wide, but the tool resolved it to the focused `crates/globset/Cargo.toml` manifest and validation audit marks it as not covering changed files. The actual final validation was the later workspace-root `cargo test`, not that `cargo check`.

## 2. Evidence roots

Campaign and Prototype 1 state:

- Project root used for review docs: `/home/brasides/code/ploke`
- Campaign id: `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Worktree/repo root for Prototype 1 parent: `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- Parent node: `node-26f01da56959fd47`
- Parent node dir: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/nodes/node-26f01da56959fd47`
- Campaign manifest: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/campaign.json`
- Run profile: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/run-profile.toml`
- Closure state: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/closure-state.json`
- Transition journal: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/transition-journal.jsonl`

Eval run root:

- Task: `BurntSushi__ripgrep-2209`
- Model route: `google/gemini-3.5-flash` through direct Google (`selected_provider=google`)
- Benchmark checkout: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- Base SHA: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- Run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/BurntSushi__ripgrep-2209/runs/run-1780406339507-structured-current-policy-34e9980e`
- Compressed record: `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/BurntSushi__ripgrep-2209/runs/run-1780406339507-structured-current-policy-34e9980e/record.json.gz`
- Agent turn trace: same run root, `agent-turn-trace.json`
- Agent turn summary: same run root, `agent-turn-summary.json`
- Raw provider responses: same run root, `llm-full-responses.jsonl`
- Validation audit: same run root, `validation-audit.json`
- Submission: same run root, `multi-swe-bench-submission.jsonl`
- Patch projection: same run root, `benchmark-patch-projection.json`

## 3. Closure state

Closure state is internally clear for eval versus protocol:

- Registry: `complete` (`expected_total=1`, `mapped_total=1`, `missing_total=0`).
- Eval: `complete` (`expected_total=1`, `complete_total=1`, `failed_total=0`, `missing_total=0`, `in_progress_total=0`).
- Protocol: `missing` (`expected_total=1`, `full_total=0`, `missing_total=1`).
- Required protocol procedures were all missing for this instance:
  - `tool-call-intent-segments`: `missing`
  - `tool-call-review`: `missing`
  - `tool-call-segment-review`: `missing`

This means the eval row closed, but the protocol layer did not adjudicate tool-call intent, usefulness, or segment quality.

One lifecycle caveat: the parent node record still says `status: running`, and the node directory only contains `node.json` and `runner-request.json`; the expected `runner-result.json` is absent for this parent node path. The closure eval row and run-root artifacts prove the baseline/eval turn completed, but the parent node runtime lifecycle surface is not fully closed in the node directory.

## 4. Exact execution path

The reviewed artifact came from the Prototype 1 state-machine eval path, not from a broad-harness child attempt.

Evidence:

- `runner-request.json` for `node-26f01da56959fd47` records argv:
  - `loop`
  - `prototype1-state`
  - `--repo-root`
  - `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345`
- `transition-journal.jsonl` records `parent_started` for the same campaign, parent/node id, generation 0, branch `prototype1-parent-p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-gen0`, repo root equal to the parent worktree, and pid `1856868`.
- `execution-log.json` records run arm `structured-current-policy`, role `treatment`, command `run single agent`, execution `agent-single-turn`, selected model `google/gemini-3.5-flash`, provider `google`, and steps through `benchmark_turn_completed`, `write_validation_audit`, `persist_full_response_trace`, `write_msb_submission`, and `write_benchmark_patch_projection`.
- Source path checked in the project worktree: `crates/ploke-eval/src/runner.rs` calls `run_benchmark_turn` and then writes the agent turn summary, validation audit, response trace, run record, final snapshot, submission, and patch projection.

Compact path:

```text
loop prototype1-state
  -> Prototype 1 parent state machine for node-26f01da56959fd47
  -> runner.rs single-agent eval path
  -> runner.rs::run_benchmark_turn
  -> agent-turn trace/summary + record.json.gz
  -> validation-audit + Multi-SWE-bench submission + benchmark patch projection
  -> protocol expected but missing
```

Record producers and read-side joins used in this review:

- Event producer: TUI/headless benchmark turn events captured into `agent-turn-trace.json` and `record.json.gz`.
- Recorder: eval runner/run record path, evidenced by `record.json.gz` schema `run-record.v1` and execution-log packaging steps.
- Raw provider ledger: `llm-full-responses.jsonl`, manually joined by response index and provider call id.
- Read-side/projection: `validation-audit.json`, `multi-swe-bench-submission.jsonl`, and `benchmark-patch-projection.json`.
- Protocol read-side: expected artifact root from closure state, but the path did not exist and no required procedure files were present.

## 5. Eval and patch output

Patch projection:

- Projection status: `passed`.
- Projection detail: `fix_patch exported from the recorded checkout cwd`.
- Checkout cwd: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`.
- Checkout head/base in projection: `4dc6c73c5a9203c5a8a89ce2161feca542329812`.
- Patch SHA-256 for `fix_patch`: `dd26afae4bb542e0fc98c20262d3f194e484f29c2cdafb48504575926b0771ad`.
- Patch byte length: `3573`.
- Patch line count: `99`.

The full JSONL submission file itself is larger than the patch string. Direct verification of the artifact showed:

- JSONL file byte length: `3756`.
- JSONL file SHA-256: `6fa5960393abb303e81efa88a58198b8bd42ebcc58031e5bdae085c950510d63`.
- The projection SHA/byte/line fields match the `fix_patch` string, not the whole JSONL wrapper.

Changed paths from validation audit:

- `crates/printer/src/util.rs`
- `crates/printer/src/standard.rs`

Patch content summary, verified against the benchmark checkout diff:

- `util.rs`: rewrites `Replacer::replace_all` from `replace_with_captures_at` to a custom `captures_iter_at` loop that stops when `m.start() >= end`, copies unmatched spans, interpolates replacements, pushes replacement `Match` ranges, and appends remaining subject bytes up to `max(last_match, end)`.
- `standard.rs`: adds `replacement_duplicative_multiline`, expecting `1:Z\n2:Z\n` for multiline replacement over `a\na\n`.

Suspicious style/content point verified against checkout:

- `crates/printer/src/util.rs` line 50 in the patched checkout is `        pub fn replace_all<'a>(`, with eight leading spaces instead of the surrounding four-space method indentation.
- The original doc comment on `replace_all` is gone from the diff.
- Since `fmt_check_observed=false`, the run cannot claim formatting success. Recorded `cargo test` says the code compiled and tests passed despite this style issue.

## 6. Oracle/MBE state

- Run profile has `[execution.mbe] enabled = false`.
- Run profile has `[selection.oracle] mode = "record-only"` and `require_evidence = true`.
- No instance-local `mbe` or `oracle` files were found under the eval instance root for this campaign.
- Closure state reports eval/protocol status only; it does not attach an oracle verdict.

Conclusion: this run has a benchmark-facing patch and recorded validation, but not an oracle/MBE pass/fail judgment. Treat benchmark usefulness as plausible from patch/validation evidence, not proven by external oracle.

## 7. LLM and tool behavior

Trace audit results from `run_trace_audit.py` on the run root:

- Provider responses: `36`.
- Provider-emitted tool calls: `35`.
- Recorded tool calls: `35`.
- Missing recorded provider call ids: `0`.
- Extra recorded call ids: `0`.
- Finish reasons: `35` `tool_calls`, `1` `stop`.
- Recorded classifications:
  - `completed`: `10`
  - `transport_failure`: `2`
  - `read_with_content`: `18`
  - `duplicate_request`: `5`

Notable behavior:

- The raw provider ledger and recorded lifecycle matched exactly by call id count; this is good record integrity for provider-tool joins.
- `request_code_context` failed early with an internal DB/fixed-rule error: requested fixed rule `ploke.TypeTargetPaths` was not found. The model recovered by using `read_file` and `list_dir`.
- There were many large/truncated reads, but they did contain useful content and led to targeted edits.
- First semantic edit on `util.rs` applied, then `cargo check` failed because the code used private/field access where method calls were needed.
- A follow-up semantic edit failed with `Content changed` stale-version verification. The model refreshed the file and used `non_semantic_patch` to repair the exact call sites.
- The model added a regression test after the implementation compiled and passed tests, then reran package validation.
- Final response claimed `cargo test --package grep-printer` and 95 tests passed. The tool payload for the final cargo call records `command=test`, `scope=workspace`, manifest path `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`, and output including `running 95 tests` plus 2 doc tests. Validation audit marks it as covering changed files.

## 8. Positive examples and adjudication candidates

Positive recovery chain:

```text
request_code_context failure
  -> model switched to file reads in util.rs, matcher lib.rs, standard.rs, and Cargo.toml
  -> model localized Replacer::replace_all and existing replacement tests
  -> first util.rs edit applied
  -> cargo check failed with 3 errors
  -> stale semantic edit failed with Content changed
  -> model refreshed util.rs and used non_semantic_patch
  -> cargo check passed
  -> cargo test passed
  -> model added regression test
  -> final cargo test passed with 95 unit tests and 2 doc tests
```

Candidate adjudication signals to preserve:

- Failed code-context retrieval followed by successful fallback localization is a useful recovery signal.
- Compiler feedback was materially used: after the failed `cargo check`, the model targeted `m.start()`/`m.end()` repairs.
- Stale semantic edit handling worked at the model level: it refreshed and switched patch strategy instead of repeating the stale request.
- Final validation strength is good enough for mechanical eval: successful recorded `cargo test` over the workspace-root manifest and changed-file coverage true.
- A weaker/incorrect validation claim also exists: the late `cargo check` that the model described as workspace-wide resolved to `crates/globset/Cargo.toml` and did not cover changed files. Adjudication should distinguish this from the later valid final test.
- Formatting/style quality is not covered: no fmt check, and the diff contains visibly abnormal indentation/doc removal.

## 9. Trace reconstruction

Concrete trace chain with event ids from `agent-turn-trace.json`:

1. Events 42-47: model requested `request_code_context` for `look-around`; tool failed with internal DB error for missing fixed rule `ploke.TypeTargetPaths`.
2. Events 50-152: model used `read_file` fallbacks. It read `crates/printer/src/util.rs` in chunks, `crates/matcher/src/lib.rs`, `crates/printer/src/lib.rs`, `crates/printer/src/standard.rs`, `crates/printer/Cargo.toml`, and then narrowed to `util.rs` lines 40-110. At this point it had enough information to act on `Replacer::replace_all`.
3. Events 156-166: model requested `apply_code_edit` for `crate::util::Replacer::replace_all`; the proposal first staged with `applied=0`, then later recorded `applied=1`. This is an example where the first `ToolCompleted` did not mean applied; the later lifecycle event is the actual apply.
4. Events 172-175: model ran `cargo check`; it failed (`ok=false`, errors=3). This gave enough information to repair the implementation.
5. Events 180-184: model attempted a semantic repair, but `apply_code_edit` failed because `util.rs` content had changed and the file version could not be verified. A second failure event records an internal staging failure for the same call id.
6. Events 187-193: model refreshed `util.rs` current lines 48-115.
7. Events 197-206: model used `non_semantic_patch` to change direct field access to method calls; the patch staged and then applied.
8. Events 212-223: `cargo check` and then `cargo test` succeeded.
9. Events 229-272: model searched/read `standard.rs` test regions and the end of the test module.
10. Events 274-285: model added `replacement_duplicative_multiline` with `non_semantic_patch`; staged then applied.
11. Events 291-294: `cargo test` succeeded and showed `replacement_duplicative_multiline ... ok` among `95` unit tests plus `2` doc tests.
12. Events 297-302: model requested bare `cargo check` and claimed workspace intent, but tool resolved `scope=focused`, manifest `crates/globset/Cargo.toml`, and validation audit later marks this call as `covers_changed_files=false`.
13. Events 306-310: final cargo call was `cargo test` and validation audit records `resolved_scope=workspace`, manifest `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`, `ok=true`, `covers_changed_files=true`.
14. Event 319/provider response 35: final assistant answer summarized the fix, test, and validation.

Last point where the model had enough information to act:

- First enough-to-edit point: after event 152, when it had the whole relevant `replace_all` body and had already inspected related matcher/printer context. It did act at event 156.
- Last enough-to-finalize point: after event 294, when the regression test passed under recorded package/workspace-root validation. The later focused `globset` `cargo check` was not useful for changed files, but the model recovered by running the final `cargo test` at event 310.

## 10. Protocol review and protocol blind spots

Protocol did not complete. Do not cite protocol success for this run.

Observed protocol state:

- Closure protocol status: `missing`.
- Required procedures missing: `tool-call-intent-segments`, `tool-call-review`, `tool-call-segment-review`.
- Expected protocol artifact directory from closure state did not exist at the checked path under `/home/brasides/.ploke-eval/protocol/prototype1/.../run-1780406339507-structured-current-policy-34e9980e`.

Blind spots caused by missing protocol:

- No protocol-level segmentation of tool-call intent.
- No protocol-level judgment of whether repeated truncated reads were useful or redundant.
- No protocol-level judgment of the stale semantic edit lifecycle.
- No protocol-level catch for the focused `globset` `cargo check` being semantically irrelevant to changed files.
- No protocol-level catch for style/formatting risk in the final patch.

The trace audit and manual artifact joins covered some of these gaps for this review, but they are not a substitute for persisted protocol artifacts.

## 11. Record-surface checklist highlights

Using the runtime-playback inventory labels:

Present:

- Campaign manifest: `campaign.json`.
- Closure state: `closure-state.json`.
- Run profile: `prototype1/run-profile.toml`.
- Scheduler projection: `prototype1/scheduler.json`.
- Transition journal: `prototype1/transition-journal.jsonl`.
- Parent identity/start evidence in transition journal.
- Node record: `prototype1/nodes/node-26f01da56959fd47/node.json`.
- Runner request: `prototype1/nodes/node-26f01da56959fd47/runner-request.json`.
- Run root record: `record.json.gz`.
- Agent turn trace/summary: `agent-turn-trace.json`, `agent-turn-summary.json`.
- Validation audit: `validation-audit.json`.
- Benchmark patch projection: `benchmark-patch-projection.json`.
- Multi-SWE-bench submission: `multi-swe-bench-submission.jsonl`.
- Indexing status and checkpoint DB; final snapshot status and final snapshot DB.

Record absent:

- Parent node `runner-result.json` at the node path named in `node.json` and `runner-request.json`.
- Sealed History block files (`history/blocks/segment-*.jsonl`) for this campaign root, based on checked file search.
- Branch registry `branches.json`, based on checked file search.
- Protocol artifact root and required procedure files for this run.
- Oracle/MBE verdict artifacts under the eval instance root.

Record present, manual join needed:

- Raw provider responses in `llm-full-responses.jsonl`; this review manually joined provider response indices/tool call ids to `agent-turn-trace.json` and trace-audit output.
- Benchmark patch projection and submission; this review manually compared projection fields, JSONL wrapper hash, `fix_patch` hash, and checkout diff.
- Validation audit; this review manually joined `event_index` entries to trace events and model statements.

Operator/convenience record:

- Closure state is useful progress evidence, but it is not proof of benchmark success or protocol success without joining to run-root artifacts.
- Campaign manifest and scheduler projection are scope/discovery evidence; the transition journal and run-root records carry stronger execution-path evidence for this turn.

## 12. What is working

- Eval run-root artifact production worked: record, trace, summary, raw provider responses, validation audit, final snapshot, submission, and patch projection were present.
- Provider and recorded tool ledgers matched by count and call id in trace audit: 35 provider-emitted tool calls and 35 recorded tool calls, with no missing or extra ids.
- The model recovered from a broken code-context tool by falling back to file reads.
- The model recovered from a stale semantic edit by refreshing and using non-semantic patching.
- Validation audit correctly warns that formatting was not checked and correctly identifies the final cargo validation as covering changed files.
- Patch projection exported a benchmark-facing `fix_patch` from the recorded checkout cwd.

## 13. What is not working yet

- Protocol is missing for the completed eval row.
- Parent node lifecycle records are stale/incomplete: `node.json` says `running` and no node-level `runner-result.json` was present, despite closure eval completion.
- `request_code_context` failed because a DB fixed rule was missing. This forced lower-quality/manual file reads and should be tracked separately from model behavior.
- Tool lifecycle can over-credit early `ToolCompleted` events for edits: staged-only completions appeared before the later applied event. Reviews must inspect the raw lifecycle and patch artifact, not only the first completion.
- Bare `cargo check` routing can mislead the model: one late bare check resolved to focused `globset` and did not cover changed files. Validation audit caught this, but the model-visible claim was still misleading.
- Formatting is not validated. The final diff includes abnormal indentation and removed method docs.
- Raw provider responses remain a manual join rather than a first-class playback family beside agent-turn events.

## 14. Action items

Blocker-level before claiming a complete campaign/protocol result:

1. Run or repair protocol production for this run root, then persist the three required procedure families. Observed gap: closure protocol status is `missing`, and the expected protocol artifact directory did not exist. Until fixed, do not claim protocol success.
2. Reconcile parent node lifecycle closure with eval closure. Observed gap: node `status=running` and node-level `runner-result.json` absent while closure eval is complete. This is an authority/lifecycle mismatch; route to the Prototype 1 lifecycle/blocker repair path if campaign advancement depends on the node being sealed/closed.

Non-blocking but important observability/tooling fixes:

3. Fix `request_code_context` DB fixed-rule availability or add a clearer fallback/error classification. Observed gap: `ploke.TypeTargetPaths` missing caused an internal tool failure despite indexing status being `completed`.
4. Expose edit proposal lifecycle as `requested -> staged -> applied/failed/stale/denied` in playback. Observed gap: same call id emitted staged-only `ToolCompleted` before actual applied events.
5. Make cargo tool resolution model-visible and policy-aware when a bare command resolves to a focused unrelated manifest. Observed gap: event 302 `cargo check` resolved to `crates/globset/Cargo.toml` and did not cover changed files while the model described a workspace check.
6. Add or require explicit formatting validation when final patches touch Rust source, or make `fmt_check_observed=false` a stronger adjudication signal. Observed gap: final patch has abnormal indentation/doc removal and no fmt evidence.
7. Load `llm-full-responses.jsonl` as a typed sidecar in playback and join it to agent-turn/tool events by response index and call id. Observed gap: this review had to manually join provider and lifecycle ledgers.

Benchmark follow-up, if this patch is selected:

8. Consider normalizing the style-only defects before submitting/using the patch as a candidate: restore the `replace_all` doc comment or confirm intentional removal, fix indentation, and run formatting. This is not a claim that the current patch fails; it is a quality concern because validation did not include formatting.
