# Prototype 1 stale broad-harness handoff experiments

Date: 2026-06-07

Campaign under analysis:
`p1-historyfix-handoff-5g1x2-a2-20260607-155010`

## E1: Missing broad-harness checkout during successor install

Status: regression added, focused verification passed

Hypothesis: a selected historical broad-harness artifact should remain
installable from its committed artifact branch even after the provisional
edit-harness checkout has been removed.

Failure evidence:

- Transition journal selected `node-2df04e70d97c9304` /
  `branch-7c33506afa12a2f3`.
- The sealed artifact still pointed at
  `prototype1/workspaces/edit-harness/node-903feb19806d1dd8`.
- That checkout was missing, while the committed artifact branch still existed.
- `install_prototype1_successor_artifact` failed before branch verification.

Regression:

- `successor_artifact_workspace_recovers_missing_broad_harness_checkout_from_branch`
  creates a committed broad-harness artifact branch, deletes or omits the
  provisional checkout path, and asserts successor artifact preparation can
  verify the target from the committed branch.

Fix direction:

- Keep `child_artifact_workspace` strict for normal child artifact paths.
- Add a successor-install-only fallback that recognizes missing campaign-owned
  broad-harness edit workspaces and constructs the workspace handle from the
  committed artifact branch.

## E2: Active parent historical traversal cycle

Status: regression added, focused verification passed

Hypothesis: when historical traversal is enabled, the active parent itself must
not be eligible for successor selection. Re-selecting the current parent creates
a traversal cycle and can point handoff at already-consumed transient artifact
state.

Failure evidence:

- The failed run selected a historical candidate after parent-successor
  handoff had already succeeded twice.
- The selected branch had been promoted earlier, but its sealed artifact still
  referenced an older transient edit-harness path.

Regressions:

- `historical_selection_rejects_already_active_parent_cycle` asserts the live
  continuation decision stops if the selected node and branch match the active
  parent identity.
- `history_candidate_filter_excludes_active_parent_before_sampling` asserts the
  active parent candidate is removed from History traversal candidates before
  stochastic scoring and sampling.

Fix direction:

- Filter the active parent out of History candidates before constructing the
  traversal selection candidate set.
- Keep a downstream `StopHistoricalTraversalCycle` guard as a defensive
  invariant check if an already-active parent slips through another projection
  path.

## E3: Prototype 1 setup embedding overrides dropped before baseline eval

Status: source fix added, focused verification passed, fresh live loop completed

Hypothesis: `prototype1-setup --embedding-model-id ... --embedding-provider ...`
must persist those values into campaign eval policy and the closure/batch runner
must forward them into the single-agent eval run that performs baseline
indexing.

Failure evidence:

- Campaign `p1-handofffix-5g1x2-a2-20260607-190702` failed before child
  planning.
- The baseline batch summary recorded an embedding preflight failure for
  `mistralai/codestral-embed-2505`.
- Source inspection showed `EvalCampaignPolicy` had no embedding fields and
  `RunMsbAgentBatchRequest` constructed `RunMsbAgentSingleRequest` with
  `embedding_model_id: None` and `embedding_provider: None`.

Regressions:

- `agent_batch_request_preserves_embedding_overrides`
- `loop_prototype1_setup_command_parses`
- `prototype1_setup_campaign_manifest_preserves_embedding_overrides`
- `prototype1_eval_set_id_includes_embedding_overrides`

Fix direction:

- Carry embedding model/provider through setup admission, campaign eval policy,
  closure eval execution, agent batch request execution, and eval-set identity.
- Keep the default embedding route unchanged when no override is provided.

Follow-up live evidence:

- Campaign `p1-handofffix-embed-5g1x2-a2-20260607-192954` persisted
  `perplexity/pplx-embed-v1-4b` / `perplexity` in the setup manifest and used
  that route during baseline and child indexing.
- The fresh continuous `prototype1-state` run advanced through several
  parent-to-successor handoffs, including continuation from a rejected parent,
  and stopped cleanly at `stop_historical_traversal_budget` after generation-4
  children completed.

## E4: Historical traversal duplicate and expanded-candidate policy

Status: source fix added, focused verification passed

Hypothesis: all-history traversal should not treat a previously expanded
parent, or the same node/branch admitted through both History and current
generation payloads, as a fresh equivalent successor coordinate.

Rerun evidence:

- Campaign `p1-handofffix-embed-5g1x2-a2-20260607-192954` completed without a
  runtime blocker, but stochastic `score_child_prop` revisited
  `node-8167e33daa3b9bc6` after it already had two children.
- The later sealed formula row contained duplicate rows for
  `node-d05350cdb42e3185` and `node-7815b0481a271a5e`.
- The terminal selection chose rejected `node-7815b0481a271a5e` as the final
  coordinate before stopping at `stop_historical_traversal_budget`.

Source boundary:

- `Candidates::with_current_generation` now removes all-history payloads with
  the same node/branch coordinate before adding a decision-grade
  current-generation payload.
- `Candidates::traverse_with_policy` now excludes candidates that already have
  successful children before invoking the traversal strategy.
- `TraversalEvidence.child_counts` now persists the pre-pruning expansion counts
  used during live scoring, so formula/replay does not recompute different
  weights from the pruned `considered` list.
- `score_child_prop_calculation_with_set` now uses the same candidate-plus-parent
  child count helper used by the frontier path.

Regressions:

- `traversal_replaces_matching_history_candidate_with_current_generation_payload`
- `traversal_does_not_replace_history_with_ineligible_current_duplicate`
- `traversal_excludes_already_expanded_candidates`
- `score_child_prop_weights_include_parent_expansion_penalty`
- `score_child_prop_formula_uses_persisted_expansion_counts_after_pruning`
- `imp_at_k_score_does_not_resurrect_expanded_parent`

## Verification commands

Focused tests:

```bash
RUSTFLAGS=-Awarnings cargo test -p ploke-eval historical_selection_rejects_already_active_parent_cycle -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval history_candidate_filter_excludes_active_parent_before_sampling -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval successor_artifact_workspace_recovers_missing_broad_harness_checkout_from_branch -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval broad_harness_request_id_projection_preserves_retry_suffix -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval agent_batch_request_preserves_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval loop_prototype1_setup_command_parses -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval prototype1_setup_campaign_manifest_preserves_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval prototype1_eval_set_id_includes_embedding_overrides -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval traversal_does_not_replace_history_with_ineligible_current_duplicate -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval score_child_prop_formula_uses_persisted_expansion_counts_after_pruning -- --nocapture
RUSTFLAGS=-Awarnings cargo test -p ploke-eval successor_selection::traversal -- --nocapture
RUSTFLAGS=-Awarnings cargo check -p ploke-eval
```
