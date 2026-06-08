# Changelog

## 2026-06-07

- Traced the failed long-ish Prototype 1 run to successor artifact preparation,
  not provider behavior, protocol adjudication, child admissibility, or
  child-observe timing.
- Added a source trace from historical artifact selection through successor
  installation and the missing broad-harness workspace error.
- Added regression coverage for selecting the active parent from historical
  traversal candidates.
- Added regression coverage for installing a committed broad-harness successor
  artifact after its provisional edit-harness checkout is gone.
- Added a successor-install-only fallback for missing campaign-owned
  broad-harness workspaces that preserves strict child artifact validation.
- Updated the repo-local `prototype1-loop-run-status` skill with successor
  failure checks and installed the read-only run summary helper.
- Abandoned `p1-handofffix-5g1x2-a2-20260607-190702` for loop progress after
  its baseline eval persisted a failed embedding preflight on the default
  `mistralai/codestral-embed-2505` route.
- Fixed the source path that dropped `prototype1-setup` embedding model/provider
  overrides before baseline agent-batch execution.
- Added focused coverage for setup manifest persistence, batch-to-single
  embedding forwarding, setup CLI parsing, and eval-set identity separation.
- Committed the embedding override fix as `75403d53` and created fresh campaign
  `p1-handofffix-embed-5g1x2-a2-20260607-192954` from that source.
- Admitted the same 5-generation, 1-2 child, 2-retry profile with explicit
  setup flags `--embedding-model-id perplexity/pplx-embed-v1-4b` and
  `--embedding-provider perplexity`.
- Verified the fresh campaign manifest stores the embedding overrides under
  `eval.embedding_model_id` and `eval.embedding_provider_slug`.
- Ran `prototype1-doctor` with live protocol and headless TUI setup preflights;
  it reported phase `baseline_eval` and no blockers.
- Started the continuous `prototype1-state` run for the fresh campaign at
  `2026-06-07T19:36:06-07:00`; early baseline indexing used
  `perplexity/pplx-embed-v1-4b`, so the previous missing override failure did
  not recur at embedding preflight.
- Observed generation 0 complete successfully at
  `2026-06-07T20:06:47-07:00`: two children ran, `node-84b583594e201ee2`
  was selected with `Keep`, `node-4f5ce42174cc01d6` was rejected, History
  selection sealed the accepted candidate, active checkout advanced to commit
  `38915de79025e2f8cca9c60984441ee4729990ba`, and successor handoff was
  acknowledged for runtime `1bdf24b7-34ed-46d6-9a15-03113415f5a4`.
- Confirmed the detached successor process started generation 1 from
  `node-84b583594e201ee2` under the same campaign/worktree with
  `--handoff-invocation`, demonstrating the first parent-to-successor handoff
  on the fresh source path.
- Observed generation 1 complete successfully at
  `2026-06-07T20:30:10-07:00`: both generation-2 children completed, the
  selection row included the historical rejected child
  `node-4f5ce42174cc01d6`, current kept child `node-8167e33daa3b9bc6`, and
  current rejected child `node-b56d3539fe294ed3`; stochastic selection chose
  `node-8167e33daa3b9bc6` with disposition `Keep`.
- Confirmed the generation-1 parent spawned successor PID `894411` from
  `node-8167e33daa3b9bc6` using handoff invocation
  `637883b5-2843-45ad-9f63-ba2ec73c2cbf`, demonstrating the second
  parent-to-successor handoff on the fresh source path.
- Observed generation 2 broad generation recover from transient
  `INVALID_MODEL_RESPONSE` warnings and materialize two generation-3 child
  candidates at `2026-06-07T20:41:14-07:00`.
- Confirmed generation 2 built and spawned both generation-3 child runners at
  `2026-06-07T20:43:40-07:00`: `node-7815b0481a271a5e`
  (`branch-feabca86774c57a6`, PID `988607`) and
  `node-d05350cdb42e3185` (`branch-9ded1d142d60298a`, PID `988624`).
- Observed both generation-3 children reject:
  `node-d05350cdb42e3185` / `branch-9ded1d142d60298a` at
  `2026-06-07T20:50:59-07:00`, and `node-7815b0481a271a5e` /
  `branch-feabca86774c57a6` at `2026-06-07T20:55:39-07:00`.
- Confirmed `require_keep_for_continuation = false` plus
  `explore_from_rejected = true` allowed continuation after the all-rejected
  generation: successor selection recorded disposition
  `continue_explore_from_rejected`, selected rejected
  `node-b56d3539fe294ed3` / `branch-afdcff78a2bf6e87`, advanced the active
  checkout, spawned successor PID `1005288`, and acknowledged handoff at
  `2026-06-07T20:56:04-07:00`.
- Confirmed the rejected successor parent continued through normal broad-child
  generation: it materialized children `node-c7ea8d834c87f9f6`
  (`branch-ed47a449ae5a187a`) and `node-1c4fb95839e61fcc`
  (`branch-1fec33b974df7b88`) at `2026-06-07T21:03:13-07:00`, then built
  and spawned both runners at `2026-06-07T21:05:42-07:00`.
- Observed the rejected-parent children complete at
  `2026-06-07T21:17:58-07:00`: `node-1c4fb95839e61fcc` /
  `branch-1fec33b974df7b88` was admitted as `Keep` with performance `15240`,
  while `node-c7ea8d834c87f9f6` / `branch-ed47a449ae5a187a` was rejected
  with performance `8345`.
- Confirmed the next successor handoff completed at
  `2026-06-07T21:18:21-07:00`, but selection did not choose the newest
  highest-weight kept child. The sealed formula row assigned weight
  `0.8360382571135474` to `node-1c4fb95839e61fcc`, but stochastic sample
  `0.20872616581425532` selected the older kept parent
  `node-8167e33daa3b9bc6` with weight `0.19720776365189244`; successor PID
  `1088814` started from handoff invocation
  `6d5ffaef-ad28-4e17-9432-4754fc12e70c`.
