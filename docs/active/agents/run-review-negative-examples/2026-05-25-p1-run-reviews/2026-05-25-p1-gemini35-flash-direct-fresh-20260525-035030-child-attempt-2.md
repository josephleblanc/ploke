# Prototype 1 Child Attempt Review: `p1-gemini35-flash-direct-fresh-20260525-035030` r2

## Verdict

The r2 broad-harness attempt is admissible as a committed child artifact and
useful trace evidence, but it is not clean improvement evidence. It produced
branch `prototype1-broad-broad-harness-request-node-dfbca03c896b03ae-r2` at
commit `e9c4b1f5ebc29da4b77617bae4bc7cff6a592885`, changing
`crates/ploke-ty-mcp/src/manager.rs` with 29 insertions and 4 deletions. The
headless-TUI terminal state is `applied`.

The patch is performance-motivated: it serializes per-server MCP startup,
reuses that startup path for autostart, locks respawn, and skips the startup
health check by default unless `PLOKE_SKIP_MCP_HEALTH_CHECK` is explicitly
disabled. That last change is the main quality concern. It is a speed/reliability
tradeoff with no benchmark measurement and no regression test for the new
default.

Cargo output was model-visible and all recorded cargo validations passed, but
every validation resolved to the focused `crates/ploke-ty-mcp/Cargo.toml`.
The persisted request contract listed `cargo check -p ploke-eval` and
`cargo test -p ploke-eval edit_surface`; those commands were not run. Treat the
attempt as useful loop evidence, not as benchmark-clean evidence.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030`
- Parent node: `node-dfbca03c896b03ae`
- Reviewed request slot:
  `prototype1/messages/edit-harness-request/node-dfbca03c896b03ae-r2.{json,md}`
- Reviewed result artifacts:
  `prototype1/messages/edit-harness-result/node-dfbca03c896b03ae-r2{,.headless-tui}.json`
- Reviewed workspace:
  `prototype1/workspaces/edit-harness/node-dfbca03c896b03ae-r2`
- Reviewed commit:
  `e9c4b1f5ebc29da4b77617bae4bc7cff6a592885`

I did not advance the loop, rerun cargo in the candidate workspace, or mutate
run artifacts. This review uses persisted artifacts plus read-only git
inspection.

## Execution Path

This is the Prototype 1 child-plan broad-harness path:

```text
prototype1-step -> child_plan broad-harness fanout
  -> tui_adapter::run_headless_with_model
  -> headless-TUI result artifacts
  -> committed candidate workspace
