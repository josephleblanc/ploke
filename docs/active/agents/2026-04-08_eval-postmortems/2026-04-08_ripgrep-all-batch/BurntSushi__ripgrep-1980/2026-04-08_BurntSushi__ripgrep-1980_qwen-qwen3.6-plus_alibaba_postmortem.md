# Eval Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-1980 postmortem
- batch id: `ripgrep-all`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`

## Header

- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-1980`
- instance: `BurntSushi__ripgrep-1980`
- repository: `BurntSushi/ripgrep`
- base sha: `9b01a8f9ae53ebcd05c27ec21843758c2c1e823f`
- stable evidence source: `run.json`, `execution-log.json`, `agent-turn-summary.json`, `agent-turn-trace.json`, `multi-swe-bench-submission.jsonl`, live batch log
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-1980/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not run

## Outcome Snapshot

- final runner state: batch step completed, run artifacts written
- final chat outcome: aborted after 22 attempts
- primary user-visible failure: the assistant never converged to a patch and aborted after repeated lookup/context failures
- did the model produce a patch: no
- did the target file change: no, all expected file hashes were unchanged
- official benchmark status: not run
- official benchmark evidence: none

## Failure Classification

- primary category: `tool-affordance-gap`
- secondary category: `tool-retry-friction`
- confidence: high

## Timeline

1. Initial diagnosis: the model understood the high-level task, namely per-pattern `smart-case` handling and HIR alternation, and looked at the regex/configuration area.
2. First meaningful tool failure: it repeatedly tried to read `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/core/Cargo.toml`, which does not exist in this repo layout ([ploke_eval_20260408_085501_361377.log#L21126](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L21126)).
3. First edit proposal: none.
4. First compile or test failure: none.
5. End-of-run state: the session aborted with `TRANSPORT_TIMEOUT`, and the submission JSONL contains an empty `fix_patch`.

## Evidence

### Correct Local Reasoning

- The model stayed in the right general domain by inspecting regex and HIR-related snippets.
- It asked for context around alternation and smart-case behavior, which matches the problem statement.

### Tool Friction

- The run spent a lot of time on nonexistent or wrong-path reads, then on `code_item_lookup` misses such as `hyperlinks` in `crates/core/args.rs` with an incorrect `node_kind` guess ([ploke_eval_20260408_085501_361377.log#L55631](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L55631)).
- A later `request_code_context` failed because the embeddings backend had no successful provider responses, which blocked more context assembly ([ploke_eval_20260408_085501_361377.log#L55678](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log#L55678)).

### Model Mistake

- The model never turned the diagnosis into a concrete edit.
- It drifted into unrelated paths and lookup retries instead of narrowing to the regex construction path that builds per-pattern HIRs.

### Artifact Ambiguity

- The summary artifact is consistent but unhelpful here: `terminal_record.outcome = aborted`, no assistant message is preserved, and `patch_artifact.edit_proposals = []`.
- The submission artifact exists but is empty, so this run is easy to misread as a valid candidate unless the patch payload is checked.

### Benchmark Follow-Through

- The official Multi-SWE-bench evaluator was not run for this run.
- The local artifact set is benchmark-shaped, but there is no actual fix to evaluate.

## Minimal Correct Fix

Implement the per-pattern `smart-case` handling in the regex/HIR construction path and stop chasing unrelated `Cargo.toml` or `args.rs` items once the lookup path stops matching.

## Open Questions

- Tool-design questions:
  - Can we reduce wrong-path reads by surfacing repo-layout constraints earlier?
- semantic editing capability questions:
  - N/A for this run, because no edit was proposed.
- runner or artifact questions:
  - Should empty submissions be flagged explicitly instead of silently producing `fix_patch: ""`?

## Follow-Up Actions

- instrumentation: add a run-level marker for repeated lookup misses on nonexistent paths.
- tool UX: make `code_item_lookup` retries more specific when the first guess is a layout mismatch.
- runner artifact changes: surface empty `fix_patch` as a failed patch emission.
- regression tests: add a guard that empty submission artifacts are treated as failures in local reporting.
