# Run review: node-26f01da56959fd47-r3 broad harness

## Short verdict

`node-26f01da56959fd47-r3` is mechanically complete: the headless TUI trace finished, an edit proposal was applied, a submitted-result JSON exists, and the candidate workspace is clean at commit `cb2a7136eea23324635627544891465cd9cca05f`.

Benchmark usefulness is weak. The applied change is in `crates/ploke-ty-mcp/src/manager.rs`; the requested validation contract was `cargo check -p ploke-eval` plus `cargo test -p ploke-eval edit_surface`, but the model-visible validation ran focused `ploke-ty-mcp` cargo commands. A direct `cargo metadata` check of the candidate workspace showed `ploke-eval` has zero direct `ploke-ty-mcp` dependencies, and a search under `crates/ploke-eval` found no `ploke-ty-mcp` references. The patch may improve the standalone MCP crate, but this review found no evidence that it can improve the active Prototype 1 descendant benchmark path.

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r3.json`
- Request prompt: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r3.md`
- Headless result: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r3.headless-tui.json`
- Submitted result JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r3.json`
- Turn trace/summary: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r3.turn-live/agent-turn-{trace,summary}.json`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness/node-26f01da56959fd47-r3`
- Campaign closure state: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/closure-state.json`
- Parent node state: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/nodes/node-26f01da56959fd47/node.json`

## Execution path proved

This was a published broad-harness headless-TUI attempt, not a benchmark eval turn:

```text
published edit-harness request r3 -> headless ploke-tui adapter -> model/tool trace -> non_semantic_patch proposal -> approve/apply -> candidate commit -> submitted edit-harness result JSON
```

Evidence:

- Request id `broad-harness-request:node-26f01da56959fd47:r3` and submitted-result path are in the request JSON.
- `agent-turn-summary.json` records selected model `google/gemini-3.5-flash`, task id `broad-harness-request:node-26f01da56959fd47:r3`, terminal outcome `completed`, and terminal summary containing the recovered `TOOL_EXECUTION_FAILED` diagnostic.
- `node-26f01da56959fd47-r3.headless-tui.json` records terminal state `applied`, proposal id `7ee070a5-3363-5161-a4df-f021d57fe773`, and changed path `crates/ploke-ty-mcp/src/manager.rs`.
- Workspace git evidence shows `HEAD cb2a7136eea23324635627544891465cd9cca05f`, parent `8f478c66e30224adf234439c8c6395bf521a6373`, subject `prototype1 broad harness result broad-harness-request:node-26f01da56959fd47:r3`, and a clean status.

## Closure and live-run state

- Campaign-level `closure-state.json` exists at the campaign root and was updated at `2026-06-02T13:22:36.932391457+00:00`, but it has no single top-level closure status field.
- Parent `node.json` still reports `status: running` with updated time `2026-06-02T13:22:36.992730554+00:00`.
- A read-only `pgrep` during review showed `./target/debug/ploke-eval loop prototype1-state --debug-tools` still running, plus active descendant `cargo check -p ploke-eval --bin ploke-eval` work for later nodes.

## Eval, patch output, and submitted result

- Submitted-result JSON is present at `node-26f01da56959fd47-r3.json`.
- It records the request, workspace path, changed file summary, and generic expectation that `ploke-eval` will compile/evaluate the admitted child artifact.
- It does not include the candidate commit hash; the commit had to be recovered from the workspace git state.
- Verified patch against the target artifact:
  - `git diff --stat 8f478c66e30224adf234439c8c6395bf521a6373 HEAD` reports `crates/ploke-ty-mcp/src/manager.rs | 82 ++++++++++++++++++++++++++++++--------`, `65 insertions(+), 17 deletions(-)`.
  - `git show HEAD:crates/ploke-ty-mcp/src/manager.rs` contains the new `start_locks: DashMap<ServerId, Arc<tokio::sync::Mutex<()>>>` field, `start_autostart` now calls `self.ensure_started(&spec.id).await?`, and `spawn_with_backoff` now retries health checks up to three times with a 150ms delay before killing/restarting the service.

No oracle, MBE, or descendant benchmark result was present for this r3 broad-harness slot at review time.

## LLM and tool behavior

Trace inventory from `agent-turn-trace.json`:

- `ToolRequested`: 63
- `ToolCompleted`: 63
- `ToolFailed`: 1
- `TurnFinished`: 1
- Tool lifecycle events by tool: `cargo` 6, `code_item_lookup` 4, `list_dir` 22, `non_semantic_patch` 3, `read_file` 70, `request_code_context` 22.

