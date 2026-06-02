# BurntSushi__ripgrep-2209 Patch Accounting Investigation

## Scope

This report investigates why the latest `BurntSushi__ripgrep-2209` eval run showed a diff-looking transcript while `ploke-eval` reported `patch proposed=no` and recorded `empty_patch`.

The key distinction is that the current authoritative run is the registered April 21, 2026 nested attempt, not the older legacy top-level instance directory. The authoritative registration is [run-1776777880059-structured-current-policy-b3bcb6bb.json](/home/brasides/.ploke-eval/registries/runs/run-1776777880059-structured-current-policy-b3bcb6bb.json:41), which points at the nested run root [execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/execution-log.json:1). The stale legacy directory under `~/.ploke-eval/runs/BurntSushi__ripgrep-2209/` should not be used as the primary source for this run.

## What We Can Confirm

### CLI-first observations

- `ploke-eval run list --instance BurntSushi__ripgrep-2209` resolves one latest attempt and reports `Execution=completed` and `Submission=empty_patch`. That matches the authoritative registration state in [run-1776777880059-structured-current-policy-b3bcb6bb.json](/home/brasides/.ploke-eval/registries/runs/run-1776777880059-structured-current-policy-b3bcb6bb.json:44).
- `ploke-eval transcript --instance BurntSushi__ripgrep-2209` prints a diff-looking assistant message. This is expected from the transcript implementation: it opens the run’s final snapshot DB and prints assistant conversation rows from there, not from `record.json.gz` or the submission artifact. See [run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:250) and [run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:258).
- `ploke-eval inspect turn --instance BurntSushi__ripgrep-2209 1` reports:
  - `tools .............. 8`
  - `patch proposed ..... no`
  - `patch applied ...... no`
- `ploke-eval inspect tool-calls --instance BurntSushi__ripgrep-2209` shows 8 successful calls, all reads/context lookups, with no patch-edit tool call.
- `ploke-eval inspect turn --instance BurntSushi__ripgrep-2209 1 --show messages --exclude-roles system,user` reports no messages.
- `ploke-eval inspect turn --instance BurntSushi__ripgrep-2209 1 --show responses` shows the raw final `stop` completion and that its content is a diff-like assistant response.

### Authoritative artifact observations

- The authoritative registration says the run finished at `2026-04-21T13:27:04.773332966+00:00`, packaging completed, and `submission_status` is `empty_patch`. See [run-1776777880059-structured-current-policy-b3bcb6bb.json](/home/brasides/.ploke-eval/registries/runs/run-1776777880059-structured-current-policy-b3bcb6bb.json:41).
- The execution log shows this sequence:
  - `benchmark_turn_completed`
  - `persist_full_response_trace`
  - `snapshot_completed`
  - `write_snapshot_status`
  - `write_msb_submission`
  See [execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/execution-log.json:21).
- The raw response sidecar [llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/llm-full-responses.jsonl:1) contains:
  - 8 assistant responses with `finish_reason="tool_calls"`
  - 1 final assistant response with `finish_reason="stop"` whose `message.content` is a fenced diff at [llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/llm-full-responses.jsonl:9)
- The per-run submission artifact is explicitly empty: [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/multi-swe-bench-submission.jsonl:1).
- The turn trace persisted:
  - a successful terminal record
  - no captured final assistant message
  - no captured `llm_response`
  - no patch proposals
  - unchanged expected file hash for `crates/printer/src/util.rs`
  This is visible in [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/agent-turn-trace.json) and the same fields are copied into [record.json.gz](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/record.json.gz).
- Counting the event kinds in the persisted trace gives:
  - `LlmResponse` events: `0`
  - `LlmEvent` debug-string events: `2`
  - `TurnFinished` events: `1`
  - `ToolRequested`: `8`
  - `ToolCompleted`: `8`
  This confirms the run artifact never recorded a structured final LLM response even though the sidecar did.

## Pipeline Trace

### 1. Model output to transcript surface

Confirmed:

- The final model completion exists in the raw sidecar as a `stop` response with diff-looking assistant text in [llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/llm-full-responses.jsonl:9).
- `transcript` does not read `record.json.gz`; it reads assistant turns from the final snapshot DB via [run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:266). That code path starts at [run_history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/run_history.rs:250).

Implication:

- A diff-looking transcript only proves an assistant message existed in snapshot-backed conversation state. It does not prove a patch tool was called or that the repo changed.

