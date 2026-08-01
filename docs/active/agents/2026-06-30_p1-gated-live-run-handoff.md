# 2026-06-30 P1 gated live-run handoff

**Date:** 2026-06-30  
**Status:** restart handoff after stopping the live run  
**Branch:** `feature/ploke-loop`  
**Primary campaign:** `p1-gated-parent-3g1x3-p3-20260630-174316`  
**Related packet:** [`2026-06-30_p1-db-filesystem-parity-live-run-template/`](2026-06-30_p1-db-filesystem-parity-live-run-template/)

## Why this handoff exists

We stopped after a live Prototype 1 gated parent/successor run produced useful normalized DB evidence, reached a generation-0 successor handoff, and then failed in generation 1 because two live drivers wrote/attempted to write the same child-plan surface. The repository is otherwise clean as of the commits listed below.

This document is the cold-restart spine for picking up the DB/filesystem parity, walk-server handoff, and controller-ownership investigation later.

## Commits made in this session

Latest commits at handoff time:

```text
2c3a7cfda docs: record P1 gated live run evidence
9d69e2e38 docs: add runnerio DB-only trajectory review
0f4c02234 Fix headless validation retry flow
aa4c369e5 docs: add P1 live loop parity worksheets
a8b21da4b Gate parent benchmark tool loop
```

Session commit meanings:

- `0f4c02234 Fix headless validation retry flow`
  - Code commit.
  - Moves applied-batch validation handling to the intended retry/terminal path instead of validating after each applied edit batch.
  - Adds retry handling for validation failures and additional broad-harness / headless-TUI trace instrumentation.
  - GitNexus staged check reported **HIGH** risk because it touches the live headless harness flow.
- `9d69e2e38 docs: add runnerio DB-only trajectory review`
  - Adds [`2026-06-27_p1-runnerio-db-db-only-trajectory-qna.md`](2026-06-27_p1-runnerio-db-db-only-trajectory-qna.md).
  - Separates DB-only answerability from file-only facts for an older runner-IO campaign.
- `2c3a7cfda docs: record P1 gated live run evidence`
  - Adds the live-run query/response evidence packet under [`2026-06-30_p1-db-filesystem-parity-live-run-template/`](2026-06-30_p1-db-filesystem-parity-live-run-template/).
  - Includes query files, raw/summarized responses, walk snapshots, poll snapshots, failure inspection, cleanup evidence, and worksheet updates.

No tests were run while making the last two docs commits. Focused validation tests for the code work had passed earlier in the broader session, but do not treat this handoff as fresh test evidence.

## Core docs and artifacts to read first

Start here:

1. [`2026-06-30_p1-db-filesystem-parity-live-run-template/answers-ledger.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/answers-ledger.md)
   - Best compact index of the run, step-by-step evidence, and terminal finding.
2. [`2026-06-30_p1-db-filesystem-parity-live-run-template/responses/child-plan-03__generation2-plan-drift__terminal.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/child-plan-03__generation2-plan-drift__terminal.md)
   - Best single artifact for the terminal DB/file parity failure.
3. [`2026-06-30_p1-db-filesystem-parity-live-run-template/responses/terminal-cleanup__gen1-controller-collision.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/terminal-cleanup__gen1-controller-collision.md)
   - Shows the live successor controller and child runner that were stopped.
4. [`2026-06-30_p1-db-filesystem-parity-live-run-template/responses/final-status__terminal-gen1-drift.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/final-status__terminal-gen1-drift.md)
   - Final process/status/count snapshot after cleanup.
