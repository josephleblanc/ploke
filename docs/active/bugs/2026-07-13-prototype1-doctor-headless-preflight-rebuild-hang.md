# BM25 control commands could wait outside caller readiness deadlines

Status: resolved; the Stage 0 campaign was not blocked

Discovered: 2026-07-13

## Broken Contract

`RagService::bm25_rebuild` is documented as a fire-and-forget command. It must
therefore report immediate mailbox admission or an explicit full/closed error;
it must not wait indefinitely for capacity. Likewise,
`bm25_status_with_timeout` must apply its caller-supplied deadline to both
mailbox admission and the status response.

Neither contract permits treating a rejected rebuild as success or accepting a
stale index.

## Initial Live Observation And Correction

Campaign and worktree:

```text
p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1
/home/brasides/.ploke-eval/worktrees/p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1
```

Two doctor commands were initially terminated by external 90-second bounds and
one by a 120-second bound before emitting JSON. That was first classified as a
BM25 enqueue hang. Process inspection disproved that attribution: the command
was CPU-active across the main process and worker threads while ingesting the
workspace.

The same command, rerun with a 600-second safety cap, passed in approximately
115 seconds:

```text
timeout --signal=TERM 600s \
  /home/brasides/code/ploke/target/debug/ploke-eval \
  loop prototype1-doctor \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-stage0-g35f-pplxembed-3g1x3-p3-20260713-1 \
  --headless-tui-setup-preflight \
  --format json
```

The report contained `headless_tui_setup_preflight.outcome = "passed"`, phase
`baseline_eval`, and no blockers. The internal 60-second BM25 readiness bound
begins after workspace preparation; an external cap shorter than total parsing
time does not test that bound.

## Independent Reproduction

The repository already contained an ignored full-mailbox reproduction for
`bm25_rebuild`. Before the repair it failed because the rebuild future consumed
the entire 50-millisecond outer bound while waiting for capacity.

A second strict reproduction established the adjacent status defect: with a
full mailbox, `bm25_status_with_timeout(20 ms)` also consumed a
50-millisecond outer bound because its timeout began only after
`Sender::send(Status)` returned.

The closed-mailbox rebuild case already returned an explicit channel error.

## Source Boundary And Cause

```text
RagService::bm25_rebuild
-> mpsc::Sender::send(Bm25Cmd::Rebuild).await

RagService::bm25_status_with_timeout
-> mpsc::Sender::send(Bm25Cmd::Status).await
-> timeout(response)
```

The bounded BM25 actor mailbox has capacity 128. Both methods could wait for
capacity without a controlling deadline. The status method then started a new
full-duration response timeout, so its advertised timeout did not bound the
whole operation.

## Repair

- `bm25_rebuild` now uses immediate `try_send` admission and maps full and
  closed mailboxes to distinct `RagError::Channel` details.
- `bm25_status_with_timeout` now creates one deadline and uses it for both the
  status enqueue and the oneshot response.
- No queue capacity, actor ordering, sparse-readiness requirement, or freshness
  check was relaxed.

## Regression And Validation Evidence

Strict regressions:

- `bm25_rebuild_backpressure_repro_blocks_before_ack`: red before repair, green
  after repair;
- `bm25_status_bounds_enqueue`: red before repair, green after repair;
- `bm25_rebuild_rejects_closed_mailbox`: green before and after repair;
- `test_bm25_rebuild`: green normal-path admission.

Broader validation:

- `cargo test -p ploke-rag`: 43 unit tests and 1 doctest passed;
- `sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion`:
  passed;
- `prototype1_doctor_headless_setup_preflight_blocks_on_rag_unavailable`:
  passed;
- `cargo test -p ploke-tui request_code_context --lib`: 7 passed and 2
  live/quarantined tests remained ignored;
- `cargo build -p ploke-eval`: passed;
- `cargo check --workspace`: passed with existing warnings;
- the measured live-worktree headless setup preflight: passed.

`cargo clippy -p ploke-rag --all-targets -- -D warnings` did not reach a
slice-specific verdict because existing `ploke-error` and `ploke-core` lints
were promoted to dependency errors first.

## Campaign Disposition

The short external timeouts were read/setup diagnostics and wrote no typestate
transition evidence. The campaign remained at `baseline_eval` throughout and
is eligible to continue. This is a resolved shared-library defect, not an
abandoned-run incident.

## Operator Follow-Up

The approximately two-minute setup has long periods without terminal output.
That observability gap should be handled as operator progress reporting, not as
evidence that the BM25 readiness deadline failed.

## Related Bugs

- [`2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md`](2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md)
  introduced the typed doctor setup preflight.
- [`2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
  covers adjacent post-apply sparse-index freshness behavior and remains green.
