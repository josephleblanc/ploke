# Eval Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-454 postmortem
- batch id: `ripgrep-all`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`

## Header

- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-454`
- instance: `BurntSushi__ripgrep-454`
- repository: `BurntSushi/ripgrep`
- base sha: `c50b8b4125dc7f1181944dd92d0aca97c2450421`
- stable evidence source: `run.json`, `execution-log.json`, `agent-turn-summary.json`, `agent-turn-trace.json`, `multi-swe-bench-submission.jsonl`, live batch log
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-454/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not run

## Outcome Snapshot

- final runner state: batch step completed, run artifacts written
- final chat outcome: aborted after 12 attempts
- primary user-visible failure: the assistant never produced a patch; the chat aborted with `TRANSPORT_TIMEOUT`
- did the model produce a patch: no
- did the target file change: no, `src/printer.rs` hash was unchanged
- official benchmark status: not run
- official benchmark evidence: none

## Failure Classification

- primary category: `tool-retry-friction`
- secondary category: `provider-behavior`
- confidence: high

## Timeline

1. Initial diagnosis: the model was in the right area for the `--only-mathing` bug and inspected printer/search code, but it did not converge on the minimal fix.
2. First meaningful tool failure: `code_item_lookup` missed `matched` in `src/printer.rs` and returned a `node_kind` hint that did not resolve the actual issue ([ploke_eval_20260408_085501_361377.log#L298365](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L298365)).
3. First edit proposal: none.
4. First compile or test failure: none.
5. End-of-run state: the session aborted with `TRANSPORT_TIMEOUT`, and `multi-swe-bench-submission.jsonl` contained an empty `fix_patch`.

## Evidence

### Correct Local Reasoning

- The model looked at the right feature area: search/output handling in ripgrep rather than unrelated infrastructure.
- It also pulled targeted context from `src/worker.rs` and `search_*` code, which is consistent with trying to understand match iteration.

### Tool Friction

- `code_item_lookup` returned a miss for `matched` in `src/printer.rs` and suggested retrying with a different node kind, which did not obviously help.
- The run then spent the rest of the turn in exploratory context fetches and eventually hit a transport timeout during the second chat request ([ploke_eval_20260408_085501_361377.log#L300217](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L300217)).

### Model Mistake

- The model never translated the diagnosis into a concrete minimal patch.
- It appears to have over-invested in exploration after the first lookup miss instead of narrowing to the output path that repeated the first match.

### Artifact Ambiguity

- The local artifacts are clear here: `terminal_record.outcome = aborted`, `patch_artifact.edit_proposals = []`, and `expected_file_changes[0].changed = false`.
- The `multi-swe-bench-submission.jsonl` file exists but contains an empty `fix_patch`, which makes the run look superficially complete unless the report checks the patch content itself.

### Benchmark Follow-Through

- The official Multi-SWE-bench evaluator was not run for this run.
- The artifact set is benchmark-ready in shape, but it does not contain a patch.

## Minimal Correct Fix

Patch the match-printing path that advances between matches so it stops reiterating the first match under `--only-mathing`, rather than reworking worker setup or broader search plumbing.

## Open Questions

- Tool-design questions:
  - Would a tighter `code_item_lookup` retry hint reduce the chance of wandering after a miss?
- semantic editing capability questions:
  - N/A for this run, because no edit was proposed.
- runner or artifact questions:
  - Should the empty `fix_patch` case be surfaced more loudly in the local summary?

## Follow-Up Actions

- instrumentation: add a clearer first-failure marker for aborted turns with no patch.
- tool UX: improve the `code_item_lookup` retry guidance when the first lookup is close but wrong.
- runner artifact changes: surface empty `fix_patch` as an explicit failed patch emission.
- regression tests: add a benchmark-side check that the reported patch artifact is non-empty before treating a run as meaningful.
