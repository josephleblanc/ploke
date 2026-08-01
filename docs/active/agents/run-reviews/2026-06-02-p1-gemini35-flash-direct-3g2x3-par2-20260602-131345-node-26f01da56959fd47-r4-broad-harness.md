# Run review: node-26f01da56959fd47-r4 broad harness

## Short verdict

`node-26f01da56959fd47-r4` is mechanically complete: the published headless-TUI attempt produced a submitted-result JSON, an applied edit, and a clean candidate workspace commit `6f06760dec15e9dcf1fe52f75bf61dd191260f54` on branch `prototype1-broad-broad-harness-request-node-26f01da56959fd47-r4`.

Benchmark usefulness is negative, not merely unknown. The r4 artifact was later materialized as child node `node-008968ba900ec740` / branch `branch-f9891f3e7c5d1375`, the child runner succeeded, and `prototype1/evaluations/branch-f9891f3e7c5d1375.json` rejected it because `tool_calls_failed` regressed from `2` to `3`. The change also did not run the request-declared validation contract; model-visible validation stayed focused on `crates/ploke-protocol/Cargo.toml` instead of `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`.

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r4.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r4.md`
- Headless result: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r4.headless-tui.json`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r4.json`
- Turn trace/summary/raw provider sidecar: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r4.turn-live/`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness/node-26f01da56959fd47-r4`
- Admitted child node: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/nodes/node-008968ba900ec740`
- Branch evaluation: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/evaluations/branch-f9891f3e7c5d1375.json`
- Treatment closure: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-treatment-branch-f9891f3e7c5d1375-1780409152614/closure-state.json`

## Execution path proved

This slot ran through the broad-harness headless-TUI adapter and was then admitted/evaluated as a child artifact:

```text
published edit-harness request r4
  -> headless ploke-tui adapter / model-tool trace
  -> non_semantic_patch staged and applied
  -> candidate commit 6f06760d
  -> materialize/build/spawn child node-008968ba900ec740
  -> prototype1-runner treatment campaign
  -> branch comparison reject
```

Evidence:

- Request id `broad-harness-request:node-26f01da56959fd47:r4`, prompt path, and submitted-result path are recorded in the request JSON.
- `agent-turn-summary.json` records task id `broad-harness-request:node-26f01da56959fd47:r4`, selected model `google/gemini-3.5-flash`, terminal outcome `completed`, and terminal summary with a recovered `TOOL_EXECUTION_FAILED` diagnostic.
- `node-26f01da56959fd47-r4.headless-tui.json` records terminal state `applied`, proposal id `a95bd8cd-b65d-583e-8433-c3159b4c1f1d`, request id `8fa769b1-ac38-490e-839b-a55b92671be9`, and changed path `crates/ploke-protocol/src/procedure.rs`.
- Workspace git evidence shows clean `HEAD 6f06760dec15e9dcf1fe52f75bf61dd191260f54`, parent `8f478c66e30224adf234439c8c6395bf521a6373`, and commit subject `prototype1 broad harness result broad-harness-request:node-26f01da56959fd47:r4`.
- Child `node.json` for `node-008968ba900ec740` links `patch_id` `broad-harness:broad-harness-request:node-26f01da56959fd47:r4` to `derived_artifact_id` `artifact:git-commit:6f06760dec15e9dcf1fe52f75bf61dd191260f54` and status `succeeded`.

The bundled `run_trace_audit.py` was not directly usable on the `.turn-live` directory because that sidecar has no `record.json`; it failed looking for `node-26f01da56959fd47-r4.turn-live/record.json`. This review therefore manually joined `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, headless result JSON, git state, and the child/evaluation artifacts.

## Closure and live-run state

- Baseline campaign `closure-state.json` reports eval complete but baseline protocol still `missing` at `2026-06-02T13:22:36.932391457+00:00`.
- Treatment closure for r4 reports eval `complete`, protocol `complete`, and one compared instance for `BurntSushi__ripgrep-2209`.
- Parent `node-26f01da56959fd47/node.json` still reports `status: running` in its persisted node state.
- A read-only process check during this review showed the parent loop still running as `./target/debug/ploke-eval loop prototype1-state --debug-tools`, plus an active `node-6ebbc85b4bd81864` child runner. Defunct earlier child `ploke-eval` processes were also visible.

## Eval, patch output, and submitted result

Submitted-result JSON is present at `node-26f01da56959fd47-r4.json`. It records the candidate workspace and changed-file summary, but not the final commit hash; the commit had to be recovered from git and the child node record.

Verified patch:

- `git show --stat HEAD` reports `crates/ploke-protocol/src/procedure.rs | 17 +++++++++++++++--`.
- The diff changes `FanOut::run`: under `#[cfg(feature = "llm")]` it runs `self.left.run(subject.clone())` and `self.right.run(subject.clone())` with `tokio::join!`; under `#[cfg(not(feature = "llm"))]` it preserves sequential awaits. The later error handling consumes `left_res` and `right_res`.
- `git status --short` in the candidate workspace was clean after commit `6f06760d`.

Validation mismatch:

- The request/result contract lists `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`.
- The headless validations actually recorded five focused `ploke-protocol` cargo calls: `cargo check`, `cargo test`, `cargo check`, `cargo test`, and a final `cargo check`, all with manifest path `.../crates/ploke-protocol/Cargo.toml` and exit code `0`.
- No model-visible validation evidence in this r4 trace proves `ploke-eval` compiled or that `edit_surface` tests passed.

Descendant evaluation:

