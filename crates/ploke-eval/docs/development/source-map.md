# Prototype 1 Source Map

Status: current mdBook source-anchor index. Use this page to choose the smallest
source excerpt that proves a documentation claim.

Rule of thumb: quote the narrowest anchor that proves the claim. Broad anchors
are retained for compatibility, but overview pages should prefer carrier-only or
method-only anchors.

## Anchor selection notes

| If the claim is... | Prefer this anchor style | Avoid |
| --- | --- | --- |
| "this carrier stores these fields" | `*_carrier` or `*_struct` | an enclosing impl block |
| "this method performs the transition" | method- or branch-specific anchor such as `prototype1_message_open_lock_success` | carrier-only excerpts |
| "this box has this path/schema" | file/schema anchor | unrelated message body methods |
| "this state alias has this shape" | one R-alias anchor | all aliases at once |
| "this is an operator-facing command safety rule" | command/help anchor | implementation internals |

## Typestate core anchors

| Anchor | Source | Claim it proves | Used in |
| --- | --- | --- | --- |
| `prototype1_runtime_product` | `src/cli/prototype1_state/typestate/runtime.rs` | `Runtime` is a product over independent axes. | `prototype1/typestate.md`, `prototype1/typestate-invariants.md` |
| `prototype1_context_facts_fields` | `src/cli/prototype1_state/typestate/context.rs` | `Facts` fields keep live values in an `Option`-heavy bookkeeping bundle. | `prototype1/typestate-invariants.md` |
| `prototype1_alias_r8` | `src/cli/prototype1_state/typestate/aliases.rs` | after child-plan receive, role/plan/children axes are shaped around `Received<ChildPlan>`. | `prototype1/typestate-invariants.md` |
| `prototype1_alias_r13b_handoff_committed` | `src/cli/prototype1_state/typestate/aliases.rs` | committed handoff means retired parent plus advanced/sealed History and recorded continuation. | `prototype1/typestate-invariants.md` |
| `prototype1_branch_r12_continuation` | `src/cli/prototype1_state/typestate/aliases.rs` | R12 continuation has stopped vs handoff-committed outcomes. | `prototype1/typestate-invariants.md` |
| `prototype1_step_trait_contract` | `src/cli/prototype1_state/typestate/transition.rs` | `Step` contract composes adjacent typed edges. | `prototype1/typestate-invariants.md` |
| `prototype1_step_input` | `src/cli/prototype1_state/typestate/transition.rs` | value-first `advance` applies the same typed edge constraint. | `prototype1/typestate-invariants.md` |
| `prototype1_operation_target` | `src/loop_graph.rs` | artifact, patch-set, and artifact-set targets are distinct. | `prototype1/typestate-invariants.md` |
| `prototype1_coordinate` | `src/loop_graph.rs` | a runtime id is paired with an operation target. | `prototype1/typestate-invariants.md` |

## Parent and startup anchors

