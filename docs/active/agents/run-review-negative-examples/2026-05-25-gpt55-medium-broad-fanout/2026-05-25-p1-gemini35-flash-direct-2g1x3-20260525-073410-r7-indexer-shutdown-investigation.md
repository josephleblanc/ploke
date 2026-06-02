# p1-gemini35-flash-direct-2g1x3 r7 Indexer Shutdown Investigation

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-2g1x3-20260525-073410`

Slot: `r7`

Request:
`/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410/prototype1/messages/edit-harness-request/node-b2a63be3e50c7ee8-r7.json`

Candidate workspace:
`/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410/prototype1/workspaces/edit-harness/node-b2a63be3e50c7ee8-r7`

## Verdict

The observed `Failed to shutdown CallbackManager via shutdown send: SendError(..)` is a real alive bug in the indexer shutdown lifecycle, not just a stale-index `ContentMismatch` symptom. The exact panic path is `crates/ingest/ploke-embed/src/indexer/mod.rs:392`, where the `IndexStatus::Completed` branch uses `shutdown.send(()).expect(...)` even though the callback receiver can already be gone.

This does not look like a new regression introduced by the recent broad headless-TUI splice work. Git history shows the panicking `Completed` shutdown send is old code from `7b4a5eb04` (`add bm25 with tests, other cleanup`), while the recent `964fbd8e0` change made the `IndexStatus::Failed` path less panicky. The active run likely exposed an older lifecycle bug under heavier concurrent post-apply indexing.

The panic is probably cleanup-after-completion rather than the primary semantic failure. It is still operationally fatal for the headless-TUI flow because the indexing handler observes the `JoinError` and reports indexing failure.

## Current Run Evidence

The requested r7 submitted result was not present at investigation time and
remains absent:

`/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410/prototype1/messages/edit-harness-result/node-b2a63be3e50c7ee8-r7.json`

At the start of this investigation, the result directory contained prior
headless summaries for r2 through r6 and a base headless summary, but no
`r7.headless-tui.json` or `r7.json`. The r7 headless artifact landed later:

`/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-2g1x3-20260525-073410/prototype1/messages/edit-harness-result/node-b2a63be3e50c7ee8-r7.headless-tui.json`

That later headless artifact records terminal state `timed_out` after 900
seconds. It is durable per-attempt diagnostic evidence, but it is not the
request-declared submitted result.

The active eval log for this campaign is:

`/home/brasides/.ploke-eval/logs/ploke_eval_20260525_005146_1721114.log`

That log contains the persisted shutdown panic cluster:

- line 759383: `Sending Indexing Failed with error message: task 29749 panicked with message "Failed to shutdown CallbackManager via shutdown send: \"SendError(..)\""`
- line 854749: same panic for task 38553
- line 854751: same panic for task 37429

The nearby log context shows the exact lifecycle race:

- `crates/ingest/ploke-embed/src/indexer/mod.rs:391`: `Sending shutdown signal to CallbackManager.`
- `crates/ploke-db/src/query/callbacks_multi.rs:110`: callback manager receives shutdown
- `crates/ploke-db/src/query/callbacks_multi.rs:154`: callback unregisters
- `crates/ingest/ploke-embed/src/indexer/mod.rs:419`: another branch logs `Cannot send shutdown message, other side dropped`
- later, the `Completed` branch send panics and `crates/ploke-tui/src/app_state/handlers/indexing.rs:213` reports the panicked indexing task

The same log has r7-specific work:

- line 768941 starts parse/index work for workspace `node-b2a63be3e50c7ee8-r7`
- lines 1001665 onward apply/scan r7 `crates/ploke-records/src/selection.rs`
- lines 1002197 and following report r7 `ContentMismatch` warnings for records in `crates/ploke-records/src/selection.rs`

The r7 workspace had local candidate edits in `crates/ploke-records/src/history.rs`, `crates/ploke-records/src/ids.rs`, and `crates/ploke-records/src/selection.rs`. The later durable r7 headless artifact records those edits as a timed-out attempt, but no submitted child result was written.

## Log Persistence

For this run, the relevant persisted process logs are under:

`/home/brasides/.ploke-eval/logs/`

`crates/ploke-eval/src/tracing_setup.rs` constructs `ploke_eval_{run_id}.log` and `llm_full_response_{run_id}.log` under `ploke_eval_home()/logs`. The matching current files are:

- `ploke_eval_20260525_005146_1721114.log`
- `llm_full_response_20260525_005146_1721114.log`

The candidate workspace did not have a local `crates/ploke-tui/logs` directory for r7. That matches the eval/headless path using the already-initialized eval tracing subscriber rather than the normal standalone TUI log setup.

## Exact Code Path

`crates/ingest/ploke-embed/src/indexer/mod.rs` has three materially different shutdown paths:

- `IndexStatus::Failed`, lines 360-377: sends shutdown with `let _ = shutdown.send(())`, awaits `idx_handle`, and returns an error.
- `IndexStatus::Completed`, lines 382-395: if the callback handler is not finished, logs at line 391 and then panics on send failure at line 392:
  `shutdown.send(()).expect("Failed to shutdown CallbackManager via shutdown send");`
- `idx_handle` completion branch, lines 408-425: attempts shutdown with a non-panicking `match`, logging `Cannot send shutdown message, other side dropped` at line 419.

The observed panic message can only come from the `Completed` branch. The code already has a non-panicking treatment for the same send failure in the `idx_handle` branch, so the `Completed` branch is internally inconsistent.

`crates/ploke-tui/src/app_state/handlers/indexing.rs` awaits the spawned indexer task at lines 204-215. If the inner task panics, the outer handler converts the `JoinError` into the observed `Sending Indexing Failed with error message: task ... panicked ...` warning at line 213.

## Git History

Relevant history for `crates/ingest/ploke-embed/src/indexer/mod.rs`:

- `7b4a5eb04 add bm25 with tests, other cleanup`: introduced the current shutdown lifecycle structure, including the panicking `Completed` branch send at line 392 and the non-panicking `idx_handle` branch send handling.
- `a151c5d51 wip: getting harness near working...`: changed some indexer logging levels, not the shutdown behavior.
- `964fbd8e0 Fix broad headless TUI splice harness`: changed `IndexStatus::Failed` from a panic-oriented path to graceful shutdown/error propagation with `let _ = shutdown.send(())`; it did not change the `Completed` branch expect at line 392.

Regression assessment: this is not caused by the May 14 failure-path change. It is an older latent shutdown bug made visible by the active broad headless-TUI workload.

## ContentMismatch Assessment

The r7 log does contain real `ContentMismatch` warnings for files under `node-b2a63be3e50c7ee8-r7/crates/ploke-records/src/selection.rs`, and earlier r7 warnings also appeared for `history.rs`. Those warnings are consistent with stale snippet/file hash reads after candidate edits.

However, the shutdown panic is separate. `ContentMismatch` appears during snippet extraction/indexing. The panic occurs later when the indexer has received a completion signal and tries to shut down a callback manager whose receiver side has already closed. The log sequence with `SHUTODWN RECEIVED: CALLBACK`, callback unregistering, and repeated shutdown sends supports a shutdown lifecycle race rather than a content-hash cause.

Primary cause classification:

- `ContentMismatch`: stale-index/read consistency issue during post-edit indexing; likely contributes noise and failed snippets.
- `Failed to shutdown CallbackManager`: separate lifecycle bug; likely cleanup-after-completion, but fatal to the headless-TUI path because it panics the indexing task.

## Other Observed Noise

The legacy parse warnings are from `crates/ingest/syn_parser/src/parser/visitor/mod.rs` skipping non-primary targets in legacy parse mode. They are persisted in the same eval log and are broad workspace parse noise, not direct evidence for the shutdown panic.

`FileManager received unexpected event: Llm(ChatCompletion(...))` comes from `crates/ploke-tui/src/file_man.rs` handling non-file-manager events. It is unexpected event routing noise in the same headless session, but the indexer panic has a direct persisted stack-independent explanation.

The `INVALID_MODEL_RESPONSE kind=ModelBehavior` entries exist in the campaign log, but the lines I checked were not r7-specific. I would not treat them as the direct cause of the r7 indexer shutdown panic without a narrower request/result correlation.

## Next Actions

1. File/fix an alive bug for `ploke-embed` indexer shutdown: make the `IndexStatus::Completed` branch treat `shutdown.send(())` failure as non-fatal, matching the `idx_handle` branch.

2. Make callback shutdown idempotent and terminal: after sending shutdown on completion, either join the callback handler deterministically or mark/report the callback lifecycle explicitly instead of repeatedly sending shutdown on subsequent `Completed` statuses.

3. Add a regression test for the completed-indexing path where the callback shutdown receiver is already dropped before the `Completed` branch sends shutdown.

4. For broad headless-TUI diagnostics, preserve the log path or live stderr pointer in the request/result artifacts when the harness exits before writing `rN.json`, so a missing result file still has a durable evidence pointer.

5. Treat the r7 `ContentMismatch` warnings as a separate stale-index/read-consistency issue. They should not block fixing the shutdown panic, and the shutdown panic should not be explained away as `ContentMismatch`.
