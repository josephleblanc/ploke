# BurntSushi ripgrep-2576 Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2576 Eval Postmortem
- task description: Document the completed eval run for instance `BurntSushi__ripgrep-2576`, with emphasis on model coherence, tool friction, artifact trust, and benchmark follow-through.
- related planning files:
  - [2026-04-08_postmortem-template.md](../../2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](../2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_batch-postmortem-meta-notes.md](../2026-04-08_batch-postmortem-meta-notes.md)
  - [2026-04-08_postmortem-template-review.md](../2026-04-08_postmortem-template-review.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-2576`
- instance: `BurntSushi__ripgrep-2576`
- model: `minimax/minimax-m2.5`
- provider: not surfaced in the stable summary
- repository: `BurntSushi/ripgrep`
- base sha: `fed4fea217abbc502f2e823465de903c8f2b623d`
- stable evidence source: [ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not found under the run directory

## Outcome Snapshot

- final runner state: summary artifact is internally inconsistent with the trace artifact
- final chat outcome: `aborted` in `agent-turn-trace.json`
- primary user-visible failure: repeated lookup churn around `line_anchor_start` and `fast_find`, followed by an aborted terminal record
- did the model produce a patch: yes
- did the target file change: yes
- official benchmark status: no downstream Multi-SWE-bench verdict artifact was present
- official benchmark evidence: the run only exposes the local submission JSONL and run artifacts, not an evaluator verdict

## Artifact Trust

- authoritative artifact: `agent-turn-summary.json` for tool/result inventory, paired with the stable eval log for chronology
- partial or stale artifacts: `agent-turn-trace.json` reports `outcome=aborted` and `final_assistant_message=null`, so it should not be treated as a complete narrative source
- why: the summary shows extensive patch activity and tool completions, while the trace captures the final aborted state but not the completed reasoning chain

## Failure Classification

- primary category: `tool-retry-friction`
- secondary category: `artifact-ambiguity`
- confidence: medium-high

## Timeline

1. Initial diagnosis:
   The assistant correctly focused on the `-w/--word-regexp` fast path and the `#2574` regression described in the task prompt.
2. First meaningful tool failure:
   `code_item_lookup` missed `line_anchor_start` in `crates/regex/src/config.rs`, then later missed `fast_find` in `crates/regex/src/word.rs`.
3. First edit proposal:
   The run accumulated many edits to `crates/regex/src/word.rs`, with 24 edit proposals overall.
4. First compile or test failure:
   No clean compile/test failure is surfaced in the stable artifacts; the visible failure mode is repeated lookup and patch churn.
5. End-of-run state:
   `agent-turn-trace.json` ends in `aborted`, while the summary still shows `applied: true` and `any_expected_file_changed: true`.

## Evidence

### Correct Local Reasoning

The assistant stayed coherent at the task level. It repeatedly framed the issue as a correctness bug in the `-w/--word-regexp` fast path and searched for the relevant code paths and tests.

Evidence:

- task prompt captured in the run summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)
- early assistant progress messages in the summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)

### Tool Friction

Two `code_item_lookup` calls failed on symbols that were likely adjacent to the target but not the right semantic node. The recovery hints were generic and did not immediately redirect the model to a grounded file-read path.

Evidence:

- `line_anchor_start` lookup failure: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)
- `fast_find` lookup failure: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)
- failed `non_semantic_patch` on `crates/regex/src/word.rs`: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)

### Model Mistake

The model did not obviously misunderstand the bug. The weaker point was spending too long in lookup-and-edit retry cycles instead of converging faster on a minimal edit to the existing word-boundary fast path.

### Artifact Ambiguity

The run artifacts do not agree cleanly. The summary shows many applied edits and a produced patch, but the trace file records an aborted terminal record and no final assistant message.

Evidence:

- summary artifact: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-summary.json)
- trace artifact: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/agent-turn-trace.json)

### Benchmark Follow-Through

There is no downstream Multi-SWE-bench verdict artifact in the run directory. The only benchmark-facing file present is the submission JSONL, so the official verdict remains unobserved from these artifacts.

Evidence:

- submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2576/multi-swe-bench-submission.jsonl)

## Minimal Correct Fix

Keep the existing `-w/--word-regexp` fast path and make the boundary check robust in place, rather than burning time on repeated semantic lookups and patch retries for adjacent symbols.

## Open Questions

- Tool-design questions:
  - Could `code_item_lookup` surface a more direct file-level fallback after repeated misses on related symbols?
- semantic editing capability questions:
  - Would a clearer path for modifying an existing Rust function reduce the retry churn here?
- runner or artifact questions:
  - Should `agent-turn-summary.json` and `agent-turn-trace.json` be emitted with clearer trust metadata when a run aborts late?

## Follow-Up Actions

- instrumentation:
  - Record a small canonical `first tool failure` / `first validation failure` summary in the run artifacts.
- tool UX:
  - Improve lookup hints so repeated misses do not oscillate on nearby symbols.
- runner artifact changes:
  - Mark the trace as partial or aborted more prominently when it diverges from the summary.
- regression tests:
  - Add a replay case for this bug pattern to catch future retry churn around word-boundary handling.
