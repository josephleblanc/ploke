//! Prototype 1 parent-run execution boundary.
//!
//! This module is the landing area for the live parent execution path that is
//! currently embedded in `cli_facing.rs`. The first job is not to add another
//! report or status view. The first job is to recover the causal path and move
//! executable authority behind a small run-core boundary.
//!
//! ## Current Parent Path
//!
//! The current `prototype1-state` parent turn performs these operations in one
//! broad CLI-facing function:
//!
//! 1. Resolve the active parent checkout.
//!    - choose `repo_root`
//!    - resolve `campaign_id`
//!    - load the admitted run shape/profile
//!    - open the campaign manifest and transition journal
//!    - optionally initialize parent identity for generation zero
//!
//! 2. Admit the runtime as the active parent for this turn.
//!    - load either a successor handoff invocation or the checkout-carried
//!      parent identity
//!    - validate predecessor History/startup evidence when this runtime was
//!      launched as a successor
//!    - enter the live `Parent<Ready>` state
//!    - append parent-start evidence and resource samples
//!
//! 3. Establish the parent baseline.
//!    - load the resolved campaign configuration
//!    - verify baseline closure state
//!    - run or load the baseline evidence used to compare children
//!
//! 4. Produce or load the child plan.
//!    - use the admitted candidate-generation mode
//!    - publish a parent-owned child plan message when needed
//!    - convert planned children into attempt inputs
//!    - carry rejected edit-surface attempts separately from runnable children
//!
//! 5. Apply child budget and schedule policy.
//!    - derive the child budget and schedule mode from the admitted run policy
//!      for complete runs
//!    - derive a single-child inspection budget for non-complete debug cuts
//!    - choose adaptive or full-batch fanout
//!
//! 6. Execute child attempts.
//!    - each runnable child enters the C1-C5 path:
//!      `ChildFiles -> C1 -> C2 -> C3 -> C4 -> C5`
//!    - C1 materializes the child Artifact
//!    - C2 builds the child Runtime
//!    - C3 spawns the child Runtime
//!    - C4 observes the child terminal result
//!    - C5 carries terminal treatment evidence for parent comparison
//!
//! 7. Compare child evidence and assemble successor-selection input.
//!    - successful children are compared against the parent baseline
//!    - rejected/failed children remain evidence, but should not be confused
//!      with an allowed successor
//!    - selection input should be a structured child-attempt result, not a
//!      string outcome assembled by the runner
//!
//! 8. Select a successor candidate.
//!    - current-generation candidates and admitted History candidates may both
//!      be consulted depending on the configured strategy
//!    - selection is evidence, not authorization to continue
//!    - historical traversal must not silently become generation progress
//!
//! 9. Decide continuation.
//!    - this is the missing authority boundary that caused the runaway run
//!    - continuation must consume the admitted run policy and persisted run
//!      inventory before any successor handoff can be sealed or spawned
//!    - an allowed continuation must prove at least:
//!      selected node is the direct child of the current parent, next
//!      generation is within budget, total nodes are within budget, child
//!      fanout respected policy, and the parent/selected successor has not
//!      already spent its one handoff
//!
//! 10. Hand off to the next parent.
//!     - only `Continuation<Allowed>` may reach successor installation,
//!       History sealing, successor invocation writing, and process spawn
//!     - `Continuation<Stopped>` should still be recorded as evidence, but it
//!       must not call the successor spawn path
//!
//! 11. Project the parent-turn result.
//!     - CLI output, monitor records, compact status, and debug strings are
//!       projections of the run-core result
//!     - projections must not mint source facts or decide continuation
//!
//! ## Extraction Rule
//!
//! New behavior should not be added to the existing CLI-facing parent runner.
//! Move authority into `run::core` in small slices, beginning with the
//! continuation gate. The CLI layer should eventually parse arguments, call the
//! run core, record failure for successor turns, and render the selected output
//! format.
//!
//! ## Intended Core Shape
//!
//! The run core should preserve role/state structure instead of flattening it
//! into broad helpers:
//!
//! ```text
//! Parent<Ready>
//!   -> ChildPlan<Admitted>
//!   -> ChildAttempt<Observed>*
//!   -> SuccessorSelection<SealedEvidence>
//!   -> Continuation<Allowed | Stopped>
//!   -> Parent<Retired> | Parent<Ready>
//! ```
//!
//! The immediate safety boundary is the post-child transition:
//!
//! ```text
//! Parent<Selectable>
//!   + SuccessorSelection
//!   + AdmittedRunPolicy
//!   + PersistedRunInventory
//!   -> Continuation<Allowed | Stopped>
//! ```
//!
//! This transition is where generation limits, total-node limits, direct-child
//! successor relation, fanout policy, and one-shot handoff spending belong.
//! Child execution typestates C1-C5 are necessary, but they do not authorize
//! parent-to-successor continuation.
//!
//!
//! Here is the live execution path inventory, split by role. This is the path to extract into `prototype1_state/run/core.rs`.
//!
//! **Entry / Dispatch**
//! - [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1193): `LoopCommand::run`
//! - [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1203): `Prototype1RunnerCommand::run`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7820): `impl Prototype1StateCommand`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7834): `Prototype1StateCommand::run`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7848): `Prototype1StateCommand::run_turn`
//!
//! **Parent Startup**
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6088): `current_dir_as_repo_root`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6110): `resolve_prototype1_state_campaign`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:779): `Prototype1StateRunShape::resolve`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7542): `initialize_prototype1_parent_identity`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7603): `resolve_prototype1_parent_identity`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:327): `validate_prototype1_successor_continuation`
//! - [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:434): `Parent<Unchecked>`
//! - [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:568): `Parent<Checked>`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7620): `acknowledge_prototype1_state_handoff`
//!
//! **Parent Baseline / Child Plan**
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:357): `ensure_prototype1_baseline_closure_state`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:368): `establish_parent_baseline`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6232): `resolve_child_plan`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:913): `run_parent_target_selection`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:937): `run_legacy_parent_target_selection`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1042): `run_tui_edit_surface_parent_target_selection`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1061): `publish_tui_edit_surface_child_plan`
//! - [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:914): `Parent<Ready>`
//! - [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:951): `Parent<Planned>`
//! - [parent.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/parent.rs:957): `Parent<Selectable>`
//!
//! **Parent-Side Child Fanout**
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6704): `run_child_fanout`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6827): `run_adaptive_child_fanout`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6401): `run_planned_child`
//!
//! **Child Attempt C1-C5**
//! - [c1.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c1.rs:386): `C1::from_child_plan`
//! - [c1.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c1.rs:497): `MaterializeBranch`
//! - [c2.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c2.rs:239): `BuildChild`
//! - [c2.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c2.rs:251): `impl Intervention<C2, C3> for BuildChild`
//! - [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:389): `SpawnChild`
//! - [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:401): `impl Intervention<C3, C4> for SpawnChild`
//! - [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:649): `wait_for_ready`
//! - [c4.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c4.rs:151): `ObserveChild`
//! - [c4.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c4.rs:210): `impl Intervention<C4, C5> for ObserveChild`
//!
//! **Child Runtime / Leaf Runner**
//! - [invocation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/invocation.rs:585): `write_child_invocation`
//! - [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1203): `Prototype1RunnerCommand::run`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1509): `execute_prototype1_runner_invocation`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1654): `run_prototype1_resolved_branch_treatment`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1440): `build_treatment_failed_runner_result`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1468): `build_succeeded_runner_result`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1491): `record_attempt_runner_result`
//!
//! **Selection / Continuation**
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7182): `ParentSelection::new`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7207): `ParentSelection::current_generation_candidates`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7295): `ParentSelection::select_successor`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6904): `continuation_disposition_for_selection`
//! - [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7412): `select_artifact_for_handoff`
//! - [scheduler.rs](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs:1022): `decide_continuation_with_selection` currently exists but is not the live authority path.
//!
//! **Successor Handoff**
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:1000): `spawn_and_handoff_prototype1_successor`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:429): `prepare_prototype1_active_successor_runtime`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:446): `install_prototype1_successor_artifact`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:524): `install_committed_successor_artifact`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:844): `spawn_prototype1_successor`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:880): `exec_prototype1_successor`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:956): `wait_for_prototype1_successor_ready`
//! - [invocation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/invocation.rs:605): `write_successor_invocation_for_retired_parent`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:243): `record_prototype1_successor_ready`
//! - [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:277): `record_prototype1_successor_completion`
//!
//! **History / Crown Touchpoints**
//! - [inner.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/inner.rs:189): `impl LockCrown for Parent<Selectable>`
//! - [history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/history.rs:4991): `SealBlock`
//! - [history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/history.rs:5573): `impl Crown<Ruling>`
//! - [history.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/history.rs:5677): `impl Crown<Locked>`
//!
//! The extraction target should probably start with only the bold spine: `run_turn`, `run_child_fanout`, `run_adaptive_child_fanout`, `run_planned_child`, `ParentSelection`, `continuation_disposition_for_selection`, and `spawn_and_handoff_prototype1_successor`.

mod core;
