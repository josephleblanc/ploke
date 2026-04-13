# BurntSushi ripgrep-2295 Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2295 Eval Postmortem
- task description: Postmortem for the `BurntSushi__ripgrep-2295` ripgrep-all batch run, with emphasis on provider behavior, tool friction, and artifact trust.
- related planning files:
  - [2026-04-08_postmortem-template.md](../../2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](../2026-04-08_batch-postmortem-index.md)
  - [2026-04-08_batch-postmortem-meta-notes.md](../2026-04-08_batch-postmortem-meta-notes.md)
  - [2026-04-08_postmortem-template-review.md](../2026-04-08_postmortem-template-review.md)

## Header

- batch id: `ripgrep-all`
- batch manifest: [/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json](/home/brasides/.ploke-eval/batches/ripgrep-all/batch.json)
- run id: `BurntSushi__ripgrep-2295`
- instance: `BurntSushi__ripgrep-2295`
- model: `qwen/qwen3.6-plus`
- provider: `alibaba`
- repository: `BurntSushi/ripgrep`
- base sha: `1d35859861fa4710cee94cf0e0b2e114b152b946`
- stable evidence source: [ploke_eval_20260408_085501_361377.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_085501_361377.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: not present in the local run artifacts

## Outcome Snapshot

- final runner state: completed artifact write, but the terminal record is `aborted`
- final chat outcome: `Request summary: [aborted] error_id=77f89746-a52f-4155-9faf-3b2af04f331e`
- primary user-visible failure: repeated lookup churn around `next` and `DirEntryBuilder`, then an upstream Alibaba rate-limit/body decode failure aborted the session before any patch landed
- did the model produce a patch: no
- did the target file change: no
- official benchmark status: no downstream evaluator verdict was captured locally
- official benchmark evidence: only the local run artifacts exist, and they show no applied proposal

## Artifact Trust

- authoritative artifact: `agent-turn-trace.json` for the terminal aborted state, paired with the batch log for the upstream provider error text
- partial or stale artifacts: `agent-turn-summary.json` has no proposals and no final assistant content, so it is not a patch-evidence source
- why: the trace shows the end state clearly, while the log shows the provider-side failure that ended the session

## Failure Classification

- primary category: `provider-behavior`
- secondary category: `tool-retry-friction`
- confidence: medium-high
- report confidence limits: the abort came from an upstream provider error, so the report can only characterize the local pre-abort search behavior and the terminal failure mode

## Timeline

1. Initial diagnosis:
   The assistant followed the intended ignore-path bug in `crates/ignore/src/dir.rs` and `crates/ignore/src/walk.rs`.
2. First meaningful tool failure:
   `code_item_lookup` on `next` in `crates/ignore/src/walk.rs` returned `InvalidFormat` because multiple items matched, and a later lookup on `DirEntryBuilder` also failed. See [trace line 893](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/agent-turn-trace.json#L893) and the later `DirEntryBuilder` failure in the same trace.
3. First edit proposal:
   No durable edit proposal landed; the session never reached a stable patch state.
4. First compile or test failure:
   No compile or test failure surfaced before the session aborted.
5. End-of-run state:
   The log shows an Alibaba upstream rate-limit message, and the trace ends with an aborted terminal record after 37 attempts.

## Evidence

### Correct Local Reasoning

The model stayed in the correct area: it read `dir.rs`, inspected `walk.rs`, and reasoned about duplicate path construction in subdirectory ignore handling. That matches the benchmark prompt.

### Tool Friction

The first lookup failure was not a semantic misunderstanding; it was a tool invariant problem. `code_item_lookup` rejected `next` as ambiguous, then later rejected `DirEntryBuilder` as nonexistent. Those errors consumed several attempts before the provider failure ended the session.

### Model Mistake

The model did not get far enough to show a substantive implementation mistake. The main problem was spending time in the lookup/retry loop while the session was becoming unstable.

### Artifact Ambiguity

The summary artifact is empty of proposals, while the trace shows an aborted terminal record and the batch log shows the provider-side failure text. That makes the trace and log authoritative here, not the summary.

### Benchmark Follow-Through

The run wrote [multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2295/multi-swe-bench-submission.jsonl) only as an artifact shell; there is no patch inside it because no proposal landed before the abort.

## Minimal Correct Fix

Adjust the ignore-path logic in `crates/ignore/src/dir.rs` so subdirectory matching does not duplicate path components, and keep the optimization to skip the second loop only when a match is already known.

## Open Questions

- Tool-design questions:
  - Should repeated `code_item_lookup` misses trigger an earlier, clearer file-level fallback?
- semantic editing capability questions:
  - Is there a better way to inspect and edit the ignore traversal code without falling into adjacent-symbol lookups?
- runner or artifact questions:
  - Should provider-side aborts be surfaced as first-class fields in the run summary instead of only appearing in the log?

## Follow-Up Actions

- instrumentation:
  - Record provider-side abort reasons and attempt counts in the run summary.
- tool UX:
  - Improve repeated-miss guidance for `code_item_lookup`.
- runner artifact changes:
  - Persist a compact abort reason in the summary artifact.
- regression tests:
  - Add a replay case for provider-aborted runs so they can be distinguished from model failures.