Concrete chain:

1. The model began with `cargo check`, which succeeded against the focused `crates/ploke-ty-mcp/Cargo.toml` manifest, not the requested `ploke-eval` package.
2. It searched for `ploke-ty-mcp dependency`, read `crates/ploke-ty-mcp/implementation_logs/0015.md`, `context7_client.rs`, `tests/e2e_mcp.rs`, `Cargo.toml`, and portions of `manager.rs`.
3. It attempted `code_item_lookup` for `from_config` as a method under `crate::manager::McpManager`; that tool failed with `invalid_format`/`No code item named from_config ... Hint: if this item is a free function, retry with node_kind=function`.
4. The model recovered by reading `manager.rs` directly, then requested a `non_semantic_patch` that added per-server `start_locks`, changed `start_autostart` to call `ensure_started`, and added same-process health-check retries.
5. The patch lifecycle is important: the first `non_semantic_patch` completion was only staged (`ok:true`, `staged:1`, `applied:0`, `auto_confirmed:false`), then the same call later completed as applied (`applied:1`, `partial:false`). The headless terminal record correctly ended as `applied`.
6. The final model-visible validation was `cargo test` with focused `ploke-ty-mcp/Cargo.toml`; it exited 0 with 3 unit tests, 3 e2e tests, and 1 doctest passing in the recorded tail.

Last point where the model had enough information to act: after direct reads of `manager.rs` around `ensure_started`, `start_autostart`, `spawn_with_backoff`, and `respawn_with_backoff`, it had enough local information to patch `ploke-ty-mcp`. It never established that this crate was in the `ploke-eval` benchmark path.

## Mechanical completion versus benchmark usefulness

Mechanical completion is real: an applied proposal, a submitted-result JSON, and a clean candidate commit exist.

Benchmark usefulness is not established:

- The request contract’s validation commands were for `ploke-eval`; the trace validations were focused on `ploke-ty-mcp`.
- `cargo metadata` for the candidate workspace found `ploke-eval` direct dependencies but no `ploke-ty-mcp` dependency (`ploke_eval_direct_ploke_ty_mcp_deps: 0`; resolved direct dependency ids did not include the `ploke-ty-mcp` package id).
- Searching `crates/ploke-eval` for `ploke-ty-mcp` returned no matches.
- The submitted result’s expected descendant effect is generic and not supported by tool or checkout evidence.

## Protocol/read-side blind spots

- The terminal summary combines `[success]` with `code=TOOL_EXECUTION_FAILED` because a recovered `code_item_lookup` failure remained in the terminal diagnostic. That is useful as a lifecycle warning but misleading as a terminal status line.
- `agent-turn-summary.json` has `patch_artifact.applied: true`, but `edit_proposals: []`, `expected_file_changes: []`, and `any_expected_file_changed: false`, even though the headless result and git diff prove a file changed. This is a projection gap: reviewers must join terminal record plus git state manually.
- The submitted-result JSON lacks the final commit hash and only records workspace/submitted-result paths and changed-file summary.

## What is working

- The broad-harness path produced a clean committed candidate from a staged proposal.
- The trace preserved enough model/tool events to reconstruct the recovered tool failure and staged-to-applied edit lifecycle.
- Focused `ploke-ty-mcp` validation output was visible to the model and recorded with exit code 0.

## What is not working yet

- The model optimized a workspace member without proving it affected the requested `ploke-eval` descendant benchmark.
- The request’s validation contract was not enforced in the headless attempt.
- Patch/read-side summaries require manual joining to identify the real commit and changed file.
- The terminal status surface does not separate recovered tool failures from terminal failure.

## Action items

1. **Validation-contract gap:** Broad-harness result accounting should record whether the model ran the request-declared validation commands. Here it did not run `cargo check -p ploke-eval` or `cargo test -p ploke-eval edit_surface`.
2. **Benchmark-path gap:** Add or expose a cheap dependency/path check before broad-harness descendants optimize unrelated workspace members. The r3 patch changed `ploke-ty-mcp`, but review evidence did not tie that crate to the `ploke-eval` runner path.
3. **Projection gap:** Include the final candidate commit hash and changed paths in the submitted-result JSON, not only in the terminal record/git checkout.
4. **Lifecycle status gap:** Split recovered tool failures from terminal failure in the headless terminal summary so `completed/applied` attempts are not summarized as `TOOL_EXECUTION_FAILED` without context.
