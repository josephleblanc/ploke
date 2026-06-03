# Run review: p1-gemini35-flash-direct-3g2x3-par2-20260602-131345 r5 broad harness

## Short verdict

`node-26f01da56959fd47-r5` is mechanically complete but benchmark-negative. The broad harness produced and committed a three-file `ploke-llm` router URL/string-allocation edit at `e8501bc32c3b96507f5d01739a5004d96625273a`, wrote the submitted-result JSON, admitted it as child `node-6ebbc85b4bd81864` / `branch-ef2894943c2ad250`, and the child runner succeeded. The branch evaluation then rejected it: same-file retry/streak, aborted, convergence, and oracle-eligibility metrics regressed against baseline.

The most important observability finding is a hard mismatch between the raw provider sidecar and the actual artifacts: `llm-full-responses.jsonl` line 2 contains a final response claiming a `ploke-protocol::FanOut` concurrency edit, but the submitted result, headless terminal record, child node, and git diff all prove this slot changed `crates/ploke-llm/src/router_only/{mod.rs,openrouter/mod.rs,google/mod.rs}` instead. The trace summary also reports `final_assistant_message: null`, so the provider stop text is not a reliable artifact for this slot.

## Evidence roots

- Request JSON: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r5.json`
- Prompt: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request/node-26f01da56959fd47-r5.md`
- Headless result: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r5.headless-tui.json`
- Submitted result: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r5.json`
- Turn sidecars: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result/node-26f01da56959fd47-r5.turn-live/{agent-turn-trace.json,agent-turn-summary.json,llm-full-responses.jsonl}`
- Candidate workspace: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness/node-26f01da56959fd47-r5`
- Child node/evaluation: `prototype1/nodes/node-6ebbc85b4bd81864/` and `prototype1/evaluations/branch-ef2894943c2ad250.json`
- Campaign run profile / closure: `prototype1/run-profile.toml`, campaign `closure-state.json`

## Execution path proved

This slot is the broad-headless edit path, not the benchmark-turn path:

```text
Prototype 1 broad request r5
  -> headless ploke-tui adapter / tui_adapter::run_headless_with_model
  -> google/gemini-3.5-flash tool loop
  -> apply_code_edit proposal lifecycle + cargo checks
  -> candidate workspace commit e8501bc32c3b96507f5d01739a5004d96625273a
  -> submitted-result JSON node-26f01da56959fd47-r5.json
  -> Prototype 1 child admission node-6ebbc85b4bd81864 / branch-ef2894943c2ad250
  -> child runner `loop prototype1-runner --node-id node-6ebbc85b4bd81864 --execute`
  -> mechanized branch evaluation branch-ef2894943c2ad250.json
```

Evidence: the request JSON names `request_id: broad-harness-request:node-26f01da56959fd47:r5`, candidate workspace, and submitted-result path. The headless result has `terminal.terminal: applied` with three applied proposal ids and changed paths in `crates/ploke-llm`. Git in the candidate workspace is clean at branch `prototype1-broad-broad-harness-request-node-26f01da56959fd47-r5`, HEAD `e8501bc32c3b96507f5d01739a5004d96625273a`. `node-6ebbc85b4bd81864/node.json` binds `patch_id: broad-harness:broad-harness-request:node-26f01da56959fd47:r5`, `derived_artifact_id: artifact:git-commit:e8501bc32c3b96507f5d01739a5004d96625273a`, and `branch_id: branch-ef2894943c2ad250`.

## Closure state

The parent campaign live process was still running during this review. The top-level `closure-state.json` still shows the baseline eval complete and protocol missing, while this r5 child has its own node status `succeeded` and runner result `exit_code: 0`. This is a per-turn child review, not a campaign fan-in synthesis.

`run-profile.toml` confirms the campaign is `continuous`, `stop_after = "complete"`, broad generation source, `max_generations = 3`, children `min = 2`, `max = 3`, `parallel_targets = 2`, model `google/gemini-3.5-flash`, and MBE disabled.

## Eval and patch output

The committed candidate diff is three files, 23 insertions and 8 deletions:

