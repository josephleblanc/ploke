# Prototype 1 Broad Headless TUI Turn-Boundary Apply Review

Review target: commit `4af3e2fd` (`prototype1: apply broad harness edits after turn completion`), focused on `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`.

## Findings

1. **High: the adapter waits for a `ChatTurnFinished`, but not the matching one.**

   `submit_prompt` returns the prompt/user message id, but `start_attempt_runtime` discards it before entering `run_attempt` ([tui_adapter.rs:192](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:192), [tui_adapter.rs:999](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:999)). `run_attempt` then accepts any `ToolCallCompleted` with a proposal payload and any `ChatTurnFinished` event ([tui_adapter.rs:302](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:302), [tui_adapter.rs:517](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:517)). It records staged ids from those events and applies them when the first completed turn arrives ([tui_adapter.rs:323](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:323), [tui_adapter.rs:390](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:390), [tui_adapter.rs:543](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:543), [tui_adapter.rs:591](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:591)).

   The TUI events carry the needed relation: `ToolCallCompleted` has `request_id`, `parent_id`, and `call_id`, and `ChatTurnFinished` has `session_id`, `request_id`, `parent_id`, and `assistant_message_id` ([events.rs:87](../../../crates/ploke-tui/src/app_state/events.rs:87), [events.rs:103](../../../crates/ploke-tui/src/app_state/events.rs:103)). `EditProposal` and `CreateProposal` also store `request_id`, `parent_id`, and `call_id` ([core.rs:360](../../../crates/ploke-tui/src/app_state/core.rs:360), [core.rs:392](../../../crates/ploke-tui/src/app_state/core.rs:392)). The adapter does not check those fields against an active-turn carrier. A stale or foreign event on the same runtime bus can make the attempt end early or apply a proposal that was not produced by the active model turn. The next patch should carry an `ActiveTurn`/`ActivePrompt` relation and filter every tool/proposal/turn event through it before staging or applying.

2. **High: multi-proposal apply is not atomic; a later failure leaves earlier mutations in the workspace and the retry path can lose that evidence.**

   After turn completion, the adapter applies staged items sequentially and immediately returns `RetryFailure` on the first failed item ([tui_adapter.rs:591](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:591), [tui_adapter.rs:596](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:596), [tui_adapter.rs:602](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:602)). At that point, earlier items may already have reached `Applied` and been recorded as applied attempts ([tui_adapter.rs:660](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:660), [tui_adapter.rs:740](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:740)). The outer loop treats this as an ordinary retry and starts a fresh runtime against the same workspace ([tui_adapter.rs:81](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:81), [tui_adapter.rs:102](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:102)).

   This is especially risky with stash transfer: downstream transfer uses only the final `HeadlessTerminal::Applied.changed_paths` ([cli_facing.rs:1372](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1372)). If attempt 1 partially mutates the workspace, attempt 2 later succeeds with different paths, the terminal evidence can omit the first mutation. Backend diff validation may reject the dirty or unexpected workspace later, but the adapter has already hidden the causal partial-apply state. The invariant should be either atomic apply for the staged candidate or a terminal partial-apply failure that reports all already-applied paths and does not retry on a mutated workspace without rollback.

3. **High: create-file apply can report `Applied` even when no file was created, and the adapter trusts that as changed evidence.**

   `approve_creations` records per-file create errors but unconditionally sets `proposal.status = EditProposalStatus::Applied` ([editing.rs:875](../../../crates/ploke-tui/src/rag/editing.rs:875), [editing.rs:900](../../../crates/ploke-tui/src/rag/editing.rs:900), [editing.rs:907](../../../crates/ploke-tui/src/rag/editing.rs:907)). The adapter then treats `Applied` as success and returns `updated.files` as changed paths ([tui_adapter.rs:740](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:740)). That can produce terminal evidence saying a file changed when `applied == 0` in the tool result. Backend admission has a later real-diff check, but terminal evidence and executor metadata are already misleading. The create apply status needs to distinguish full success, partial success, and zero-applied failure before the adapter can trust it.

4. **Medium: repaired tool failures are collapsed into one string and then ignored if the turn later stages anything.**

   Each `ToolCallFailed` overwrites `pending_retry` ([tui_adapter.rs:460](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:460), [tui_adapter.rs:479](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:479)). If the turn completes and at least one staged item exists, the adapter only logs `recovered_tool_failure_before_apply` and proceeds ([tui_adapter.rs:541](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:541), [tui_adapter.rs:579](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:579)). This matches the intended repair case, but it does not preserve whether the failed call was an invalid-argument repair, a provider/tool infrastructure failure, a timeout, or a failed required validation step. Provider-unavailable detection currently depends on assistant error message text ([tui_adapter.rs:481](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:481), [tui_adapter.rs:784](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:784)). Keep all failed call ids/error codes in evidence, and only classify a failure as recovered when the active turn semantics make that true.