- `node-008968ba900ec740/runner-result.json` reports `status: succeeded`, `disposition: succeeded`, and treatment campaign id `p1-gemini35-flash-direct-3g2x3-par2-20260602-131345-treatment-branch-f9891f3e7c5d1375-1780409152614`.
- `prototype1/evaluations/branch-f9891f3e7c5d1375.json` records `overall_disposition: reject` with reason `BurntSushi__ripgrep-2209: tool_calls_failed regressed: 2 -> 3`.
- Compared metrics show baseline `tool_calls_total: 35`, `tool_calls_failed: 2`, and treatment `tool_calls_total: 52`, `tool_calls_failed: 3`; both had nonempty valid patches and convergence, so the rejection was specifically operational-tool regression, not absence of an eval run.

## Oracle/MBE state

MBE is disabled in the run profile. The r4 treatment closure has complete protocol artifacts, but the branch evaluation is a mechanized operational comparison; this review found no oracle/MBE report that would overturn the `reject` disposition.

## LLM and tool behavior

Trace inventory from `agent-turn-trace.json`:

- `ToolRequested`: 35
- `ToolCompleted`: 34
- `ToolFailed`: 2
- `TurnFinished`: 1
- Requested tools: `cargo` 5, `code_item_lookup` 1, `list_dir` 7, `non_semantic_patch` 1, `read_file` 17, `request_code_context` 4.

Concrete chain:

1. The model explored the workspace and read parent state. A `read_file` of the parent `runner-result.json` failed because that file was absent at the time.
2. It selected `crates/ploke-protocol/src/procedure.rs` as a target and attempted `code_item_lookup` for method `run` in `crate::procedure`; the tool failed with `invalid_format` because multiple `run` methods matched.
3. The model recovered by reading `crates/ploke-protocol/src/procedure.rs` lines `330..440` directly, then reading tests from `601..1000`.
4. It requested a `non_semantic_patch` to parallelize `FanOut::run` with `tokio::join!` under the `llm` feature. The same call first completed as staged (`ok:true`, `staged:1`, `applied:0`, `auto_confirmed:false`) and later completed as applied (`applied:1`, `partial:false`) for `crates/ploke-protocol/src/procedure.rs`.
5. It ran focused `cargo check` / `cargo test` on `crates/ploke-protocol/Cargo.toml`; all recorded validations passed, but none matched the declared `ploke-eval` validation commands.
6. Late trace activity searched for `fan_out`, `descendant performance`, `ToolCallReview`, and related protocol code; the final recorded cargo command was another focused `ploke-protocol` `cargo check` with exit code `0`.

Last point where the model had enough local information to act: after the direct `procedure.rs` reads it had enough information to make the FanOut concurrency edit. It did not have evidence that this would improve the descendant benchmark, and the later branch evaluation showed it did not under the run's operational metrics.

## Provider/raw trace and protocol blind spots

- The turn summary combines terminal `completed` with a diagnostic containing `code=TOOL_EXECUTION_FAILED`, caused by recovered tool failures rather than a terminal failed attempt. The headless terminal state and git state must be consulted to see that an edit was applied.
- `agent-turn-summary.json` has `patch_artifact.applied: true` but empty `edit_proposals`, empty `expected_file_changes`, and `any_expected_file_changed: false`, despite the headless result and git commit proving a one-file change.
- `llm-full-responses.jsonl` is not a clean slot-local ledger for r4. A jq comparison found 97 provider-emitted tool-call ids versus 35 recorded `ToolRequested` ids; 63 raw ids were absent from the trace, and one recorded trace request was absent from the raw ids. The raw sidecar also contains a final narrative about `ploke-ty-mcp` changes that contradicts the actual r4 artifact (`ploke-protocol/src/procedure.rs`). For this review, the authoritative executed path is the recorded trace, headless result, git commit, and child/evaluation artifacts.
- The submitted-result JSON lists requested checks but not actual validation commands or the final commit hash.

## What is working

- The broad-harness path produced an applied, committed candidate and a submitted-result JSON.
- Staged-versus-applied edit lifecycle was preserved for the patch call.
- The live campaign admitted the r4 artifact, ran a child treatment, completed protocol for that treatment, and produced a branch evaluation.
- The branch evaluation gave a concrete negative signal (`tool_calls_failed` regression) rather than leaving usefulness unknown.

## What is not working yet

- The headless validation surface did not enforce or clearly report the request-declared `ploke-eval` validation contract.
- Raw provider and summary sidecars are misleading for this slot unless manually cross-checked against the trace and git commit.
- The terminal status text still conflates recovered tool failures with terminal attempt outcome.
- The submitted-result JSON is too thin for later review: it omits commit hash, actual validation commands, and descendant evaluation linkage.

## Action items

1. **Validation-contract gap:** Record and adjudicate whether the model actually ran `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface`; r4 only proved focused `ploke-protocol` commands.
2. **Selection/evaluation signal:** Treat r4 as a rejected descendant candidate unless a later artifact explicitly supersedes `branch-f9891f3e7c5d1375.json`; the observed branch evaluation rejects it for `tool_calls_failed` regression.
3. **Raw provider sidecar isolation:** Investigate why `llm-full-responses.jsonl` for r4 contains many provider tool-call ids and a final narrative inconsistent with the executed r4 trace and commit.
4. **Submitted-result schema gap:** Include final candidate commit, actual validations run, and child/evaluation ids in the submitted-result JSON so reviewers do not have to recover them from git and transition records.
5. **Tool lifecycle diagnostics:** Keep recovered `ToolFailed` events visible, but separate them from terminal outcome so `completed/applied` attempts are not summarized as `TOOL_EXECUTION_FAILED` without context.
