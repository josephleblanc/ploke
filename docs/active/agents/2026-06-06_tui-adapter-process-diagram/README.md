# 2026-06-06 — tui_adapter process diagram

Short description: source-grounded diagram packet for the Prototype 1 broad headless `tui_adapter` path, including the interaction between `ploke-eval`, `ploke-tui`, the event system, and persistence/history surfaces.

Related files in this packet:
- `tui-adapter-process.mmd` — Mermaid source.
- `tui-adapter-process.png` — rendered Mermaid PNG.
- `tui-adapter-process.excalidraw` — editable Excalidraw scene.

## Scope

The diagram follows the broad harness path used by Prototype 1:

1. `ploke-eval` publishes a broad edit harness request and allocates fresh slots.
2. Each slot calls `run_broad_headless_tui_attempt_with_options`.
3. `tui_adapter` starts a headless `ploke-tui` runtime, submits the prompt, observes events, settles staged edit/create proposals, runs declared validation, and classifies the terminal outcome.
4. `ploke-eval` writes diagnostics/turn-live records, only writes a submitted result for `HeadlessTerminal::Applied`, then attempts backend admission and child-plan publication.
5. Child evidence and node projections feed later Prototype 1 evaluation/selection; sealed `History` blocks are an append-only store used by traversal/selection, not a free-standing status field.

## Success state

Primary success is narrow:

- `HeadlessTerminal::Applied` from `tui_adapter`.
- `finish_broad_headless_tui_attempt` writes `SubmittedBroadHarnessResult`.
- `try_admit_request_result` and `GitWorktreeBackend::admit_submitted_broad_harness_result` accept the request binding, workspace diff, surface policy, and persisted artifact.
- `publish_broad_harness_child_plan_from_admitted_batch` has at least `child_budget.min` admitted transactions and publishes child plan/node projections.

Diagram label: `SUCCESS — Submitted result admitted; child plan published`.

## Failure states shown

The diagram groups failures by boundary:

- Headless terminal failures:
  - `Exhausted`
  - `CompletedWithoutEdit`
  - `NoEdit`
  - `ToolFailed`
  - `ProviderUnavailable`
  - `ContextUnavailable`
  - `AppliedValidationFailed`
  - `AppliedValidationMissing`
  - `AppliedTurnAborted`
  - `AppliedTimedOut`
  - `TimedOut`
- Submission/admission failures:
  - no submitted result JSON
  - result/request binding mismatch
  - source/candidate workspace mismatch
  - dirty, stale, no-change, invalid-path, out-of-policy, or unexpected-dirty candidate workspace
- Batch-level failure:
  - admitted children are fewer than `child_budget.min`, so the parent is projected failed and rejected surface-attempt evidence is persisted.

## Source anchors

Primary source files inspected for this diagram:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `publish_broad_edit_harness_request`
  - `admit_broad_harness_batch`
  - `run_broad_slot_for_admission`
  - `run_broad_headless_tui_attempt_with_options`
  - `finish_broad_headless_tui_attempt`
  - `try_admit_request_result`
  - `write_broad_headless_tui_diagnostics`
  - `write_broad_headless_tui_turn_live_bundle`
  - `publish_broad_harness_child_plan_from_admitted_batch`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`
  - `run_headless_with_model_capture_responses`
  - `run_headless_with_model_inner`
  - `start_attempt_runtime`
  - `submit_prompt`
  - `run_attempt`
  - `settle_staged_batch`
  - `wait_for_selected`
  - `wait_for_refresh`
  - `run_contract_validations`
  - `classify_applied_terminal`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness_io.rs`
  - `HeadlessRun`
  - `HeadlessAttemptResult`
  - `HeadlessTerminal`
  - `HeadlessRun::agent_turn_artifact_record`
- `crates/ploke-eval/src/cli/prototype1_state/backend/harness_ingestion.rs`
  - `GitWorktreeBackend::validate_tui_attempt`
  - `GitWorktreeBackend::admit_submitted_broad_harness_result`
- `crates/ploke-eval/src/runner/mod.rs`
  - `WorkspaceTuiRuntime`
  - `setup_workspace_tui_runtime_with_read_roots`
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
  - `TestRuntime`
  - `RelayStateCmd::run_relay`
  - actor spawn methods
- `crates/ploke-tui/src/lib.rs`
  - `AppEvent`
  - `AppEvent::priority`
- `crates/ploke-tui/src/event_bus/mod.rs`
  - `EventBus`
  - `run_event_bus`
- `crates/ploke-tui/src/app_state/events.rs`
  - `SystemEvent`
- `crates/ploke-tui/src/app_state/commands.rs`
  - `StateCommand`
- `crates/ploke-tui/src/app/mod.rs`
  - `App::pump_pending_events`
- `crates/ploke-eval/src/cli/prototype1_state/history/stored/mod.rs`
  - `BlockStore`
  - `FsBlockStore`
- `crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs`
  - `History::candidates`