5. **Medium: protected-surface checks happen before apply, but the adapter does not rebind or recheck the current proposal at apply time.**

   The new staging path checks proposal paths before pushing a staged id ([tui_adapter.rs:369](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:369), [tui_adapter.rs:437](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:437)). `apply_edit` and `apply_create` then approve by id and trust the current registry status/path fields ([tui_adapter.rs:633](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:633), [tui_adapter.rs:713](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:713)). Backend admission still validates the actual diff against policy ([backend.rs:1641](../../../crates/ploke-eval/src/cli/prototype1_state/backend.rs:1641)), so protected-core writes should not be admitted. The adapter boundary is still weaker than it claims because the staged id is not a typed checked proposal; it should re-read, rebind to the active turn, and re-run the path policy immediately before approve.

6. **Medium: terminal evidence cannot faithfully identify multiple applied proposal/create items.**

   For multiple staged items, `HeadlessTerminal::Applied` stores only the first staged id as `proposal_id` while `changed_paths` is a union across all applied items ([tui_adapter.rs:586](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:586), [tui_adapter.rs:613](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:613), [tui_adapter.rs:1490](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1490)). The evidence schema and executor metadata also expose one proposal id ([tui_adapter.rs:1712](../../../crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1712), [cli_facing.rs:1462](../../../crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1462)). The attempts list contains per-item applied rows, but the terminal and transaction handle do not. For a multi-file candidate staged through multiple tool calls, terminal evidence should carry `applied_items: [{kind, id, request_id, parent_id, call_id, paths}]` or an equivalent typed carrier.

## Direct Answers

- The adapter now waits until a `ChatTurnFinished(outcome = "completed")` before approving/applying staged edit/create proposals. It does not yet prove that the turn is the matching model turn.
- Staged proposal ids are not sufficiently associated with the active model turn/session/request. The raw data exists in TUI events and proposal records, but the adapter does not enforce the relation.
- Protected path checks are adequate as an early rejection and backend admission has the stronger final diff guard. The adapter apply boundary should still recheck the currently loaded proposal before approve.
- Repaired tool failures are handled in the happy repair case, but the representation is too lossy to prove fatal failures are not hidden.
- Duplicate staged events for the same `Staged` id are deduplicated in memory. Multiple staged proposals are applied sequentially, not atomically. Create-file proposals are supported, but zero/partial create failures can be mislabeled as applied. Stale proposals become retry failures, but partial prior mutations are not rolled back.
- Terminal evidence accurately reports the union of paths only for the all-applied sequential case. It does not faithfully identify all applied item ids and it can become misleading on create failure or partial multi-item failure.

## Test Gaps

- Deterministic adapter test where a foreign `ChatTurnFinished` arrives before the active prompt's turn and must be ignored.
- Deterministic adapter test where a proposal payload id exists but `proposal.request_id`, `parent_id`, or `call_id` does not match the active turn and must not stage/apply.
- Multi-proposal success test that proves all applied items are represented in terminal evidence, not only the first id.
- Partial multi-proposal failure test: first item applies, second fails. Expected behavior should be explicit terminal partial failure or rollback, not ordinary retry with hidden workspace mutation.
- Create-file zero-applied and partial-applied tests. The adapter should not return `HeadlessTerminal::Applied` when the create result says no files were created.
- Repaired invalid-tool-call test versus fatal provider/tool failure test, using typed failure metadata rather than substring-only provider detection.
- Apply-time protected-path recheck test, especially for proposal records whose current paths differ from the paths observed when first staged.

## Invariants For The Next Patch Or Live Smoke

- Active-turn carrier: `submit_prompt` must mint or return the id needed to bind `PromptConstructed`, `ToolCallRequested`, `ToolCallCompleted`, `ToolCallFailed`, staged proposals, and `ChatTurnFinished`.
- A staged item is not just `Edit(Uuid)` or `Create(Uuid)`; it must carry kind, proposal/request id, parent id, call id, checked paths, and the active-turn binding.
- Apply should be all-or-terminal-partial. Do not silently retry after mutating the candidate workspace unless rollback is proven.
- Terminal evidence should be a projection of applied item carriers, not a single proposal id plus path union.
- Create-file status must encode full/partial/zero apply before the eval adapter treats it as changed evidence.
- Backend diff validation remains the final admission authority; adapter evidence should still avoid claiming changes that the adapter did not actually prove.

## Verification

Ran bounded local tests:

```text
cargo test -p ploke-eval tui_adapter --lib 2>&1 | tail -n 80
```

Result: passed (`18 passed; 0 failed; 7 ignored; 578 filtered out`). The covered tests are mostly prompt/evidence/classifier tests; they do not cover the active-turn binding or multi-proposal apply failure cases above.