5. [`2026-06-30_p1-db-filesystem-parity-live-run-template/typestate-db-schema-ledger.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/typestate-db-schema-ledger.md)
   - Schema/phase ledger plus terminal run notes.
6. [`2026-06-30_p1-db-filesystem-parity-live-run-template/live-loop-functionality-questions.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/live-loop-functionality-questions.md)
   and [`self-improvement-loop-questions.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/self-improvement-loop-questions.md)
   - Contain partial run answers, not a complete question-by-question pass.

Important caveat: the response/helpfulness/schema notes were **not** written consistently during the run. Do not treat the question docs as fully answered. The user explicitly wanted these notes written as the run progressed; backfilling them later would mix later knowledge into earlier-step interpretation.

## Live campaign trajectory

Campaign:

```text
p1-gated-parent-3g1x3-p3-20260630-174316
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316
```

Committed-code binary used for the initial run:

```text
/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
sha256: 51cd1a0696bd16024452360bff0f190a08333616711b8ae50365c73a83d09b13
```

Generation 0 path:

1. R0→R7 established parent identity, policy, and baseline.
2. R7→R8 broad harness ran under gated tool-loop policy and produced child-plan evidence.
3. R10→R11 initially timed out at the child-result boundary for one child.
4. A second R10→R11 recovered the missing branch evaluation from terminal channel evidence without respawning children.
5. R11→R12 projected report facts.
6. R12 plain step correctly refused because selected-successor evidence required the handoff gate.
7. R12→R13b with `--allow git-changes` succeeded and installed/spawned successor `node-2fe75acd9e9cf6c3`.

Key generation-0 successor evidence:

- Query: [`queries/continuation-01__selection-handoff.cozo`](2026-06-30_p1-db-filesystem-parity-live-run-template/queries/continuation-01__selection-handoff.cozo)
- Response: [`responses/continuation-01__selection-handoff__step-15.json`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/continuation-01__selection-handoff__step-15.json)
- Continuation was `continue_explore_from_rejected`, selected node `node-2fe75acd9e9cf6c3`, branch `branch-88aaa7a4b0328baf`.

## Failure mode to investigate

After R13b handoff, R13→R14 failed with:

```text
stale walk server: walk server binary differs from client binary; restart the walk server
```

The walk server was stopped/restarted. The restarted walk server reconstructed the active checkout as successor generation 1 at R7. However, R13b handoff had already spawned a full successor controller process:

```text
ploke-eval loop prototype1-state \
  --campaign p1-gated-parent-3g1x3-p3-20260630-174316 \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316 \
  --handoff-invocation .../node-2fe75acd9e9cf6c3/invocations/a96b5766-b5e0-43a8-99d8-b81936cb5449.json \
  --stop-after complete \
  --format json
```

Then a manual `walk step` was issued for gen1 R7→R8 while that successor `prototype1-state` process was still active. This created two live drivers for the same generation-1 parent:

- the autonomous successor `prototype1-state --stop-after complete` process; and
- the manually restarted walk server/client driving gen1 R7→R8.

Resulting terminal evidence:

- DB row: child-plan id `9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04`
  - DB `message_sha256`: `be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499`
  - DB child: `node-e9be81e08bda07a1`
- Current child-plan file at same `message_path`:
  - file sha256: `361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f`
  - file child: `node-5a626eeb21d0c85a`

The eval-store correctly rejected same-key/different-hash child-plan persistence. Do **not** loosen this invariant.

## Current live-process state

At cleanup time, the successor controller, child runner, and walk server were terminated. Final evidence:

- [`responses/terminal-cleanup__gen1-controller-collision.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/terminal-cleanup__gen1-controller-collision.md)
- [`responses/final-status__terminal-gen1-drift.md`](2026-06-30_p1-db-filesystem-parity-live-run-template/responses/final-status__terminal-gen1-drift.md)

Final `walk status` in that artifact reports offline. Do not resume this campaign as a clean success run; use it as failure evidence.

## Design interpretation

The most likely design problem is not the DB validation. The validation worked.

The open design question is the intended ownership model after R13b:

- If R13b handoff is supposed to terminate/restart the walk server under successor code, the server did not make that lifecycle obvious to the operator.
- If R13b handoff intentionally spawns a full successor `prototype1-state --stop-after complete`, then the operator must not manually walk the same successor generation. The walk/status surface should make that explicit.
- If the desired UX is step-by-step successor walking, then R13b should not simultaneously spawn an autonomous full-loop successor, or it should spawn it in a mode that hands control back to the walk server safely.

## Recommended next work

1. **Clarify intended R13b lifecycle in docs and code.**
   - Should the old walk server exit automatically?
   - Should a new walk server start under the successor binary?
   - Should R13b spawn an autonomous `prototype1-state --stop-after complete`, or only a ready-check successor?
2. **Add first-class controller/session ownership.**
   - Track active controller PID, parent node, generation, stop-after mode, lease/owner role, spawned child/successor PIDs, and cleanup/exit status.
   - Make `walk status` show when a successor controller is authoritative.
3. **Add duplicate-driver guard.**
   - Before `walk step` drives a generation, check whether a successor `prototype1-state` controller is already active for the same campaign/node/generation.
   - Refuse unless an explicit takeover/recovery gate is passed.
4. **Preserve strict child-plan hash validation.**
   - Add recovery/quarantine tooling for drift rather than permissive import.
5. **Add DB/file parity helper views or CLI helpers.**
   - For file-backed rows with path + sha256, report current hash, DB hash, and drift status without ad hoc shell probes.
6. **Improve in-flight trace persistence.**
   - `eval_model_exchange` stayed zero despite provider activity. Mid-turn/provider facts are still not DB-primary.
7. **Do not retroactively complete the questionnaire from memory.**
   - The question docs contain useful partial answers, but the per-response helpfulness/schema notes were not written at the correct time.

## If resuming live work

Do not resume the stopped campaign as proof of clean loop completion. Prefer a fresh committed-code campaign after deciding the R13b ownership semantics.

Before starting another live run:

1. Rebuild committed-code binary.
2. Verify broad harness and parent benchmark are still `ToolLoopMode::Gated`.
3. Use `/home/brasides/.ploke-eval/worktrees/<campaign>` for live worktrees.
4. Submit at most one mutating live step at a time.
5. During successor handoff, observe spawned process ownership before issuing any manual `walk step` against the successor.
6. Keep DB-only and file-only claims separate.

## Related open technical threads

- Dense/mock post-apply freshness and consecutive same-file edit repro are still unresolved.
- BM25 strictness/search strategy must not be weakened.
- `eval_harness_submission*` remained zero in this run despite broad harness activity; determine whether this is expected for the current admission surface or a mirror gap.
- Query ergonomics remain rough: count queries require relation-specific key knowledge, and row-printer queries were often used because there are no documented derived views yet.
