# BurntSushi ripgrep-2209 Eval Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2209 Eval Postmortem
- task description: Postmortem for the `BurntSushi__ripgrep-2209` ripgrep-all batch run, with emphasis on model error, tool friction, and artifact ambiguity.
- related planning files:
  - [2026-04-08_postmortem-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-plan.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `ploke_eval_20260408_063814_328460.log`
- instance: `BurntSushi__ripgrep-2209`
- model: `minimax/minimax-m2.5`
- provider: `friendli`
- repository: `BurntSushi/ripgrep`
- base sha: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- stable evidence source: [ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not present in local artifacts

## Outcome Snapshot

- final runner state: completed
- final chat outcome: completed with `Request summary: [success]`
- primary user-visible failure: the first fix attempt still failed `grep-regex` tests with a boundary mismatch in `word::tests::various_find`
- did the model produce a patch: yes
- did the target file change: yes, `crates/printer/src/util.rs`
- official benchmark status: no separate benchmark verdict was available in the local artifacts
- official benchmark evidence: the local submission artifact was written, but no external evaluator result was captured

## Failure Classification

- primary category: `tool-affordance-gap`
- secondary category: `artifact-ambiguity`
- confidence: medium

## Timeline

1. Initial diagnosis:
   The model correctly identified the replacement-path boundary issue and focused on the same `range.end` guard already used by the search path.
2. First meaningful tool failure:
   `code_item_lookup` reported multiple matches for `replace_with_captures_at`, and `non_semantic_patch` rejected a malformed payload before the run recovered. See [log lines 266479-266516](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266479).
3. First edit proposal:
   The model proposed inserting a guard in the replacement callback for `crates/printer/src/util.rs`; the final submission shows that exact shape.
4. First compile or test failure:
   The first concrete validation failure was `word::tests::various_find`, which reported `(0, 5)` where `(1, 4)` was expected. See [log lines 266706-266707](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266706).
5. End-of-run state:
   The agent recovered, re-ran tests successfully, and wrote the submission JSONL.

## Evidence

### Correct Local Reasoning

- The model recognized the real bug class: replacement logic needed the same boundary rejection already used by `find_iter_at_in_context`.
- The final patch in the submission adds exactly that guard in `crates/printer/src/util.rs` before interpolation. See [submission jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl) and [log lines 266611-266617](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266611).

### Tool Friction

- `code_item_lookup` could not uniquely resolve `replace_with_captures_at` and returned a multi-match invariant error instead of a clear next step. See [log lines 266479-266480](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266479).
- `non_semantic_patch` rejected a malformed diff payload, which forced the run onto a slower recovery path. See [log lines 266482-266498](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266482).
- The trace shows the agent eventually switching to direct file reads, which was the right fallback once semantic lookup stopped being reliable.

### Model Mistake

- The model’s first validation attempt was not yet correct; it still failed the `grep-regex` regression suite on a boundary mismatch in `word::tests::various_find`.
- The mistake was not the bug target, but the exact boundary behavior: the first attempt did not line up with the expected start/end offsets. See [log lines 266706-266707](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log#L266706).

### Artifact Ambiguity

- `agent-turn-summary.json` does not carry the final assistant content (`final_assistant_message` is null), so the trace is the only place that preserves the last assistant messages.
- `agent-turn-trace.json` shows `patch_artifact.expected_file_changes` as empty even though the submission clearly changed `crates/printer/src/util.rs`, which makes the artifact state harder to trust at a glance.
- No separate official benchmark verdict was emitted into the local run directory, so the submission artifact is the only completion proof available locally.

### Benchmark Follow-Through

- The local run wrote [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl), but there is no external benchmark report in the local artifacts.
- A follow-up verdict would need the batch evaluator output, not just the run directory artifacts.

## Minimal Correct Fix

Keep the existing replacement path and add the same `range.end` rejection guard that the search path already uses, before interpolating replacement text.

## Open Questions

- Tool-design questions:
  - Should `code_item_lookup` surface a disambiguation path when multiple matches satisfy the same file/module/item tuple?
  - Should malformed patch payloads be converted into a stronger structured hint than a generic diff-format rejection?
- semantic editing capability questions:
  - Should the edit tool expose a more explicit "insert into existing method body" action instead of requiring a full replacement-shaped patch?
- runner or artifact questions:
  - Should the turn summary retain the final assistant message content, not just the final status?
  - Should submission artifacts record the applied file set more explicitly when `expected_file_changes` is empty?

## Follow-Up Actions

- instrumentation:
  - Record the first tool error, the first recovery step, and the first validation failure as separate reportable fields.
- tool UX:
  - Improve `code_item_lookup` guidance for trait or multi-match targets.
  - Make patch-format failures include a concrete example diff skeleton.
- runner artifact changes:
  - Persist the final assistant message text in the turn summary.
  - Persist the applied file list alongside the submission artifact.
- regression tests:
  - Add a replay fixture for this boundary-replacement regression so future tool changes can be compared against it.