### 2. Model output to run record

Confirmed:

- `handle_benchmark_event` only records final assistant content into the turn artifact when it sees `AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response { ... }))`, where it sets `artifact.llm_response` and pushes `ObservedTurnEvent::LlmResponse`. See [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3338).
- `RunRecord::add_turn_from_artifact` reconstructs `turn.llm_response` only from `ObservedTurnEvent::LlmResponse` by calling `extract_llm_response_from_events`. See [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:544) and [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:1488).
- `TurnRecord::messages()` appends an assistant message only if `artifact.llm_response` is present. See [record.rs](/home/brasides/code/ploke/crates/ploke-eval/src/record.rs:905).
- For this run, the persisted trace and record have no `LlmResponse` event and no `llm_response`.

Implication:

- `inspect turn --show messages` has no assistant message because the turn record never captured one, even though the raw sidecar and transcript surfaces still have the freeform diff.

### 3. Turn record to `patch proposed=no`

Confirmed:

- The CLI’s `patch proposed` flag is not based on freeform assistant text. It is computed solely by checking whether any tool call used `non_semantic_patch`. See [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:8428).
- This run has 8 tool calls and none are patch tools.

Implication:

- `patch proposed=no` is internally correct for the current implementation. The assistant typed a diff, but never proposed a patch through the patch tool path that this metric tracks.

### 4. Turn / repo state to submission artifact

Confirmed:

- The submission artifact is written by computing a git diff of the working tree against `base_sha` or `HEAD`, not by reusing assistant text. See [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1001) and [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:1034).
- The run’s `patch_artifact` says:
  - `edit_proposals = []`
  - `create_proposals = []`
  - `any_expected_file_changed = false`
  - expected file `crates/printer/src/util.rs` has identical before/after SHA-256
- The written submission JSONL has `fix_patch: ""` in [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776777880059-structured-current-policy-b3bcb6bb/multi-swe-bench-submission.jsonl:1).

Implication:

- No repo mutation was captured for submission. Under the current packaging logic, an empty submission artifact is the correct result.

### 5. Submission artifact to run accounting

Confirmed:

- After writing the submission artifact, the runner reloads `fix_patch` and updates the registration’s submission status from the string contents. See [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2418).
- `update_submission_status` classifies non-empty string as `nonempty_patch`, empty string as `empty_patch`, and missing artifact as `missing`. See [inner/registry.rs](/home/brasides/code/ploke/crates/ploke-eval/src/inner/registry.rs:298).
- The registration therefore ends at `submission_status = "empty_patch"` in [run-1776777880059-structured-current-policy-b3bcb6bb.json](/home/brasides/.ploke-eval/registries/runs/run-1776777880059-structured-current-policy-b3bcb6bb.json:72).

## Conclusion

What is confirmed:

- The model did emit a freeform diff-like final answer.
- That answer survived in two places:
  - the raw full-response sidecar
  - the final snapshot DB that powers `ploke-eval transcript`
- The run record did not capture that answer as `llm_response`.
- No patch tool was called.
- No repo change was recorded for the expected file.
- The submission artifact was therefore empty, and the run registry correctly classified it as `empty_patch`.

What is inference:

- The most plausible cause of the missing `llm_response` in the run record is a timing gap in `run_benchmark_turn`: the loop exits as soon as `artifact.terminal_record` is set by `ChatTurnFinished`, at [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:3233), and after that it does not keep draining the realtime/background receivers before finalizing the artifact. Since the raw full-response sidecar is copied later from a separate log source at [runner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/runner.rs:2348), a final `ChatEvt::Response` can plausibly land in the sidecar and snapshot-backed conversation while still missing from `ObservedTurnEvent::LlmResponse`.
- I cannot prove the exact event ordering from persisted logs alone, because the persisted trace shows only the absence of `LlmResponse`, not the dropped event itself.

## Practical Reading

The shortest accurate explanation is:

- `transcript` showed a typed diff because it reads snapshot conversation state.
- `patch proposed=no` because the model never used a patch tool.
- `empty_patch` because the repo never changed, so git diff packaging emitted `fix_patch: ""`.
- The confusing part is that the final assistant text was not copied into the run record, so CLI inspection surfaces split:
  - `transcript` and `inspect ... --show responses` reveal the diff-like text
  - `inspect turn` and submission accounting do not
