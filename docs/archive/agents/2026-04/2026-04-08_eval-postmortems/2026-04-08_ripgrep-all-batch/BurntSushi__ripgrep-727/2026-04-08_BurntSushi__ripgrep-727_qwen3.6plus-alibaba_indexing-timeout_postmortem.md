# BurntSushi ripgrep-727 Indexing Timeout Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-727 Indexing Timeout Postmortem
- task description: Postmortem for the `BurntSushi__ripgrep-727` ripgrep-all batch run, which timed out during indexing before any assistant turn completed.
- related planning files:
  - [2026-04-08_postmortem-template.md](../../2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](../2026-04-08_batch-postmortem-index.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-727`
- instance: `BurntSushi__ripgrep-727`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `b6177f0459044a7e3fb882ecda9c80e44e4d95de`
- stable evidence source: [ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-727/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-727/run.json)
  - execution log: not present
  - turn summary: not present
  - turn trace: not present
  - submission jsonl: not present
  - official benchmark logs/report: not present

## Outcome Snapshot

- final runner state: failed during indexing
- final chat outcome: none; the run never reached a completed assistant turn
- primary user-visible failure: indexing timed out after repeated parse failures in `globset` and `grep`
- did the model produce a patch: no
- did the target file change: no evidence of a patch or file edit
- official benchmark status: not reached
- official benchmark evidence: only the batch summary and indexing-failure snapshot exist locally

## Failure Classification

- primary category: `tool-affordance-gap`
- secondary category: `artifact-ambiguity`
- confidence: high

## Timeline

1. Initial diagnosis:
   The runner restored and activated the codestral embedding set, then began indexing `BurntSushi__ripgrep-727`.
2. First meaningful tool failure:
   The parser reported `Parse failed for crate: /home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset` and then continued to surface partial parsing success for `grep`. See [log lines 292038-292042](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L292038).
3. First edit proposal:
   None. The run never reached a model turn.
4. First compile or test failure:
   None. Indexing failed before any compile/test phase.
5. End-of-run state:
   The runner timed out waiting for `indexing_completed` and persisted [indexing-failure.db](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-727/indexing-failure.db) with label `indexing timeout`.

## Evidence

### Correct Local Reasoning

- No assistant turn completed, so there is no model reasoning to assess.

### Tool Friction

- The parser produced a repeated partial-success failure before semantic tools were available.
- The agent never advanced beyond the index/bootstrap phase.

### Model Mistake

- No model mistake is evidenced here; the model never got a turn.

### Artifact Ambiguity

- There is no `agent-turn-summary.json`, `agent-turn-trace.json`, or `execution-log.json` for this instance.
- The canonical identity fields live in `run.json.source.*`; the top-level identity fields are null.
- The batch summary reports only `timed out waiting for 'indexing_completed' after 300 seconds`.

### Benchmark Follow-Through

- The official benchmark was not reached because indexing never completed.

## Minimal Correct Fix

No model-side fix applies. The runner should surface indexing bootstrap failures more explicitly so a pre-turn timeout is easier to distinguish from an agent failure.

## Open Questions

- Tool-design questions:
  - Should the parser or runner summarize the first failing crate instead of repeating partial-success lines?
- semantic editing capability questions:
  - N/A; no semantic turn was reached.
- runner or artifact questions:
  - Should pre-turn failures get their own artifact type distinct from normal run summaries?

## Follow-Up Actions

- instrumentation:
  - Add a structured `pre-turn failure` record with the first parse failure and crate name.
- tool UX:
  - Make the indexing timeout message point directly at the first failing crate.
- runner artifact changes:
  - Persist a tiny failure summary for runs that never reach a turn.
- regression tests:
  - Add coverage for indexing-timeout runs that fail across multiple crates.
