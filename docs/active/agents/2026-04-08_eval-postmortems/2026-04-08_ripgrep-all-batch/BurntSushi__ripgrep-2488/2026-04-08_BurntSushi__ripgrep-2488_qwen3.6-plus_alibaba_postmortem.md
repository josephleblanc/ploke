# BurntSushi ripgrep-2488 Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2488 Eval Postmortem
- task description: Postmortem for the `BurntSushi__ripgrep-2488` ripgrep-all batch run, with emphasis on tool friction, model coherence, and artifact trust.
- related planning files:
  - [2026-04-08_postmortem-template.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_batch-postmortem-meta-notes.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_batch-postmortem-meta-notes.md)
  - [2026-04-08_postmortem-template-review.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_ripgrep-all-batch/2026-04-08_postmortem-template-review.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-2488`
- instance: `BurntSushi__ripgrep-2488`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `041544853c86dde91c49983e5ddd0aa799bd2831`
- stable evidence source: [ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not present in the local run artifacts

## Outcome Snapshot

- final runner state: completed
- final chat outcome: `Request summary: [success]`
- primary user-visible failure: repeated lookup and patch-format friction around `matcher_rust`, `join_patterns`, and malformed `non_semantic_patch` payloads before the run converged
- did the model produce a patch: yes
- did the target file change: yes, `crates/core/args.rs`
- official benchmark status: no downstream evaluator verdict was captured locally
- official benchmark evidence: only the local submission JSONL is present

## Artifact Trust

- authoritative artifact: `agent-turn-summary.json` for proposal/state inventory, paired with `agent-turn-trace.json` for the step-by-step chronology
- partial or stale artifacts: the stable log is noisy and the trace does not carry the full final assistant message
- why: the summary shows the patch inventory and file hashes, while the trace preserves the lookup failures and retry path that led there

## Failure Classification

- primary category: `tool-retry-friction`
- secondary category: `artifact-ambiguity`
- confidence: medium
- report confidence limits: no downstream benchmark verdict was available, so this report only covers the local run and submission artifact

## Timeline

1. Initial diagnosis:
   The assistant correctly identified the issue as pattern-joining behavior in `crates/core/args.rs`, not a search-engine bug.
2. First meaningful tool failure:
   `code_item_lookup` on `matcher_rust` in `crates/core/args.rs` returned `InvalidFormat`, and later `non_semantic_patch` rejected a 3-patch payload. See [trace line 739](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-trace.json#L739) and [trace line 1429](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/agent-turn-trace.json#L1429).
3. First edit proposal:
   The assistant converged on a `join_patterns` helper that wraps each pattern in `(?:...)` and preserves a single-pattern fast path.
4. First compile or test failure:
   No hard compile/test failure is visible in the stable artifacts; the run recovered before any terminal validation failure.
5. End-of-run state:
   The runner completed and wrote the submission JSONL, but `CHANGELOG.md` remained unchanged even though it was listed in `expected_file_changes`.

## Evidence

### Correct Local Reasoning

The model understood the intended fix: keep multiple patterns from affecting each other by isolating them with non-capturing groups, while preserving the single-pattern case. The final trace shows `join_patterns` implemented that way.

### Tool Friction

The most visible friction was the lookup/mode mismatch around `matcher_rust`, followed by a malformed `non_semantic_patch` payload and an invalid canonical target for `join_patterns`. Those errors pushed the run through several recovery steps before it converged.

### Model Mistake

The mistake was not the bug target, but the path there: the assistant over-relied on tool retries and helper extraction instead of settling earlier on the minimal edit in `crates/core/args.rs`.

### Artifact Ambiguity

`agent-turn-summary.json` says `applied: true` but `all_proposals_applied: false`, and `expected_file_changes` reports that `CHANGELOG.md` did not change. The final assistant message is also absent from the summary artifact.

### Benchmark Follow-Through

The local run produced [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2488/multi-swe-bench-submission.jsonl), but there is no downstream benchmark verdict in the local artifacts.

## Minimal Correct Fix

Wrap each joined pattern in a non-capturing group in `crates/core/args.rs`, and keep the single-pattern fast path so existing one-pattern behavior does not change.

## Open Questions

- Tool-design questions:
  - Should `code_item_lookup` give a stronger disambiguation path when the same file/module/item tuple has multiple matches?
  - Should malformed `non_semantic_patch` payloads include an example of the expected `patches` array shape?
- semantic editing capability questions:
  - Should there be a clearer way to express "edit this existing helper in place" without falling into a helper-extraction detour?
- runner or artifact questions:
  - Should the turn summary retain the final assistant message content and the changed-file set explicitly?

## Follow-Up Actions

- instrumentation:
  - Record the first tool failure, first recovery step, and first validation failure as separate reportable fields.
- tool UX:
  - Improve `code_item_lookup` disambiguation and patch-format feedback.
- runner artifact changes:
  - Persist the final assistant message and the changed-file inventory in the summary artifact.
- regression tests:
  - Add a replay case for this join-pattern regression so future tool changes can be compared against it.
