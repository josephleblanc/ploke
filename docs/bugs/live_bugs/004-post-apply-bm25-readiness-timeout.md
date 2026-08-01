# Bug report

2026-06-29

## Issue: post-apply harness waits report BM25 readiness timeout while BM25 is not the slow component

### Status

Alive / open.

A timeout workaround was committed in `9dec3fcb0` (`POST_APPLY_INDEX_TIMEOUT_SECS: 180 -> 600`), but the root cause is not fixed.

### Short version

The failing broad-harness run did not show BM25 being intrinsically slow. Initial BM25 startup rebuild completed normally and was already ready with 7282 docs. The timeout happened after broad-harness applied edits to a large `syn_parser` module while the TUI freshness pipeline was doing parse/retract/reindex work and the embedding indexer was still processing batches.

The most likely root cause is a freshness-orchestration/backpressure bug:

- `wait_for_refresh` treats "BM25 ready" as the post-apply freshness gate for sparse/strict retrieval.
- `RagService::bm25_rebuild()` only awaits enqueueing `Bm25Cmd::Rebuild`; it does not acknowledge rebuild start or completion.
- The same BM25 actor mailbox also receives `IndexBatch` messages from the embedding indexer.
- If the mailbox/actor is backpressured, the rebuild command may not start before the post-apply deadline. In the failed log there is no post-apply `BM25 Rebuild: starting rebuild from database` line before timeout.
- The resulting error is labeled as BM25 readiness even though the observable work is scan/reindex/backlog coordination.

### Evidence

Failing campaign:

- `p1-harnesssubmit-db2-20260628-213744`
- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-harnesssubmit2-src-41cdad18`
- Diagnostic file:
  - `/home/brasides/.ploke-eval/campaigns/p1-harnesssubmit-db2-20260628-213744/prototype1/messages/edit-harness-result/node-f4d740eb7e97e066.headless-tui.json`
- Log file:
  - `/home/brasides/.ploke-eval/logs/ploke_eval_20260628_143758_29435.log`

Initial prompt diagnostic showed BM25 already ready:

```json
"bm25": {
  "status": "ready",
  "docs": 7282,
  "error": null
}
```

Initial BM25 rebuild was normal:

```text
15:11:35 BM25 Rebuild: starting rebuild from database
15:11:45 BM25 Rebuild: completed successfully with 7282 docs
```

The failure happened after two applied non-semantic patches to:

```text
crates/ingest/syn_parser/src/parser/visitor/mod.rs
```

Key log sequence:

```text
15:12:51 ns_patch proposal apply completed ... visitor/mod.rs ... elapsed_ms=18
15:12:51 scan_for_change in crate_name: syn_parser
15:12:52 run_parse_no_transform ... Parse: run the parser on .../crates/ingest/syn_parser
15:13:10 Retracted 24 stale descendants before partial graph update
15:13:48 cargo_command_finished ... exit_code=101
15:13:48 second ns_patch proposal apply completed ... visitor/mod.rs ... elapsed_ms=23
15:13:50 scan_for_change in crate_name: syn_parser
15:13:51 run_parse_no_transform ... Parse: run the parser on .../crates/ingest/syn_parser
15:17:15 timed out waiting for BM25 readiness after applying proposal batch after 180s
```

During and after this window the embedding/indexer continued processing 32-node batches. At the timeout it was still logging `Indexer::run ... process_batch` and `Ticking with time`. There was no post-apply `BM25 Rebuild: starting rebuild from database` log before the timeout.

### Refined root-cause hypothesis

The immediate failure is probably not "BM25 rebuild took >180s." The more precise hypothesis is:

1. Broad-harness approves a staged edit.
2. `wait_for_selected` sees the proposal status become `Applied`.
3. `wait_for_refresh` sends `StateCommand::ScanPathsForChange` for the changed path.
4. The TUI app runs `scan_for_change_target`, which reparses the changed crate and queues reindex work.
5. For sparse/strict retrieval, `wait_for_sparse_search_refresh` calls `rag.bm25_rebuild().await` and then polls `bm25_status_with_timeout` until ready.
6. `RagService::bm25_rebuild()` only sends `Bm25Cmd::Rebuild` to the BM25 actor mailbox. It does not wait for the actor to start or finish rebuilding, and the send itself is not bounded by the post-apply deadline.
7. The embedding indexer can also send `Bm25Cmd::IndexBatch` into the same actor mailbox.
8. Under actor/mailbox backpressure, `Rebuild` and/or `Status` can sit behind queued work. If `bm25_rebuild().await` spends most or all of the post-apply budget waiting to enqueue, `wait_for_sparse_search_refresh` can later report the generic readiness timeout without any post-apply BM25 rebuild log.

Relevant code:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`
  - `wait_for_refresh`
  - `wait_for_sparse_search_refresh`
- `crates/ploke-tui/src/app_state/database.rs`
  - `scan_for_change_target`
  - `scan_paths_for_change`
- `crates/ploke-rag/src/core/mod.rs`
  - `RagService::bm25_rebuild` sends `Bm25Cmd::Rebuild` but does not wait for start/completion
  - `RagService::bm25_status_with_timeout`
- `crates/ingest/ploke-embed/src/indexer/mod.rs`
  - sends `Bm25Cmd::IndexBatch` for each embedding batch