| Anchor | Source | Claim it proves | Used in |
| --- | --- | --- | --- |
| `prototype1_parent_struct` | `src/cli/prototype1_state/parent.rs` | `Parent<S>` is the concrete parent role carrier. | `prototype1/typestate.md`, `prototype1/typestate-invariants.md` |
| `prototype1_parent_startup_gate` | `src/cli/prototype1_state/parent.rs` | `Startup<Validated>` carries private startup/History evidence. | `prototype1/typestate-invariants.md` |
| `prototype1_parent_check_validation` | `src/cli/prototype1_state/parent.rs` | backend checkout validation is required before proceeding. | `prototype1/typestate-invariants.md` |
| `prototype1_parent_ready_validation` | `src/cli/prototype1_state/parent.rs` | startup evidence is validated before readiness is admitted. | `prototype1/typestate-invariants.md` |
| `prototype1_parent_accept_harness_plan` | `src/cli/prototype1_state/parent.rs` | broad-harness response handling returns `AwaitingHarnessPlan` to `Ready`. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_parent_planned_from_locked_child_plan` | `src/cli/prototype1_state/parent.rs` | retry replay rebuilds `Parent<Planned>` before child-plan unlock. | `prototype1/child-plan-authority/zero-admission-flow.md` |

## Message-box anchors

| Anchor | Source | Claim it proves | Used in |
| --- | --- | --- | --- |
| `prototype1_message_box_trait` | `src/cli/prototype1_state/inner.rs` | a message box has one lock edge and one unlock edge. | `prototype1/typestate-invariants.md` |
| `prototype1_message_trait` | `src/cli/prototype1_state/inner.rs` | message implementations own close/fail/receiver-validation logic. | `prototype1/typestate-invariants.md` |
| `prototype1_message_open_carrier` | `src/cli/prototype1_state/inner.rs` | `Open<M>` carrier fields only. | `prototype1/typestate-invariants.md` |
| `prototype1_message_open_from_sender` | `src/cli/prototype1_state/inner.rs` | opening consumes the sender role/state. | `prototype1/typestate-invariants.md` |
| `prototype1_message_open_lock_success` | `src/cli/prototype1_state/inner.rs` | successful lock closes the sender, disarms the guard, and returns `Locked<M>`. | `prototype1/typestate-invariants.md`, `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_message_open_lock_failure` | `src/cli/prototype1_state/inner.rs` | failed lock returns the message-specific failed-sender state. | `prototype1/typestate-invariants.md` |
| `prototype1_message_open_drop_guard` | `src/cli/prototype1_state/inner.rs` | armed open messages panic on silent drop. | `prototype1/typestate-invariants.md` |
| `prototype1_message_locked_carrier` | `src/cli/prototype1_state/inner.rs` | `Locked<M>` carrier fields only. | `prototype1/typestate-invariants.md` |
| `prototype1_message_locked_from_box` | `src/cli/prototype1_state/inner.rs` | replay constructs `Locked<M>` by reading the typed box. | `prototype1/typestate-invariants.md`, `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_message_locked_unlock` | `src/cli/prototype1_state/inner.rs` | unlock validates the receiver before returning `Received<M>`. | `prototype1/typestate-invariants.md`, `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_message_received_carrier` | `src/cli/prototype1_state/inner.rs` | `Received<M>` carrier fields only. | `prototype1/typestate-invariants.md` |

## Child-plan anchors

| Anchor | Source | Claim it proves | Used in |
| --- | --- | --- | --- |
| `prototype1_child_plan_message_carrier` | `src/cli/prototype1_state/parent.rs` | `ChildPlan` is the message marker. | Not currently quoted; prefer only when the marker itself is the claim. |
| `prototype1_child_plan_body_carrier` | `src/cli/prototype1_state/parent.rs` | `ChildPlanFiles` body fields include children and rejected attempts. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_child_plan_file_carrier` | `src/cli/prototype1_state/parent.rs` | `ChildPlanFile` is the typed file marker. | Not currently quoted; prefer only when the marker itself is the claim. |
| `prototype1_child_plan_lock_transition` | `src/cli/prototype1_state/parent.rs` | the lock edge is `Parent<Ready> -> Parent<Planned>`. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_child_plan_unlock_transition` | `src/cli/prototype1_state/parent.rs` | the unlock edge is `Parent<Planned> -> Parent<Selectable>`. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_child_plan_box_schema` | `src/cli/prototype1_state/parent.rs` | child-plan box path and lock/unlock association. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_child_plan_ready_receiver` | `src/cli/prototype1_state/parent.rs` | receiver validation checks box path, parent, and generation. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_broad_harness_below_min_persist_rejected_plan` | `src/cli/prototype1_state/cli_facing.rs` | below-minimum broad-harness admission persists a rejected child plan before returning the error. | `prototype1/child-plan-authority/zero-admission-flow.md` |
| `prototype1_persist_rejected_child_plan` | `src/cli/prototype1_state/cli_facing.rs` | rejected child-plan persistence writes empty children plus rejected attempts through `Open<ChildPlan>::lock`. | `prototype1/child-plan-authority/zero-admission-flow.md` |

## Handoff and History anchors

