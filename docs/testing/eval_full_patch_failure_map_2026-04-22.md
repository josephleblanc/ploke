# Eval Full-Patch Failure Map

Purpose

- Reduce "everything is broken" into a small number of binary checkpoints.
- Separate provider failures from harness failures so downstream noise stops contaminating the diagnosis.
- Record which preconditions for a successful full-patch eval are already covered by tests, and which ones are still unproven.

How To Use This Note

1. Pick one pinned model/provider pair.
2. Walk the checkpoints in order.
3. Stop at the first failing checkpoint.
4. Do not blame later stages until the earlier one is proven good.

Current High-Risk Commits

- `61ef9106`
  - LLM/session error handling and repair-loop refactor.
  - Plausible impact: fewer recoverable turns, more repair-budget exhaustion, more missing final response capture.
- `1af74d1d`
  - Eval finalization started re-reading the exported MSB submission artifact.
  - Plausible impact: run can fail after work is already done.
- `d7b302cd`
  - Proposal id migration and edit-submission overlay changes.
  - Plausible impact: mixed apply results can be marked more cleanly than they are actually observed.
- `6b2de796`
  - Batched `non_semantic_patch` support.
  - Plausible impact: same-file patch batches that arrive split across entries now fail hard.
- `6feee5a3`
  - Insertion tool added and mixed-tool proposal interactions got more complicated.
  - Plausible impact: more stale/conflict outcomes for otherwise disjoint edits in the same file.

Observed Failure Classes From Recent Live Runs

- Provider transport failure
  - `429`, `502`, decode timeout.
- Tool-call repair failure
  - invalid args for `cargo` or `read_file`
  - repair budget exhausted
- Edit pipeline observability failure
  - edits appear applied in trace but final summary/export state is incomplete
- Finalization failure
  - run mutates state or writes partial artifacts but does not flush coherent final outputs

Success Preconditions

1. The prepared run resolves correctly.

- Meaning:
  - prepared manifest exists
  - checkout resets
  - prompt construction runs
- Existing evidence:
  - live runs do consistently reach repo reset and prompt construction
  - `crates/ploke-eval/tests/setup_phase_integration.rs`
- Still unknown:
  - whether recent registry/finalization changes can mark a run failed after successful work
- Smallest missing test:
  - an integration test that starts from a prepared run manifest and asserts the run reaches "ready to chat" without depending on later submission parsing

2. The provider response body arrives and is classified correctly.

- Meaning:
  - the HTTP/body layer receives a real response
  - embedded provider errors are surfaced with retryable/non-retryable semantics that match reality
- Existing tests:
  - `crates/ploke-eval/src/tests/llm_deserialization.rs`
    - proves a real captured qwen reasoning-only body can be deserialized or fail in a documented way
  - `crates/ploke-llm/src/manager/session.rs`
    - `parse_outcome_choice_error_returns_api_error_with_provider_metadata`
- Known risks:
  - `61ef9106` now aborts on the first `choice.error`
  - `61ef9106` maps some embedded provider errors to `status=200`, which can bypass retry behavior that keys off status
- Smallest missing test:
  - a unit test fixture where one choice has `error` and a later choice is valid, proving whether the parser still accepts a usable later choice

3. The chat loop preserves a usable response long enough to attempt tools.

- Meaning:
  - final content or tool calls survive the session loop
  - repair budget is not exhausted prematurely
- Existing tests:
  - `crates/ploke-tui/src/llm/manager/session.rs`
    - `repair_budget_is_bounded_locally`
  - `crates/ploke-tui/src/llm/manager/semantics.rs`
    - repair normalization tests for invalid provider tool args
- Known risks:
  - `61ef9106` introduced a hard session-wide repair cap of 4
  - recent live runs failed here with invalid tool args and `REPAIR_BUDGET_EXHAUSTED`
  - `repair_attempts` is a single session-wide counter; it does not reset after partial progress
- Smallest missing test:
  - a deterministic chat-loop fixture that needs more than four repair rounds before converging, proving whether the current cap is too aggressive for real provider behavior

4. Parsed tool calls reach the tool layer in the supported shape.

