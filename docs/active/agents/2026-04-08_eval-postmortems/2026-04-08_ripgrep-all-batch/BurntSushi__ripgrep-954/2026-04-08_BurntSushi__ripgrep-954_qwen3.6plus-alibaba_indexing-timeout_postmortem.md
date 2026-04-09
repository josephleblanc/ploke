# BurntSushi ripgrep-954 Indexing Timeout Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-954 Indexing Timeout Postmortem
- task description: Postmortem for the `BurntSushi__ripgrep-954` ripgrep-all batch run, which timed out during indexing before any assistant turn completed.
- related planning files:
  - [2026-04-08_postmortem-template.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-954`
- instance: `BurntSushi__ripgrep-954`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `223d7d9846bff4a9aaf6ba84f5662a1ee7ffa900`
- stable evidence source: [ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-954/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-954/run.json)
  - execution log: not present
  - turn summary: not present
  - turn trace: not present
  - submission jsonl: not present
  - official benchmark logs/report: not present

## Outcome Snapshot

- final runner state: failed during indexing
- final chat outcome: none; the run never reached a completed assistant turn
- primary user-visible failure: indexing timed out after repeated parse failures in `globset`
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
   The runner restored and activated the codestral embedding set, then began indexing `BurntSushi__ripgrep-954`.
2. First meaningful tool failure:
   The parser reported `Parse failed for crate: /home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/globset` after `Partial parsing success: 6 succeeded, 1 failed`, then later repeated the same pattern while parsing `grep`. See [log lines 291974-291976](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L291974) and [291969-291972](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L291969).
3. First edit proposal:
   None. The run never reached a model turn.
4. First compile or test failure:
   None. Indexing failed before any compile/test phase.
5. End-of-run state:
   The runner timed out waiting for `indexing_completed` and persisted [indexing-failure.db](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-954/indexing-failure.db) with label `indexing timeout`.

## Evidence

### Correct Local Reasoning

- No assistant turn completed, so there is no model reasoning to assess.

### Tool Friction

- The parser repeatedly surfaced a partial-success failure before semantic tools were available.
- The run never got to the point where the model could act on the recovery hints.

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
  - Should repeated parser failures be de-duplicated in the active log so the first failure is easier to spot?
- semantic editing capability questions:
  - N/A; no semantic turn was reached.
- runner or artifact questions:
  - Should the batch summary record the first failing crate per instance?

## Follow-Up Actions

- instrumentation:
  - Add a structured `pre-turn failure` record with the first parse failure and crate name.
- tool UX:
  - Make the indexing timeout message point directly at the first failing crate.
- runner artifact changes:
  - Persist a tiny failure summary for runs that never reach a turn.
- regression tests:
  - Add coverage for indexing-timeout runs with repeated parser failures.