| Anchor | Source | Claim it proves | Used in |
| --- | --- | --- | --- |
| `prototype1_parent_seal_block_with_artifact` | `src/cli/prototype1_state/inner.rs` | selectable parent retires while sealing artifact-backed History. | `prototype1/typestate-invariants.md` |
| `prototype1_fs_block_store_append` | `src/cli/prototype1_state/history/stored/mod.rs` | append verifies hash/state/head before advancing projections. | `prototype1/typestate-invariants.md`, `prototype1/operator-map.md` |
| `prototype1_live_edge_r12_handoff_branch` | `src/cli/prototype1_state/live_edges.rs` | R12 delegates handoff and returns the handoff-committed branch. | `prototype1/typestate-invariants.md` |
| `prototype1_handoff_block_fields` | `src/cli/prototype1_process.rs` | handoff block opens from current lineage state and installed artifact. | `prototype1/typestate-invariants.md`, `prototype1/operator-map.md` |
| `prototype1_successor_startup_validation` | `src/cli/prototype1_process.rs` | successor rechecks sealed History runtime/artifact/identity. | `prototype1/typestate-invariants.md`, `prototype1/operator-map.md` |
| `prototype1_history_block_verify_current_artifact_tree` | `src/cli/prototype1_state/history/seal/mod.rs` | successor verifies current checkout tree against sealed artifact claim. | `prototype1/typestate-invariants.md` |
| `prototype1_history_block_verify_current_surface` | `src/cli/prototype1_state/history/seal/mod.rs` | successor verifies current surface against sealed block. | `prototype1/typestate-invariants.md` |

## Operator/reference anchors used elsewhere in the book

| Anchor | Source | Used in |
| --- | --- | --- |
| `ploke_eval_cli_trust_order_help` | `src/cli/args/root.rs` | `prototype1/operator-map.md` |
| `prototype1_walk_step_live_edge_admission` | `src/cli/args/loop_args.rs` | `prototype1/operator-map.md`, `operations/command-safety.md` |
| `ploke_eval_operator_projection_read` | `src/projection.rs` | `prototype1/operator-map.md` |
| `ploke_eval_run_msb_agent_single_command` | `src/cli/args/run.rs` | `operations/command-safety.md` |
| `ploke_eval_run_msb_agent_batch_command` | `src/cli/args/run.rs` | `operations/command-safety.md` |
| `ploke_eval_instances_dir` | `src/layout.rs` | `operations/run-lifecycle.md` |
| `ploke_eval_attempt_runs_dir` | `src/run_registry.rs` | `operations/run-lifecycle.md` |
| `model_providers_route_behavior` | `src/cli/handlers/model.rs` | `reference/provider-routing.md` |
| `campaign_direct_google_provider_normalization` | `src/campaign.rs` | `reference/provider-routing.md` |
| `campaign_resolve_direct_google_provider` | `src/campaign.rs` | `reference/provider-routing.md` |

## Broad anchors retained but usually not preferred for snippets

| Broad anchor | Prefer instead |
| --- | --- |
| `prototype1_context_facts` | `prototype1_context_facts_fields` when the claim is only the `Facts` field shape |
| `prototype1_message_open` | `prototype1_message_open_carrier`, `prototype1_message_open_from_sender`, `prototype1_message_open_lock_success`, `prototype1_message_open_lock_failure`, `prototype1_message_open_drop_guard` |
| `prototype1_message_open_lock` | `prototype1_message_open_lock_success` or `prototype1_message_open_lock_failure` when the claim is branch-specific |
| `prototype1_message_locked` | `prototype1_message_locked_carrier`, `prototype1_message_locked_from_box`, `prototype1_message_locked_unlock` |
| `prototype1_message_received` | `prototype1_message_received_carrier` when only the shape is needed |
| `prototype1_child_plan_types` | `prototype1_child_plan_message_carrier`, `prototype1_child_plan_body_carrier`, `prototype1_child_plan_file_carrier` |
| `prototype1_child_plan_transitions` | `prototype1_child_plan_lock_transition`, `prototype1_child_plan_unlock_transition` |
| `prototype1_child_plan_message_impl` | `prototype1_child_plan_ready_receiver` when the claim is receiver validation |
| `prototype1_step_trait` | `prototype1_step_trait_contract` when the claim is the trait contract, not blanket impls |
| `prototype1_parent_check` / `prototype1_parent_ready` | `prototype1_parent_check_validation`, `prototype1_parent_ready_validation` for validation-only claims |
| `prototype1_typestate_aliases_and_branches` | individual R-state or branch anchors |
| `prototype1_loop_graph_vocabulary` | individual graph vocabulary anchors |

## Remaining source-anchor backlog

- Add page-specific projection-authority anchors when operator docs repeat claims
  about scheduler, branch, node, runner-result, and CLI table files being weaker
  than handoff authority.
- Add a concise binary-provenance source comment before quoting operator-started
  parent binary policy as a code-backed invariant.