- `crates/ploke-llm/src/router_only/mod.rs`: replaced `format!("{}/{}/{}", ...)` in `Router::endpoints_url` with manual `String::with_capacity` and `push_str` construction.
- `crates/ploke-llm/src/router_only/openrouter/mod.rs`: replaced `format!("{}/{}", ...)` in `OpenRouterModelId::into_url_format` with manual string construction while preserving `%3A` variant encoding.
- `crates/ploke-llm/src/router_only/google/mod.rs`: replaced `format!("{}/{}", ...)` in `GoogleModelId::into_url_format` with manual string construction.

The submitted-result JSON is present and lists these changed files, but it is generic headless-adapter output. It does not itself include the commit hash, child node id, branch id, or branch evaluation result; those had to be manually joined through the child node and branch registry artifacts.

`branch-ef2894943c2ad250.json` rejects the branch. Baseline versus treatment metrics for `BurntSushi__ripgrep-2209` include:

- `same_file_patch_retry_count`: `0 -> 2`
- `same_file_patch_max_streak`: `1 -> 3`
- `aborted`: `false -> true`
- `convergence`: `true -> false`
- `oracle_eligible`: `true -> false`

The child runner mechanically succeeded, but the descendant benchmark outcome is worse by the operational selection metrics used here.

## Oracle / MBE state

No MBE oracle was run for this slot (`execution.mbe.enabled = false`). The branch evaluation records `oracle_eligible: false` for the treatment, so this child does not provide useful oracle evidence despite a nonempty applied patch in the treatment run.

## LLM and tool behavior

The headless trace records 52 tool requests, 57 tool-completed events, 2 tool-failed events, 3 proposal records, and one completed turn. Tool request counts: `read_file` 18, `list_dir` 11, `request_code_context` 10, `cargo` 5, `code_item_lookup` 4, `apply_code_edit` 4.

Validation evidence is weaker than the request contract:

- Requested validation in the r5 contract was `cargo check -p ploke-eval` and `cargo test -p ploke-eval edit_surface` from the candidate workspace.
- Observed validations instead include focused `cargo check` using `crates/ploke-tui/Cargo.toml`, a focused `cargo test` that failed with exit 101, `cargo test -p ploke-llm` that failed with exit 101, then two focused `cargo check` successes. The final successful checks did not prove `ploke-eval` or `edit_surface`.
- The failed test previews are truncated and are not repaired/re-run after the applied edit. The final green signal is compile-only and focused through the TUI manifest.

The model found model-id URL benchmark notes and URL-formatting code, then made allocation-oriented edits. That is a plausible micro-optimization, but no benchmark command, `ploke-eval` validation, or descendant-performance proof connects it to the requested Prototype 1 benchmark.

## Concrete trace reconstruction

One useful chain is the invalid-canon recovery path:

```text
request_code_context("endpoints_url") / read router files
  -> code_item_lookup returns `crate::router_only::endpoints_url`
  -> apply_code_edit call function-call-ff228120... uses canon `crate::router_only::endpoints_url`
  -> ToolFailed twice: invalid method canon, then internal staging failure
  -> code_item_lookup on `Router` trait returns `crate::router_only::Router`
  -> apply_code_edit call function-call-ffbec47b... uses `crate::router_only::Router::endpoints_url`
  -> staged=1/applied=0 preview events, then applied=1 with new file hash
  -> later final cargo check succeeds
```

This shows real recovery from a tool-format failure into a successful edit. The same proposal lifecycle repeats for `OpenRouterModelId::into_url_format` and `GoogleModelId::into_url_format`: staged-preview completions are followed by applied completions, and the workspace commit verifies the edits landed.

A negative chain is the post-edit context/validation path:

```text
three edits applied
  -> cargo check succeeds in focused ploke-tui scope
  -> request_code_context("get(url") returns degraded/stale context and tells the model to refresh or read_file before relying on it
  -> model does not read current files or run a stronger target after that warning
  -> final cargo check succeeds in the same focused scope
  -> raw provider stop message claims an unrelated ploke-protocol FanOut edit and claims cargo test passed
```