- Meaning:
  - tool args are syntactically valid
  - multi-file and same-file patch payloads conform to current `ns_patch` contract
- Existing tests:
  - `crates/ploke-tui/tests/integration/ns_patch_completion_regression.rs`
    - proves multi-file batch staging works
  - `crates/ploke-tui/tests/integration/tool_call_event_ordering.rs`
    - proves request/completion event ordering
- Known risks:
  - `6b2de796` rejects duplicate same-file `non_semantic_patch` entries
  - provider-side tool-call formatting is failing before we even test apply semantics
- Smallest missing test:
  - an integration test where the same file appears twice in a patch batch, documenting the failure shape and establishing whether coalescing is required upstream

5. The edit pipeline applies what it stages, or fails loudly enough to recover.

- Meaning:
  - semantic or non-semantic edits either apply correctly or surface a precise failure
  - partial apply is not silently reclassified as success
- Existing tests:
  - `crates/ploke-eval/src/tests/replay.rs`
    - proves submission export is based on repo diff, not fabricated from assistant text
  - `crates/ploke-tui/src/rag/tests/editing_bulk_tests.rs`
    - proves overlapping pending semantic proposals are staled/applied in a defined order
  - `crates/ploke-tui/tests/integration/post_apply_rescan.rs`
    - proves approving a proposal triggers post-apply rescan messaging
  - `crates/ploke-io/src/write_tests.rs`
    - local write/apply coverage for patch application behavior
- Known risks:
  - `d7b302cd` treats batched ns apply as successful whenever `applied > 0`
  - async auto-confirm means repo state can change after the tool response has already been emitted
  - mixed-tool same-file conflicts can stale valid insertions or patches because ns ranges are treated as whole-file
- Smallest missing test:
  - an integration test where a batched ns proposal has one file apply and one file fail, asserting that the proposal is not reported as simply applied

6. Final response capture and turn finalization stay coherent.

- Meaning:
  - terminal turn record
  - final assistant message / llm response
  - tool trace
  - patch artifact
  - submission export
  all describe the same turn outcome
- Existing tests:
  - `crates/ploke-eval/src/runner.rs`
    - `drain_post_terminal_events_captures_late_llm_response`
- Known risks:
  - `61ef9106` only drains post-terminal events when `artifact.llm_response` is still `None`
  - fixed `750ms` drain window may still miss late events under load
  - recent live run `run-1776859491700-structured-current-policy-1573e9d4` showed applied edits in trace with missing final artifact fields
- Smallest missing test:
  - an integration test that simulates both race directions:
    - response arrives before `TurnFinished`
    - response arrives after `TurnFinished`
  and asserts the final artifact is coherent in both cases

7. Packaging/export cannot retroactively fail a successful run without saying exactly why.

- Meaning:
  - once a repo diff is real, export should either succeed or produce an explicit packaging failure marker
- Existing tests:
  - `crates/ploke-eval/src/tests/replay.rs`
    - proves export reads repo diff, not invented content
  - `crates/ploke-eval/src/runner.rs`
    - submission artifact write tests now assert the runner keeps the in-memory `fix_patch` instead of re-reading the file it just wrote
- Known risks:
  - packaging can still fail while writing the submission artifact itself
  - current failure mode can still make post-work metadata failure look like a model failure unless packaging is surfaced distinctly
- Smallest missing test:
  - a runner integration test where the submission file is unreadable or malformed after patch production, asserting the run records a packaging failure distinctly from model/tool failure

What Is Already Proved

- Export uses actual repo diff, not assistant text.
- Qwen reasoning-only response deserialization has at least one documented fixture.
- Tool-call request/completion ordering is covered.
- Multi-file `ns_patch` staging is covered.
- Post-apply rescan signaling is covered.
- Proposal persistence load/save is covered.

What Is Not Proved Yet

- Whether a full runner path preserves enough final artifacts to make a post-diff packaging failure obvious without opening the registry record.
- Whether the current shared repair budget is generous enough for real provider behavior beyond four repair rounds.

Resolved In This Pass