- `crates/ploke-db/src/bm25_index/bm25_service.rs`
  - single BM25 actor processes `IndexBatch`, `Rebuild`, `Status`

### Why this may be surfacing now

This does not look like a recent DB-persistence code change directly modified BM25 or the core reindexer. Git history for the relevant files points to older freshness/indexing work (`Wait for BM25 rebuild during TUI setup`, `Fix post-apply refresh replay`, etc.), plus the later timeout-only workaround in `9dec3fcb0`.

More likely, recent work changed the workload and execution path enough to expose a latent bug:

1. **Broad-harness path is active again.** Runs using the older deterministic-tools edit target would not exercise live headless TUI tool use, proposal approval, post-apply scan, and sparse refresh in the same way. The failing profile uses:

   ```toml
   [generation]
   source = "broad-harness-request"
   ```

2. **The live model chose a very expensive edit target.** The failed run edited `syn_parser/src/parser/visitor/mod.rs`, a large module root. Nearby files include `code_visitor.rs` (~135 KB) and `code_visitor_syn1.rs` (~131 KB). Changing module declarations in `visitor/mod.rs` caused a full `syn_parser` scan/parse and stale descendant retraction.

3. **The same request applied twice.** The first applied patch failed validation (`cargo check -p ploke-eval` returned exit 101), then the model produced a second applied patch to the same file. That doubled the scan/reindex pressure in one headless session.

4. **Fresh committed-code worktrees amplify cold-index effects.** The proof runs intentionally use fresh/current committed-code worktrees. That is correct for authority, but it can leave the embedding/indexer doing substantial background work while broad-harness edits are already running.

5. **The codebase has grown during DB-persistence work.** The DB-persistence changes did not directly touch BM25, but they added Rust code/tests/docs. The failing log shows the indexer processing snippets from `ploke-eval`, `xtask`, tests, and other workspace areas after the `syn_parser` edits. That suggests there was workspace-wide indexing backlog, not just the changed crate.

6. **Older broad-harness successes mostly changed smaller or different files.** Examples:
   - `p1-broad-db-20260627-111555`: changed `crates/ploke-egui/src/ui/view/projection.rs`.
   - `p1-policyharness-db-20260628-032903`: changed `crates/ingest/ploke-embed/src/local/mod.rs`.
   - `p1-dbdual-broad3g1x3...`: many runs changed `ploke-rag`, `ploke-core`, `ploke-tui`, etc.; some timed out before tool activity or hit a separate scan-barrier failure, but did not hit this exact BM25-labeled timeout.

So this is probably a latent orchestration/backpressure bug exposed by the combination of broad-harness + fresh worktree + large `syn_parser` edit + validation retry + active indexer backlog.

### Existing and new reproduction coverage

Existing useful test:

- `sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion`
  - Location: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tests.rs`
  - This shows that a simple post-apply sparse refresh can return on BM25 without waiting for dense indexing completion. That means "wait_for_refresh always waits for dense indexing" is **not** the direct bug.

New ignored repro test:

- `bm25_rebuild_backpressure_repro_blocks_before_ack`
  - Location: `crates/ploke-rag/src/core/unit_tests.rs`
  - This constructs a `RagService` with a full BM25 mailbox and a live receiver that does not drain. It then asserts the desired behavior: `bm25_rebuild()` should not be able to spend the whole post-apply budget before the rebuild is queued/acknowledged.
  - Current behavior: the ignored test fails because `bm25_rebuild()` blocks on the full actor mailbox.

Run explicitly with:

```bash
cargo test -p ploke-rag bm25_rebuild_backpressure_repro_blocks_before_ack --lib -- --ignored
```

This is intentionally a minimal internal reproduction, not a full live-LLM reproduction. Live API calls are not needed to validate the suspected queue/backpressure mechanism; live LLM only made the system choose a costly edit target.

### Workaround observed

Increasing `POST_APPLY_INDEX_TIMEOUT_SECS` from 180s to 600s allowed later broad-harness attempts to progress further:

- Commit: `9dec3fcb0 Increase headless post-apply index wait`
- Later campaign: `p1-harnesssubmit-db3-20260628-222009`

Important caveat: the later successful terminal result edited `crates/ploke-selection-score/src/papers/raser.rs`, not the same `syn_parser` file. So db3 supports "the broad-harness timeout budget was too tight for some live runs," but it does **not** prove the exact `syn_parser` workload would pass with 600s.

### Recommended next steps

- Add explicit instrumentation for:
  - scan barrier start/end
  - reindex queued/start/end
  - BM25 rebuild enqueue/start/end
  - BM25 status wait enqueue/start/end
  - BM25 actor mailbox depth or send latency
  - embedding indexer queue depth or batch count
- Rename or split the timeout error so it distinguishes:
  - BM25 rebuild enqueue timeout
  - BM25 actor/status timeout
  - post-apply scan timeout
  - dense embedding/reindex backlog timeout
- Replace `bm25_rebuild()` fire-and-forget enqueue with an acknowledged rebuild command or a freshness token for post-apply waits.
- Bound `bm25_rebuild()` enqueue by the caller's remaining deadline.
- Consider prioritizing `Status`/`Rebuild` over `IndexBatch`, or coalescing `IndexBatch` messages during post-apply refresh.
- Avoid using BM25 readiness as a proxy for full post-apply freshness.
