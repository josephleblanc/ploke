# BurntSushi ripgrep-2610 Postmortem

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2610 Eval Postmortem
- task description: Document the completed eval run for instance `BurntSushi__ripgrep-2610`, with emphasis on assistant coherence, tool friction, and artifact ambiguity.
- related planning files:
  - [2026-04-08_postmortem-template.md](../../2026-04-08_postmortem-template.md)
  - [2026-04-08_batch-postmortem-index.md](../2026-04-08_batch-postmortem-index.md)

## Header

- batch id: `2026-04-08_ripgrep-all-batch`
- run id: `BurntSushi__ripgrep-2610`
- instance: `BurntSushi__ripgrep-2610`
- model: `minimax/minimax-m2.5`
- provider: `friendli` (from the stable eval log envelope)
- repository: `BurntSushi/ripgrep`
- base sha: not surfaced in the stable turn summary
- stable evidence source: [ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)
- artifact paths:
  - run manifest: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/run.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/run.json)
  - execution log: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/execution-log.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/execution-log.json)
  - turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json)
  - turn trace: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-trace.json)
  - submission jsonl: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/multi-swe-bench-submission.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/multi-swe-bench-submission.jsonl)
  - official benchmark logs/report: [/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)

## Outcome Snapshot

- final runner state: completed
- final chat outcome: `Request summary: [success]`
- primary user-visible failure: none clearly evidenced in the stable artifacts; the run mainly showed churn and tool noise
- did the model produce a patch: yes
- did the target file change: yes
- official benchmark status: success in the stable turn summary
- official benchmark evidence: `agent-turn-summary.json` reports `outcome=completed` and `summary=Request summary: [success]`

## Failure Classification

- primary category: `tool-retry-friction`
- secondary category: `artifact-ambiguity`
- confidence: medium

## Timeline

1. Initial diagnosis:
   The assistant stayed mostly coherent and worked the right problem space for the benchmark task.
2. First meaningful tool failure:
   `code_item_lookup` repeatedly missed the `clap_matches` symbol in `crates/core/args.rs`, with recovery hints that alternated between `function` and `method` instead of resolving the lookup.
3. First edit proposal:
   The run moved into edits across `crates/core/app.rs`, `crates/printer/src/lib.rs`, and `crates/printer/src/standard.rs`, rather than converging on a single minimal change.
4. First compile or test failure:
   The stable summary does not show a terminal compile/test failure; the main issue is repeated tool/patch churn.
5. End-of-run state:
   The summary says success, but the trace file looks stale/partial and should not be treated as the completed-run source.

## Evidence

### Correct Local Reasoning

The assistant did not look lost. It consistently explored the ripgrep repo, focused on printer/core/searcher code, and continued iterating toward the requested feature.

Evidence:

- stable run summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json)
- matching eval log: [/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)

### Tool Friction

The clearest friction was repeated lookup failure around `clap_matches`, followed by patch churn. The summary records one failed edit proposal and many repeated applied edits to the same small set of files.

Evidence:

- `code_item_lookup` failure for `clap_matches` in the eval log: [ploke_eval_20260408_063814_328460.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_063814_328460.log)
- failed edit proposal in the turn summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json)
- repeated applied edits to `crates/core/app.rs` and `crates/printer/src/standard.rs`: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json)

### Model Mistake

This run does not show a strong reasoning collapse. The weaker point was over-iterating through tool failures instead of locking onto the correct symbol and converging on a minimal edit path.

### Artifact Ambiguity

The artifacts are not fully consistent. The stable summary says the run completed successfully, but `agent-turn-trace.json` looks stale/partial and the run directory may have been reused by newer activity.

Evidence:

- stable summary: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-summary.json)
- stale/partial trace artifact: [/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-trace.json](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2610/agent-turn-trace.json)

### Benchmark Follow-Through

The stable run summary reports success, but the trace file is not trustworthy enough to use as a completed-run record. For postmortem purposes, the summary plus the eval log are the reliable sources.

## Minimal Correct Fix

No product fix is clearly implied by the stable artifacts alone. The minimal process fix would have been to stop the lookup/edit churn earlier, anchor on the correct `clap_matches` target, and avoid treating the stale trace as authoritative.

## Open Questions

- Tool-design questions:
  - Should `code_item_lookup` recovery hints point more directly to the defining file when lookup keeps missing the same symbol?
- semantic editing capability questions:
  - Were the repeated edits necessary, or did the assistant just need a more direct target selection path?
- runner or artifact questions:
  - Should completed runs get immutable per-instance artifact copies so later activity cannot blur the trace state?

## Follow-Up Actions

- instrumentation:
  - Preserve one immutable completed-run trace per instance.
- tool UX:
  - Tighten lookup recovery so repeated misses do not oscillate between `function` and `method`.
- runner artifact changes:
  - Make summary and trace freshness explicit in the reportable metadata.
- regression tests:
  - Add a replay case that exercises the repeated lookup-and-patch pattern without relying on a mutable run directory.