That chain separates mechanical tool success from semantic progress. The model had enough information to act after the applied edit and focused check, but it did not prove the requested validation or produce an accurate final explanation.

## Suspicious-result verification

Suspicious claim checked: the raw provider final response says it optimized `FanOut::run` in `crates/ploke-protocol/src/procedure.rs` with `tokio::join!` and says `cargo check` and `cargo test` passed.

Verification against artifacts disproves that claim for r5:

- Git HEAD `e8501bc32c3b96507f5d01739a5004d96625273a` changes only `crates/ploke-llm/src/router_only/mod.rs`, `openrouter/mod.rs`, and `google/mod.rs`.
- The headless result `terminal.changed_paths` lists only those three `ploke-llm` paths.
- The submitted-result JSON lists only those three changed files.
- `node-6ebbc85b4bd81864/node.json` points to the same derived commit and `target_relpath: crates/ploke-llm/src/router_only/google/mod.rs`.
- The validation ledger records two failed tests and no requested `ploke-eval` validation.

This is not just a harmless final-answer typo: provider-side prose, trace summary, submitted-result content, and workspace diff disagree in ways that would mislead an automated semantic reviewer if it trusted the final model text.

## Protocol review and blind spots

No node-scoped `protocol-artifacts` directory was present for `node-6ebbc85b4bd81864` during this review. The branch evaluation is operational/metric-based and correctly rejects the child on recorded treatment regressions, but it does not adjudicate whether the r5 router micro-optimization was semantically useful. The campaign-level closure state still shows required protocol procedures missing for the baseline instance.

Trace/read-side blind spots observed here:

- `agent-turn-summary.json` and `agent-turn-trace.json` have `patch_artifact.edit_proposals: []` while the headless result records three proposal ids and the workspace commit proves changes. That is a playback/projection gap, not absence of proposal activity.
- The terminal summary is `[success] code=TOOL_EXECUTION_FAILED` because it preserves the first failed apply call even though later proposals applied. That is useful as a warning, but bad as a terminal classifier.
- The submitted-result JSON is present but does not carry enough identity fields to join result -> commit -> child node -> evaluation without manual path/content search.

## What is working

- The broad harness made a real, bounded, outside-`ploke-eval` source edit and committed it cleanly.
- Apply-code edit lifecycle was recoverable after an invalid canon error.
- Admission and runner plumbing did not stop at the submitted-result file; r5 became child `node-6ebbc85b4bd81864` and got a mechanized branch evaluation.
- The branch evaluator rejected a mechanically successful but regressive treatment instead of treating child runner success as a keep.

## What is not working yet

- The final provider text is artifact-inconsistent and appears to describe a prior `ploke-protocol` FanOut attempt, not r5.
- The model did not run the requested validation contract (`cargo check -p ploke-eval`, `cargo test -p ploke-eval edit_surface`).
- The child’s micro-optimization target is not tied to actual descendant benchmark improvement; the branch evaluation shows regression.
- Current summaries underrepresent proposal lifecycle details and overexpose misleading terminal/provider labels.

## Action items

1. **Provider-final-response integrity gap:** add a run-review/alive-bug item for cases where `llm-full-responses.jsonl` final stop content disagrees with submitted result and workspace diff. This r5 artifact gap is concrete: provider says `ploke-protocol::FanOut`; all durable outputs say `ploke-llm` router edit.
2. **Validation contract gap:** broad harness reports should distinguish requested validation from observed validation. Here the requested `ploke-eval` checks were absent, while focused TUI checks were allowed to serve as the final green signal.
3. **Submitted-result join gap:** include `candidate_commit`, `branch_id`, `child_node_id`, and evaluation artifact path in submitted-result or headless-result summaries once known. r5 required manual joins through `node-6ebbc85b4bd81864` and `branch-ef2894943c2ad250.json`.
4. **Proposal playback gap:** promote headless proposal ids and staged/applied transitions into `agent-turn-summary.patch_artifact`; `edit_proposals: []` is misleading for this applied slot.
5. **Adjudication signal:** flag model runs that ignore degraded/stale context warnings after edits and proceed with only a compile-only check, especially when the final explanation claims tests passed.
