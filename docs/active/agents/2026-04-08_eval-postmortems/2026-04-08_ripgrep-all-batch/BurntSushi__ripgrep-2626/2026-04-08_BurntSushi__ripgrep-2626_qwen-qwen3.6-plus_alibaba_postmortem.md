# BurntSushi__ripgrep-2626 qwen/qwen3.6-plus alibaba Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2626 Batch Postmortem
- task description: Document the qwen/qwen3.6-plus + alibaba batch run for `BurntSushi__ripgrep-2626`, emphasizing why it aborted before producing a patch and what tool-contract issues surfaced.
- related planning files:
  - [2026-04-08_postmortem-template.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_postmortem-template-review.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_postmortem-template-review.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-2626`
- instance: `BurntSushi__ripgrep-2626`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `7099e174acbcbd940f57e4ab4913fee4040c826e`
- stable evidence source: [/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: none found in the run artifacts

## Outcome Snapshot

- final runner state: aborted
- final chat outcome: `Request summary: [aborted] error_id=bed3d831-c4cb-4014-90da-fb3e7617a993`
- primary user-visible failure: the session aborted before any patch was produced
- did the model produce a patch: no
- did the target file change: no
- official benchmark status: not run inside `ploke-eval`
- official benchmark evidence: none beyond the local logs and empty submission artifact

## Failure Classification

- primary category: `tool-affordance-gap`
- secondary category: `tool-retry-friction`
- confidence: high

## Timeline

1. Initial diagnosis:
   The model understood the issue direction correctly: a broad Clap-to-lexopt migration with rollups and docs generation.
2. First meaningful tool failure:
   The live log shows the model still trying an unsupported `run_shell_command` tool name, which the system rejected before the session aborted. That is a contract mismatch, not just a bad edit.
3. First edit proposal:
   None reached a durable accepted state; the run produced zero edit proposals and zero create proposals in the trace artifact.
4. First compile or test failure:
   No compile/test result was reached in the run artifacts before abort. The session ended on the tool/provider failure path instead.
5. End-of-run state:
   The assistant session aborted with `HTTP_200` in the warning path and wrote only the snapshot and empty submission artifacts.

## Evidence

### Correct Local Reasoning

The prompt and issue were understood at a high level. The assistant stayed on the Clap-to-lexopt migration problem instead of wandering into unrelated code.

### Tool Friction

The key friction is explicit in the log: the model attempted `run_shell_command`, which is not a valid tool in this harness, and the session later aborted. That is stronger evidence of tool contract mismatch than of a bad code decision.

Evidence:

- unsupported tool name events in the live log: [/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- aborted terminal record with zero edits: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2626/agent-turn-trace.json)

### Model Mistake

The assistant did not recover cleanly after the tool contract mismatch. Instead of converging to the allowed tools, the session exited before emitting a patch.

### Artifact Ambiguity

The trace clearly shows an abort, but the same run still writes `final-snapshot.db`, `execution-log.json`, and `multi-swe-bench-submission.jsonl`. That can make a failed pre-patch session look more complete than it was.

### Benchmark Follow-Through

No official Multi-SWE-bench verdict was produced for this run in the ploke-eval artifacts. The submission artifact exists, but it is empty of a real fix.

## Minimal Correct Fix

There was no code fix to evaluate because the assistant never reached a viable patch state. The minimal process fix would have been to stay within the allowed tool contract and reissue the request with `request_code_context`, `code_item_lookup`, or `apply_code_edit` instead of `run_shell_command`.

## Open Questions

- Tool-design questions:
  - Should the harness surface a stronger error when the model tries a tool name that is not available in this batch?
- semantic editing capability questions:
  - Would a clearer lookup hint reduce the chance of the model reaching for unsupported shell tooling?
- runner or artifact questions:
  - Should empty submission artifacts be labeled more explicitly as pre-patch aborts?

## Follow-Up Actions

- instrumentation:
  - Surface unsupported-tool attempts as a first-class run event.
- tool UX:
  - Tighten the tool schema reminder in the prompt after the first invalid tool call.
- runner artifact changes:
  - Emit a distinct status for pre-patch aborts with empty submissions.
- regression tests:
  - Add a replay case that checks the unsupported-tool abort path produces a clean, typed failure.