```

The broad-harness request asked for an edit outside the protected `ploke-eval`
authority surface to improve the `Prototype 1 descendant performance`
benchmark.

## What Changed

Commit `e9c4b1f5` changed one file:

- `crates/ploke-ty-mcp/src/manager.rs`

The committed diff:

- adds `start_locks: DashMap<ServerId, Arc<tokio::sync::Mutex<()>>>` to
  `McpManager`;
- initializes `start_locks` in `from_config`;
- adds a double-check lock around `ensure_started`;
- routes `start_autostart` through `ensure_started`;
- locks `respawn_with_backoff`;
- skips `spawn_with_backoff` health checks by default unless
  `PLOKE_SKIP_MCP_HEALTH_CHECK` is set to `0` or `false`.

The concurrency lock is plausible: it prevents duplicate process spawns when
parallel callers all see a server as not running. The health-check default is
much more suspicious. It changes startup semantics from "spawn and prove
`list_tools` works" to "spawn and trust the process connection" by default. That
may reduce startup latency, but it is unmeasured and weakens startup
correctness.

## Trace Reconstruction

The headless result contains 110 compact events, 9 attempt records, 5 cargo
validations, and terminal `applied`.

Early useful context:

- The model first ran focused `cargo check` and `cargo test`; both passed before
  the edit.
- It listed `crates/ploke-ty-mcp/src`, read `lib.rs`, and explored the workspace.
- A `list_dir` request for `../../nodes` failed because the resolved path was
  outside configured roots. The model continued.
- It used `request_code_context` and direct reads to find
  `crates/ploke-ty-mcp/src/manager.rs`.
- It read the relevant startup, health-check, autostart, and respawn code.

Edit lifecycle:

- `function-call-b2c0269f-ce8b-41e0-b94b-5325fd28e9a8` staged and applied the
  new `start_locks` field on `McpManager`. The same call first produced
  `staged = 1, applied = 0`, then later an applied result.
- `function-call-616f2f04-32f4-4d30-b9c1-0d0a401a4c2e` tried a semantic edit
  for `from_config` with canon `crate::manager::from_config`; it failed with
  `invalid_format`.
- `function-call-37d46a13-3c5a-49f4-919f-02e21b15fe01` retried
  `crate::manager::McpManager::from_config`; it failed because two candidates
  matched after method parsing.
- `function-call-d163690a-7024-4f3e-abbc-2361e75c3db3` tried a
  `non_semantic_patch`, but failed because multiple patch entries targeted the
  same file.
- `function-call-89547f1c-a418-441e-9a33-bdd83b9a4d6a` retried as a single
  patch and applied the final one-file diff.

The observed `TOOL_EXECUTION_FAILED` warning lines up with real persisted tool
failures, but those failures were recoverable. The final terminal state and git
commit agree on the changed file.

I did not find the live "non-unique import-id" warning text in the r2
headless-TUI JSON or nearby campaign JSON/Markdown artifacts. From the
persisted evidence available here, that warning looks like a console/database
load warning rather than a model-visible tool result.

## Cargo Visibility And Validation

Cargo output was visible to the model. The headless ledger records each cargo
call as a `tool_completed` event with JSON payloads containing `ok`, exit code,
manifest path, stderr tails, and stdout tails.

Recorded cargo validations:

- `function-call-e76424af-536e-4501-b03d-ee6380f12993`: `cargo check`,
  focused manifest `crates/ploke-ty-mcp/Cargo.toml`, success.
- `function-call-e487bfd5-326c-494e-a2ef-0f74390f4da4`: `cargo test`,
  focused manifest `crates/ploke-ty-mcp/Cargo.toml`, success.
- `function-call-ef216e38-6eb0-4e3c-866c-d10593297e4f`: post-edit
  `cargo check`, focused manifest `crates/ploke-ty-mcp/Cargo.toml`, success.
- `function-call-65ce6c3d-7f10-44b1-b68e-a52cc17a53dd`: post-edit
  `cargo test`, focused manifest `crates/ploke-ty-mcp/Cargo.toml`, success.
- `function-call-0b90855f-20d5-4d9f-8098-ffc367303301`: repeated
  `cargo test`, focused manifest `crates/ploke-ty-mcp/Cargo.toml`, success.

The successful focused cargo output is meaningful for basic compilation of the
changed crate. It is not the same as the request contract. The submitted result
contract listed:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval edit_surface
```

No recorded cargo validation ran either command. I also found no formatting
check and no performance measurement.

## Classification

- `admissible`: yes, as a committed broad-harness child attempt outside the
  protected `ploke-eval` surface.
- `useful`: yes, because it shows a recoverable edit-tool sequence and
  model-visible cargo output.
- `benchmark-improving`: unproven. The patch has no benchmark measurement and
  makes an untested startup health-check tradeoff.
- `suspicious`: yes. The final answer overclaims "dramatically improve"
  performance and "all tests passing" while only focused `ploke-ty-mcp` checks
  were run.
- `blocker`: no immediate run-artifact blocker. This should not stop the loop
  by itself, but the candidate should be downgraded or separately adjudicated
  before promotion.

## Positive Examples And Adjudication Candidates

Positive examples:

- `tool failure -> recovery`: invalid semantic edit targets and a malformed
  multi-entry patch did not end the attempt; the model switched to a valid
  single-file non-semantic patch.
- `applied patch -> cargo validation`: after the final applied patch, the model
  ran focused `cargo check` and two focused `cargo test` calls and saw passing
  payloads.
- `context -> targeted patch`: direct reads of `manager.rs` led to a coherent
  one-file edit rather than unrelated churn.

Candidate adjudication fields:

- Did the child run the validation commands specified by the request contract?
- Did final validation cover the changed crate, the requested validation
  surface, or both?
- Did the patch trade correctness/safety checks for speed without measurement?
- Did the model's final claim match the actual validation scope?
- Did recoverable tool failures lead to a corrected edit path rather than
  repeated failing calls?

## Action Items

1. Add or promote an adjudication signal for validation-scope mismatch:
   focused crate checks can be useful, but they should not satisfy a
   `ploke-eval` contract.
2. Add an adjudication signal for performance patches that disable or bypass
   health checks without benchmark and failure-mode evidence.
3. Consider a broad-harness validation improvement that exposes the requested
   validation commands more directly to the model or runs them as an admission
   check after the model stops.
4. Track console-only database warnings such as non-unique import-id warnings in
   durable headless artifacts if they are relevant to judging retrieval quality.
