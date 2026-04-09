# BurntSushi ripgrep-2209 qwen/alibaba Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2209 qwen/alibaba Postmortem
- task description: Batch-specific postmortem for the `qwen/qwen3.6-plus` + `alibaba` ripgrep-all run on instance `BurntSushi__ripgrep-2209`, with emphasis on tool friction, artifact trust, and comparison against earlier model reports.
- related planning files:
  - [2026-04-08_postmortem-template.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_batch-postmortem-meta-notes.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-meta-notes.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-2209`
- instance: `BurntSushi__ripgrep-2209`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- stable evidence source: [ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not present under the local run directory

## Outcome Snapshot

- final runner state: completed
- final chat outcome: completed with `Request summary: [success]`
- primary user-visible failure: an early `read_file` call targeted the `crates/globset` directory instead of a file, then the run recovered
- did the model produce a patch: yes
- did the target file change: yes, `crates/printer/src/util.rs`
- official benchmark status: no downstream Multi-SWE-bench verdict artifact was captured locally
- official benchmark evidence: the local submission JSONL was written, but no external evaluator result was present in the run dir

## Failure Classification

- primary category: `tool-affordance-gap`
- secondary category: `artifact-ambiguity`
- confidence: medium

## Timeline

1. Initial diagnosis:
   The model correctly identified the printer replacement boundary issue and focused on the same `range.end` guard already used by the search path.
2. First meaningful tool failure:
   `read_file` was called on `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset`, which is a directory, so the tool returned `InvalidFormat`. See [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json#L84) and [agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/agent-turn-trace.json#L112).
3. First edit proposal:
   The eventual patch applied to `crates/printer/src/util.rs` and the turn summary records two applied edit proposals for that file.
4. First compile or test failure:
   None surfaced in the local run. `cargo check -p grep-regex` succeeded, and the run completed with a success summary.
5. End-of-run state:
   The agent recovered, completed the turn successfully, and wrote the submission JSONL with `util.rs` changed.

## Evidence

### Correct Local Reasoning

- The model recognized the actual bug class: replacement logic needed the same boundary rejection already used by `find_iter_at_in_context`.
- The final patch changed `crates/printer/src/util.rs`, and the turn trace records the run as completed successfully with 16 attempts.

### Tool Friction

- The first bad tool call was a directory path passed to `read_file`, which produced an `InvalidFormat` retry hint instead of a useful file read.
- The recovery path was ultimately successful, but the initial tool response was a poor affordance for the model's first exploration step.

### Model Mistake

- No major model mistake surfaced after the initial tool miscall. The model stayed aligned with the bug description and recovered to a correct patch.

### Artifact Ambiguity

- `final_assistant_message` is still null in the turn trace, so the last natural-language output is not preserved there.
- There is no separate official benchmark verdict in the run directory, so local completion is not the same as benchmark validation.

### Benchmark Follow-Through

- The local run wrote [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/multi-swe-bench-submission.jsonl), but the external evaluator output was not captured locally.
- This report should be read as a local run postmortem, not as the official benchmark verdict.

## Minimal Correct Fix

Keep the existing replacement path and add the same `range.end` rejection guard that the search path already uses, before interpolating replacement text.

## Open Questions

- Tool-design questions:
  - Should `read_file` explain the directory/file distinction earlier, before the model spends a turn on the wrong path?
- semantic editing capability questions:
  - Would a more direct "inspect file contents" affordance reduce the chance of a directory/file mismatch?
- runner or artifact questions:
  - Should the turn trace retain `final_assistant_message` even when the run completes successfully?

## Follow-Up Actions

- instrumentation:
  - Record the first tool error and first recovery step as explicit fields in the run artifacts.
- tool UX:
  - Make directory/file misuse return a shorter, more actionable hint.
- runner artifact changes:
  - Persist the final assistant message text in the turn trace or summary.
- regression tests:
  - Add a replay fixture for this boundary-replacement regression.