- `parse_chat_outcome` no longer aborts immediately on the first errored choice when a later choice is usable.
- the terminal branch now always enters post-terminal draining, and the drain loop no longer exits immediately just because `llm_response` was already populated before entering.
- mixed-result batched `ns_patch` apply is no longer classified as success at the proposal level just because one file applied.
- the runner no longer re-reads `multi-swe-bench-submission.jsonl` just to recover `fix_patch`; packaging now carries the in-memory patch it already computed.
- repair-budget bookkeeping now lives in one helper, and the current policy is explicitly tested: one shared per-session budget across repair-triggering failures, with the fifth repairable failure aborting as `REPAIR_BUDGET_EXHAUSTED`.
- `run_chat_session` now has a deterministic integration-style test proving the loop can survive four consecutive tool-argument repair rounds and still complete on the fifth response.
- `run_chat_session` now also has the complementary deterministic test proving the loop aborts before a would-be sixth response if a fifth repair would be required.
- `run_benchmark_turn` now has deterministic tests for both final-response race directions:
  - response before `ChatTurnFinished`
  - response after `ChatTurnFinished`
- duplicate same-file `non_semantic_patch` batch entries are now pinned to one deterministic failure shape: the tool rejects them before staging and tells the caller to combine hunks into one unified diff per file.
- packaging write failure after a real repo diff now has a deterministic classification test: the run registration lands in `Packaging = Failed`, `submission_status = Missing`, and the failure is not misclassified as setup failure.

Regression Tests Added

- `crates/ploke-llm/src/manager/session.rs`
  - `parse_outcome_skips_errored_choice_when_later_choice_is_valid`
- `crates/ploke-eval/src/runner.rs`
  - `terminal_branch_still_drains_events_when_llm_response_already_present`
- `crates/ploke-tui/src/rag/tests/editing_bulk_tests.rs`
  - `approve_edits_marks_mixed_result_ns_batch_as_failed`
- `crates/ploke-eval/src/runner.rs`
  - submission artifact write tests now assert the returned in-memory `fix_patch` matches the written record
- `crates/ploke-tui/src/llm/manager/session.rs`
  - `consume_repair_budget_marks_error_exhausted_after_limit`
  - `run_chat_session_can_converge_after_four_tool_arg_repairs`
  - `run_chat_session_aborts_when_a_fifth_repair_would_be_required`
- `crates/ploke-eval/src/runner.rs`
  - `run_benchmark_turn_keeps_response_when_it_arrives_before_turn_finished`
  - `run_benchmark_turn_captures_response_when_it_arrives_after_turn_finished`
- `crates/ploke-tui/tests/integration/ns_patch_completion_regression.rs`
  - `ns_patch_rejects_duplicate_same_file_entries_in_one_request`
- `crates/ploke-eval/src/runner.rs`
  - `packaging_write_failure_marks_packaging_phase_failed_after_real_repo_diff_exists`

Best Current Guess About The First Bad Boundary

- Not yet proven, but the strongest candidates are:
  - final artifact coherence when packaging fails after a real repo diff exists
  - parse/repair behavior when provider responses need more than four repair rounds before converging

Recommended Next Two Tests

1. Packaging failure artifact-coherence test

- Force submission artifact writing to fail after a real repo diff exists in a full runner path
- Expected result:
  - packaging failure is recorded distinctly from model/tool failure
  - registry, execution log, and artifact set still make it obvious that the repo diff was real

2. High-repair convergence test

- Drive the session loop through more than four repairable malformed tool calls and then a valid one using a realistic provider-style sequence
- Expected result:
  - either the current cap is confirmed sufficient for realistic traces
  - or we prove the cap is now too aggressive for providers that used to converge

Stop Conditions

- If test 1 fails, stop treating packaging as a clean post-processing step.
- If test 2 fails, treat the current repair cap as a likely regression contributor rather than a secondary policy detail.
- Same-file split `ns_patch` failure is now characterized; stop treating that contract as an unknown.

Bottom Line

- The system is not failing at one boundary.
- The current unknowns are now small enough to attack with three deterministic tests.
- Until those tests exist, live eval reruns will keep mixing provider failure, tool repair failure, apply/reporting failure, and finalization failure into one unreadable pile.
