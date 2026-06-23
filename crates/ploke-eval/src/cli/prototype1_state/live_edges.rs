//! Canonical live transition edges for the Prototype 1 parent runtime.
//!
//! These constructors are shared by the existing CLI path and the debug walk
//! server so both advance through the same typed transitions.

use std::fs;

use ploke_core::EXECUTION_DEBUG_TARGET;
use tracing::{debug, info};

use crate::{
    CampaignOverrides, ResolvedCampaignConfig,
    cli::{
        InspectOutputFormat, Prototype1StateStopAfter,
        prototype1_process::{
            record_prototype1_successor_completion, record_prototype1_successor_ready,
            spawn_and_handoff_prototype1_successor, validate_prototype1_successor_continuation,
        },
        prototype1_state::{
            backend::GitWorktreeBackend,
            cli_facing::{
                ParentSelection, PlannedChildren, Prototype1StateReport, Prototype1StateRunShape,
                append_parent_target_sample, campaign_manifest_path_for_id,
                current_dir_as_repo_root, ensure_prototype1_baseline_closure_state,
                establish_parent_baseline_for_id, initialize_prototype1_parent_identity,
                live_successor_continuation_decision, outcome_for_report,
                prototype1_state_report_path, prototype1_state_successor_handoff_mode,
                prototype1_state_transition_error, record_active_prototype1_monitor_target,
                resolve_campaign_config_for_id, resolve_child_plan_for_id,
                resolve_parent_policy_budget, resolve_prototype1_parent_identity,
                resolve_prototype1_state_campaign, run_adaptive_child_fanout, run_child_fanout,
                same_existing_path, select_artifact_for_handoff, traversal_metric_inputs,
            },
            eval_store::{ConfiguredEvalStore, EvalStore, ParentStartedEvidence},
            event::RecordedAt,
            invocation::{self, InvocationAuthority, SuccessorCompletionStatus},
            journal::{self, JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            observe,
            parent::{Check, Genesis, Parent, Predecessor, Startup, Unchecked},
            profile::EvalStorageBackend,
            successor::Record as SuccessorRecord,
            typestate,
        },
    },
    intervention::{Prototype1ChildBudget, Prototype1ChildScheduleMode, RecordStore},
    spec::PrepareError,
};

fn eval_storage_backend_name(backend: EvalStorageBackend) -> &'static str {
    match backend {
        EvalStorageBackend::Fs => "fs",
        EvalStorageBackend::Database => "database",
        EvalStorageBackend::DualStrict => "dual-strict",
    }
}

// ANCHOR: prototype1_live_edges
/// Resolve command-derived context and open the transition journal.
///
/// This is the first live edge: it consumes raw CLI input (`R0`) and produces
/// collected campaign/run context (`R1`) without admitting a parent yet.
// ANCHOR: prototype1_live_edge_r0_to_r1
pub(crate) fn r0_to_r1(
    r0: typestate::R0,
) -> Result<typestate::R1<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let command = r0.into_command();
    let repo_root = if let Some(path) = command.repo_root.clone() {
        path
    } else {
        current_dir_as_repo_root()?
    };
    let campaign_id = resolve_prototype1_state_campaign(&command, &repo_root)?;
    record_active_prototype1_monitor_target(&campaign_id, &repo_root);
    let manifest_path = campaign_manifest_path_for_id(&campaign_id)?;
    let run_shape = Prototype1StateRunShape::resolve(&command, &manifest_path)?;
    let resolved_campaign =
        resolve_campaign_config_for_id(&campaign_id, &CampaignOverrides::default())?;
    ensure_prototype1_baseline_closure_state(&resolved_campaign)?;
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let journal = PrototypeJournal::new(journal_path.clone());

    Ok(typestate::R1::from_collected(
        typestate::context::Collected::new(
            command,
            repo_root,
            campaign_id,
            manifest_path,
            run_shape,
            resolved_campaign,
            journal_path,
            journal,
        ),
    ))
}
// ANCHOR_END: prototype1_live_edge_r0_to_r1

/// Branch from collected context into parent-identity initialization or lookup.
///
/// `R2a` is the `--init-parent-identity` inspection/setup branch; `R3` carries
/// an existing resolved `ParentIdentity` for a normal turn.
// ANCHOR: prototype1_live_edge_r1_to_r2a_or_r3
pub(crate) fn r1_to_r2a_or_r3(
    r1: typestate::R1<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R1Branch<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::context::CollectedParts {
        command,
        repo_root,
        campaign_id,
        manifest_path,
        run_shape,
        campaign_config: resolved_campaign,
        journal_path,
        journal,
        handoff_invocation: _,
        facts,
    } = r1.into_collected().into_parts();

    if command.init_parent_identity {
        let identity = initialize_prototype1_parent_identity(
            &command,
            &campaign_id,
            &manifest_path,
            &repo_root,
        )?;
        let collected = typestate::context::Collected::new(
            command,
            repo_root,
            campaign_id,
            manifest_path,
            run_shape,
            resolved_campaign,
            journal_path,
            journal,
        )
        .with_facts(facts);
        return Ok(typestate::R1Branch::R2a(
            typestate::R2a::from_collected_identity(collected, identity),
        ));
    }

    let parent_identity = if let Some(invocation_path) = command.handoff_invocation.as_deref() {
        match invocation::load_executable(invocation_path)? {
            InvocationAuthority::Successor(invocation) => {
                validate_prototype1_successor_continuation(&invocation, &manifest_path)?
            }
            InvocationAuthority::Child(_) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "handoff invocation '{}' is a child invocation, expected successor",
                        invocation_path.display()
                    ),
                });
            }
        }
    } else {
        resolve_prototype1_parent_identity(&campaign_id, &repo_root)?
    };
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = if command.handoff_invocation.is_some() { "successor_invocation" } else { "artifact_identity" },
        transition = if command.handoff_invocation.is_some() { "SuccessorInvocation->ParentIdentity" } else { "active_checkout->ParentIdentity" },
        campaign = %campaign_id,
        parent_id = %parent_identity.parent_id(),
        node_id = %parent_identity.node_id(),
        generation = parent_identity.generation(),
        branch_id = %parent_identity.branch_id(),
        "resolved active parent identity"
    );
    let collected = typestate::context::Collected::new(
        command,
        repo_root,
        campaign_id,
        manifest_path,
        run_shape,
        resolved_campaign,
        journal_path,
        journal,
    )
    .with_facts(facts);
    Ok(typestate::R1Branch::R3(
        typestate::R3::from_collected_identity(collected, parent_identity),
    ))
}
// ANCHOR_END: prototype1_live_edge_r1_to_r2a_or_r3

/// Load the resolved parent identity as `Parent<Unchecked>`.
///
/// Successor handoff invocations contribute the runtime id; genesis/normal
/// parent starts load from the active checkout identity.
// ANCHOR: prototype1_live_edge_r3_to_r4a
pub(crate) fn r3_to_r4a(
    r3: typestate::R3<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::R3Parts {
        collected,
        parent_identity,
    } = r3.into_parts();
    let parts = collected.into_parts();
    let parent = if let Some(invocation_path) = parts.command.handoff_invocation.as_deref() {
        let runtime_id = match invocation::load_executable(invocation_path)? {
            InvocationAuthority::Successor(invocation) => {
                let invocation_campaign_id = invocation.campaign_id().clone();
                if invocation_campaign_id != parts.campaign_id {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "handoff invocation campaign '{}' does not match command campaign '{}'",
                            invocation_campaign_id, parts.campaign_id
                        ),
                    });
                }
                if invocation.node_id() != parent_identity.node_id() {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "handoff invocation node '{}' does not match parent identity node '{}'",
                            invocation.node_id(),
                            parent_identity.node_id()
                        ),
                    });
                }
                invocation.runtime_id()
            }
            InvocationAuthority::Child(_) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "handoff invocation '{}' is a child invocation, expected successor",
                        invocation_path.display()
                    ),
                });
            }
        };
        Parent::<Unchecked>::load_with_runtime_id(
            &parts.manifest_path,
            parent_identity,
            runtime_id,
        )?
    } else {
        Parent::<Unchecked>::load(&parts.manifest_path, parent_identity)?
    };
    Ok(typestate::R4a::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r3_to_r4a

/// Validate startup evidence and branch to genesis-checked or ready-parent.
///
/// Genesis still needs the final `Startup<Genesis>` proof (`R4b -> R4c`), while
/// successor handoff validation can enter the unified `Parent<Ready>` boundary.
// ANCHOR: prototype1_live_edge_r4a_to_r4b_or_r4c
pub(crate) fn r4a_to_r4b_or_r4c(
    r4a: typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<
    typestate::R4aStartupBranch<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    let typestate::R4aParts { collected, parent } = r4a.into_parts();
    let parts = collected.into_parts();
    let Some(invocation_path) = parts.command.handoff_invocation.as_deref() else {
        let backend = GitWorktreeBackend;
        let parent = parent.check(
            &backend,
            &parts.manifest_path,
            Check {
                campaign_id: &parts.campaign_id,
                active_root: &parts.repo_root,
            },
        )?;
        return Ok(typestate::R4aStartupBranch::GenesisChecked(
            typestate::R4bGenesisChecked::from_collected_parent(parts.into_collected(), parent),
        ));
    };

    let invocation = match invocation::load_executable(invocation_path)? {
        InvocationAuthority::Successor(invocation) => invocation,
        InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "handoff invocation '{}' is a child invocation, expected successor",
                    invocation_path.display()
                ),
            });
        }
    };
    let identity = parent.identity().clone();

    let invocation_campaign_id = invocation.campaign_id().clone();
    if invocation_campaign_id != parts.campaign_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation campaign '{}' does not match command campaign '{}'",
                invocation_campaign_id, parts.campaign_id
            ),
        });
    }
    if invocation.node_id() != identity.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation node '{}' does not match parent identity node '{}'",
                invocation.node_id(),
                identity.node_id()
            ),
        });
    }
    let active_parent_root = invocation
        .active_parent_root()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation '{}' is missing active_parent_root",
                invocation_path.display()
            ),
        })?
        .to_path_buf();
    if !same_existing_path(&active_parent_root, &parts.repo_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation active_parent_root '{}' does not match command repo_root '{}'",
                active_parent_root.display(),
                parts.repo_root.display()
            ),
        });
    }

    let sealed_identity =
        validate_prototype1_successor_continuation(&invocation, &parts.manifest_path)?;
    if sealed_identity != identity {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "sealed successor parent identity for node '{}' does not match loaded parent identity",
                invocation.node_id()
            ),
        });
    }
    let startup =
        Startup::<Predecessor>::from_history(&identity, &parts.manifest_path, &parts.repo_root)?;
    let parent = parent.ready_from_predecessor_startup(startup)?;
    let ready = record_prototype1_successor_ready(&invocation)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        runtime_id = %invocation.runtime_id(),
        pid = ready.pid,
        invocation_path = %invocation_path.display(),
        active_parent_root = %active_parent_root.display(),
        "prototype1 successor acknowledged handoff before entering typed parent run"
    );
    Ok(typestate::R4aStartupBranch::PredecessorReady(
        typestate::R4cReady::from_collected_parent(
            parts
                .into_collected()
                .with_handoff_invocation(Some(invocation)),
            parent,
        ),
    ))
}
// ANCHOR_END: prototype1_live_edge_r4a_to_r4b_or_r4c

/// Convert a checked genesis parent into the unified `Parent<Ready>` state.
// ANCHOR: prototype1_live_edge_r4b_to_r4c_genesis
pub(crate) fn r4b_to_r4c_genesis(
    r4b: typestate::R4bGenesisChecked<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R4cReady<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::R4bParts { collected, parent } = r4b.into_parts();
    let parts = collected.into_parts();
    let startup = Startup::<Genesis>::from_history(parent.identity(), &parts.manifest_path)?;
    let parent = parent.ready(startup)?;
    Ok(typestate::R4cReady::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r4b_to_r4c_genesis

/// Record parent-start evidence after `Parent<Ready>` is proven.
///
/// This edge writes the parent-start journal entry and resource sample, then
/// advances to the baseline phase.
// ANCHOR: prototype1_live_edge_r4c_to_r5
pub(crate) fn r4c_to_r5(
    r4c: typestate::R4cReady<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R5<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::R4cParts { collected, parent } = r4c.into_parts();
    let mut parts = collected.into_parts();
    let parent_identity = parent.identity().clone();
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "history_startup",
        transition = "Parent<Checked>->Parent<Ready>",
        campaign = %parts.campaign_id,
        parent_id = %parent_identity.parent_id(),
        node_id = %parent_identity.node_id(),
        generation = parent_identity.generation(),
        branch_id = %parent_identity.branch_id(),
        handoff_runtime_id = ?parts.handoff_invocation.as_ref().map(|invocation| invocation.runtime_id()),
        "parent entered ready state for active turn"
    );
    let handoff_runtime_id = parts
        .handoff_invocation
        .as_ref()
        .map(|invocation| invocation.runtime_id());
    let evidence = ParentStartedEvidence {
        campaign_id: parts.campaign_id.clone(),
        parent_identity: parent_identity.clone(),
        repo_root: parts.repo_root.clone(),
        handoff_runtime_id,
        pid: std::process::id(),
        parent_recorded_at: RecordedAt::now(),
        resource_recorded_at: RecordedAt::now(),
    };
    match parts.run_shape.eval_storage_backend {
        EvalStorageBackend::Fs => {
            let mut store = ConfiguredEvalStore::fs(&mut parts.journal);
            store.put_parent_started(evidence).map_err(|err| {
                prototype1_state_transition_error("prototype1_parent_start", err.to_string())
            })?;
        }
        EvalStorageBackend::Database | EvalStorageBackend::DualStrict => {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_eval_store",
                detail: format!(
                    "eval storage backend '{}' is not wired for production parent-start until the DB handle slice",
                    eval_storage_backend_name(parts.run_shape.eval_storage_backend)
                ),
            });
        }
    }

    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %parts.campaign_id,
        parent_id = %parent_identity.parent_id(),
        generation = parent_identity.generation(),
        repo_root = %parts.repo_root.display(),
        journal_path = %parts.journal_path.display(),
        "starting typed prototype1 parent turn"
    );
    Ok(typestate::R5::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r4c_to_r5

/// Establish or load the complete parent baseline used to compare children.
// ANCHOR: prototype1_live_edge_r5_to_r6
pub(crate) async fn r5_to_r6(
    r5: typestate::R5<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R6<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::ReadyParts { collected, parent } = r5.into_parts();
    let mut parts = collected.into_parts();
    let parent_identity = parent.identity().clone();
    let parent_baseline = establish_parent_baseline_for_id(
        &parts.campaign_id,
        &parts.campaign_config,
        &parts.manifest_path,
        &parent_identity,
    )
    .await?;
    parts.facts.parent_baseline = Some(parent_baseline);
    Ok(typestate::R6::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r5_to_r6

/// Load complete-run policy and reserve the child-planning budget.
// ANCHOR: prototype1_live_edge_r6_to_r7
pub(crate) fn r6_to_r7(
    r6: typestate::R6<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R7<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::ReadyParts { collected, parent } = r6.into_parts();
    let mut parts = collected.into_parts();
    let parent_identity = parent.identity().clone();
    let (complete_search_policy, plan_child_budget) =
        resolve_parent_policy_budget(&parts.manifest_path, &parts.run_shape, &parent_identity)?;
    parts.facts.complete_search_policy = complete_search_policy;
    parts.facts.plan_child_budget = Some(plan_child_budget);
    Ok(typestate::R7::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r6_to_r7

/// Resolve and publish/load the child plan, carrying planned children forward.
// ANCHOR: prototype1_live_edge_r7_to_r8
pub(crate) async fn r7_to_r8(
    r7: typestate::R7<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R8<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::ReadyParts { collected, parent } = r7.into_parts();
    let mut parts = collected.into_parts();
    let plan_child_budget =
        parts
            .facts
            .plan_child_budget
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R7 child-plan transition missing plan child budget".to_string(),
            })?;
    let planned_children = resolve_child_plan_for_id(
        &parts.campaign_id,
        &parts.manifest_path,
        &parts.repo_root,
        parent,
        parts.run_shape.candidate_generation,
        parts.command.node_id.as_deref(),
        plan_child_budget,
        parts.run_shape.broad_tui,
        parts.run_shape.anti_attractor_policy,
        parts.campaign_config.route_source.clone(),
    )
    .await?;
    let PlannedChildren {
        parent,
        plan,
        children,
        rejected_surface_attempts,
    } = planned_children;
    parts.facts.plan_child_budget = Some(plan_child_budget);
    parts.facts.child_plan = Some(typestate::context::ChildPlanFacts {
        plan,
        children,
        rejected_surface_attempts,
    });
    Ok(typestate::R8::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r7_to_r8

/// Shape the planned children into the concrete child schedule and budget.
// ANCHOR: prototype1_live_edge_r8_to_r9
pub(crate) fn r8_to_r9(
    r8: typestate::R8<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R9<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::SelectableParts { collected, parent } = r8.into_parts();
    let mut parts = collected.into_parts();
    let plan_child_budget =
        parts
            .facts
            .plan_child_budget
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R8 schedule transition missing plan child budget".to_string(),
            })?;
    let planned_child_count = parts
        .facts
        .child_plan
        .as_ref()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "R8 schedule transition missing child plan facts".to_string(),
        })?
        .plan
        .body()
        .children()
        .len();
    let (mut child_budget, mut child_schedule_mode) =
        if let Some(policy) = parts.facts.complete_search_policy.as_ref() {
            (plan_child_budget, policy.child_schedule_mode)
        } else {
            // Non-Complete modes intentionally run one child as a debug/inspection slice.
            (
                Prototype1ChildBudget::new(1, 1),
                Prototype1ChildScheduleMode::AdaptiveBatch,
            )
        };
    if parts.run_shape.stop_after != Prototype1StateStopAfter::Complete
        && parts.command.node_id.is_some()
    {
        // Non-Complete + explicit node id is a single-node debug path.
        child_budget = Prototype1ChildBudget::new(1, 1);
        child_schedule_mode = Prototype1ChildScheduleMode::AdaptiveBatch;
    }
    if parts.command.node_id.is_none() {
        let child_plan =
            parts
                .facts
                .child_plan
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R8 schedule transition missing child plan facts".to_string(),
                })?;
        child_plan.children.truncate(child_budget.max as usize);
    }
    parts.facts.planned_child_count = Some(planned_child_count);
    parts.facts.child_budget = Some(child_budget);
    parts.facts.child_schedule_mode = Some(child_schedule_mode);
    Ok(typestate::R9::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r8_to_r9

/// Resolve the active successor-selection strategy for this parent turn.
// ANCHOR: prototype1_live_edge_r9_to_r10
pub(crate) fn r9_to_r10(
    r9: typestate::R9<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R10<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::SelectableParts { collected, parent } = r9.into_parts();
    let mut parts = collected.into_parts();
    let metric_inputs = traversal_metric_inputs(parts.run_shape.successor_selection_metrics);
    let selection_strategy = parts.run_shape.successor_selection.active_strategy(
        metric_inputs,
        parts.run_shape.successor_oracle_mode,
        parts.run_shape.successor_oracle_require_evidence,
        parts.run_shape.successor_metrics_policy,
    );
    parts.facts.selection_strategy = Some(selection_strategy);
    Ok(typestate::R10::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r9_to_r10

/// Run child fanout or rejected-only projection and carry selection evidence.
// ANCHOR: prototype1_live_edge_r10_to_r11
pub(crate) async fn r10_to_r11(
    r10: typestate::R10<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R10FanoutBranch<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError>
{
    let typestate::SelectableParts { collected, parent } = r10.into_parts();
    let mut parts = collected.into_parts();
    let parent_identity = parent.identity().clone();
    let parent_baseline = parts.facts.parent_baseline.as_ref().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "R10 fanout transition missing parent baseline".to_string(),
        }
    })?;
    let child_budget =
        parts
            .facts
            .child_budget
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R10 fanout transition missing child budget".to_string(),
            })?;
    let child_schedule_mode =
        parts
            .facts
            .child_schedule_mode
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R10 fanout transition missing child schedule mode".to_string(),
            })?;
    let selection_strategy =
        parts
            .facts
            .selection_strategy
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R10 fanout transition missing selection strategy".to_string(),
            })?;
    let child_plan =
        parts
            .facts
            .child_plan
            .take()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R10 fanout transition missing child plan facts".to_string(),
            })?;
    let typestate::context::ChildPlanFacts {
        plan: _,
        children,
        rejected_surface_attempts,
    } = child_plan;
    let rejected_only_plan = parts.run_shape.stop_after == Prototype1StateStopAfter::Complete
        && children.is_empty()
        && !rejected_surface_attempts.is_empty();
    if rejected_only_plan {
        let projection = ParentSelection::new(
            &parts.manifest_path,
            &parent_identity,
            &[],
            &rejected_surface_attempts,
        )
        .current_generation_candidates()?;
        parts.facts.child_outcomes = Some(Vec::new());
        parts.facts.selection = None;
        parts.facts.rejected_attempt_payloads = Some(projection.considered.len());
        return Ok(typestate::R10FanoutBranch::RejectedOnly(
            typestate::R11aRejectedOnly::from_collected_parent(parts.into_collected(), parent),
        ));
    }

    let (child_outcomes, selection) = if parts.run_shape.stop_after
        == Prototype1StateStopAfter::Complete
        && child_schedule_mode == Prototype1ChildScheduleMode::AdaptiveBatch
    {
        run_adaptive_child_fanout(
            &parts.campaign_id,
            &parts.manifest_path,
            &parts.repo_root,
            &parts.journal_path,
            &parent_identity,
            parent_baseline,
            child_budget,
            parts.run_shape.observe_child_stale_after,
            children,
            &rejected_surface_attempts,
            parts.run_shape.successor_selection_seed,
            selection_strategy,
        )
        .await?
    } else {
        let child_outcomes = run_child_fanout(
            &parts.campaign_id,
            &parts.manifest_path,
            &parts.repo_root,
            &parts.journal_path,
            &parent_identity,
            parent_baseline,
            parts.run_shape.stop_after,
            parts.run_shape.observe_child_stale_after,
            child_schedule_mode,
            child_budget,
            0,
            children,
        )
        .await?;
        let parent_selection = ParentSelection::new(
            &parts.manifest_path,
            &parent_identity,
            &child_outcomes,
            &rejected_surface_attempts,
        );
        let selection = if parts.run_shape.stop_after == Prototype1StateStopAfter::Complete {
            parent_selection
                .select_successor(parts.run_shape.successor_selection_seed, selection_strategy)?
        } else {
            None
        };
        (child_outcomes, selection)
    };
    parts.facts.child_outcomes = Some(child_outcomes);
    parts.facts.selection = selection;
    parts.facts.rejected_attempt_payloads = None;
    Ok(typestate::R10FanoutBranch::FanoutComplete(
        typestate::R11FanoutComplete::from_collected_parent(parts.into_collected(), parent),
    ))
}
// ANCHOR_END: prototype1_live_edge_r10_to_r11

/// Project child outcomes into report facts before continuation handling.
// ANCHOR: prototype1_live_edge_r11_to_r12
pub(crate) fn r11_to_r12(
    r11: typestate::R10FanoutBranch<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let typestate::SelectableParts { collected, parent } = match r11 {
        typestate::R10FanoutBranch::RejectedOnly(r11a) => r11a.into_parts(),
        typestate::R10FanoutBranch::FanoutComplete(r11) => r11.into_parts(),
    };
    let mut parts = collected.into_parts();
    let fallback_node = parent.node().clone();
    let planned_child_count =
        parts
            .facts
            .planned_child_count
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R11 report transition missing planned child count".to_string(),
            })?;
    let child_outcomes =
        parts
            .facts
            .child_outcomes
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R11 report transition missing child outcomes".to_string(),
            })?;
    let report_child = if parts.facts.rejected_attempt_payloads.is_some() {
        None
    } else {
        let selected_node_id = parts
            .facts
            .selection
            .as_ref()
            .map(|(decision, _)| decision.candidate_node_id.as_str());
        outcome_for_report(child_outcomes, selected_node_id)
    };
    let report = if let Some(payloads) = parts.facts.rejected_attempt_payloads {
        typestate::context::ReportFacts {
            outcome: format!(
                "rejected_surface_attempts_only;children_ran=0;children_planned={};rejected_attempt_payloads={payloads}",
                planned_child_count
            ),
            node_id: fallback_node.node_id.clone(),
            node_status: fallback_node.status,
            workspace_root: fallback_node.workspace_root.clone(),
            binary_path: fallback_node.binary_path.clone(),
            child_runtime: None,
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }
    } else {
        let report_child =
            report_child
                .as_ref()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "child fanout completed without any child outcome".to_string(),
                })?;
        typestate::context::ReportFacts {
            outcome: format!(
                "{};children_ran={};children_planned={}",
                report_child.outcome,
                child_outcomes.len(),
                planned_child_count
            ),
            node_id: report_child.node_id.clone(),
            node_status: report_child.node_status,
            workspace_root: report_child.workspace_root.clone(),
            binary_path: report_child.binary_path.clone(),
            child_runtime: report_child.child_runtime.clone(),
            successor_runtime: None,
            successor_pid: None,
            successor_ready_path: None,
        }
    };
    parts.facts.report = Some(report);
    Ok(typestate::R12::from_collected_parent(
        parts.into_collected(),
        parent,
    ))
}
// ANCHOR_END: prototype1_live_edge_r11_to_r12

/// Decide stopped vs successor-handoff continuation and record the result.
// ANCHOR: prototype1_live_edge_r12_to_r13
pub(crate) fn r12_to_r13(
    r12: typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<
    typestate::R12ContinuationBranch<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    let typestate::SelectableParts { collected, parent } = r12.into_parts();
    let mut parts = collected.into_parts();
    let parent_identity = parent.identity().clone();
    parts.facts.parent_identity = Some(parent_identity.clone());

    if let Some((selection_decision, selection_material)) = parts.facts.selection.take() {
        let material = selection_material;
        let artifact = material.selected_artifact()?;
        let node = artifact.node().clone();
        let search_policy = parts
            .facts
            .complete_search_policy
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail:
                    "successor selection reached handoff without an admitted or scheduler search policy"
                        .to_string(),
            })?;
        let decision = live_successor_continuation_decision(
            &parts.manifest_path,
            &parent_identity,
            search_policy,
            &selection_decision,
            &material,
            &node,
        )?;
        let handoff = if decision.disposition.allows_successor() {
            let selected_artifact = select_artifact_for_handoff(&selection_decision, &material)?;
            let selection_entry = material.into_entry(selection_decision.clone())?;
            Some((selected_artifact, selection_entry))
        } else {
            None
        };
        observe::Step::start(observe::span!(
            "prototype1.parent.select_successor",
            campaign_id = %parts.campaign_id,
            node_id = %node.node_id,
            generation = node.generation,
            selection_procedure = %selection_decision.procedure_id,
            selection_outcome = ?selection_decision.outcome,
            disposition = ?decision.disposition,
            selected_next_branch_id = ?decision.selected_next_branch_id,
            next_generation = decision.next_generation,
            total_nodes_after_continue = decision.total_nodes_after_continue,
        ))
        .success();
        parts
            .journal
            .append(JournalEntry::Successor(
                SuccessorRecord::selected_with_decision(
                    parts.campaign_id.clone(),
                    node.node_id.clone(),
                    decision.clone(),
                    selection_decision.clone(),
                ),
            ))
            .map_err(|err| {
                prototype1_state_transition_error("prototype1_successor_selection", err.to_string())
            })?;
        parts
            .facts
            .report
            .as_mut()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R12 continuation transition missing report facts".to_string(),
            })?
            .outcome
            .push_str(&format!(
                ";selection={:?};successor={}",
                selection_decision.outcome, selection_decision.candidate_node_id
            ));

        // ANCHOR: prototype1_live_edge_r12_handoff_branch
        if let Some((selected_artifact, selection_entry)) = handoff {
            match spawn_and_handoff_prototype1_successor(
                &parts.campaign_id,
                selected_artifact,
                &parts.repo_root,
                parent,
                selection_entry,
                prototype1_state_successor_handoff_mode(),
            )? {
                (retired, Some(successor)) => {
                    let report = parts.facts.report.as_mut().ok_or_else(|| {
                        PrepareError::InvalidBatchSelection {
                            detail: "R12 handoff transition missing report facts".to_string(),
                        }
                    })?;
                    report.successor_runtime = Some(successor.runtime_id.to_string());
                    report.successor_pid = Some(successor.pid);
                    report.successor_ready_path = Some(successor.ready_path);
                    report.outcome.push_str(";successor_handoff=acknowledged");
                    Ok(typestate::R12ContinuationBranch::HandoffCommitted(
                        typestate::R13bHandoffCommitted::from_collected_parent(
                            parts.into_collected(),
                            retired,
                        ),
                    ))
                }
                (retired, None) => {
                    parts
                        .facts
                        .report
                        .as_mut()
                        .ok_or_else(|| PrepareError::InvalidBatchSelection {
                            detail: "R12 handoff transition missing report facts".to_string(),
                        })?
                        .outcome
                        .push_str(";successor_handoff=timed_out");
                    Ok(typestate::R12ContinuationBranch::HandoffCommitted(
                        typestate::R13bHandoffCommitted::from_collected_parent(
                            parts.into_collected(),
                            retired,
                        ),
                    ))
                }
            }
        // ANCHOR_END: prototype1_live_edge_r12_handoff_branch
        } else {
            parts
                .journal
                .append(JournalEntry::Successor(SuccessorRecord::stopped(
                    parts.campaign_id.clone(),
                    node.node_id.clone(),
                    decision.clone(),
                    selection_decision.clone(),
                )))
                .map_err(|err| {
                    prototype1_state_transition_error(
                        "prototype1_successor_stopped",
                        err.to_string(),
                    )
                })?;
            parts
                .facts
                .report
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R12 stopped transition missing report facts".to_string(),
                })?
                .outcome
                .push_str(&format!(
                    ";successor_handoff=skipped:{:?}",
                    decision.disposition
                ));
            Ok(typestate::R12ContinuationBranch::Stopped(
                typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent),
            ))
        }
    } else {
        if parts.run_shape.stop_after == Prototype1StateStopAfter::Complete {
            parts
                .facts
                .report
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R12 no-selection transition missing report facts".to_string(),
                })?
                .outcome
                .push_str(";selection=none");
        }
        Ok(typestate::R12ContinuationBranch::Stopped(
            typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent),
        ))
    }
}
// ANCHOR_END: prototype1_live_edge_r12_to_r13

/// Emit the final CLI report and record parent completion side effects.
// ANCHOR: prototype1_live_edge_emit_final_report_from_parts
pub(crate) fn emit_final_report_from_parts(
    mut parts: typestate::context::CollectedParts<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<
    typestate::context::Collected<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    let parent_identity = parts.facts.parent_identity.as_ref().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: "R14 final transition missing parent identity".to_string(),
        }
    })?;
    append_parent_target_sample(
        &mut parts.journal,
        &parts.campaign_id,
        parent_identity,
        parts
            .handoff_invocation
            .as_ref()
            .map(|invocation| invocation.runtime_id()),
        &parts.repo_root,
        journal::resource::Phase::ParentComplete,
    );
    let report_facts =
        parts
            .facts
            .report
            .as_ref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R14 final transition missing report facts".to_string(),
            })?;
    let report = Prototype1StateReport {
        campaign_id: parts.campaign_id.clone(),
        node_id: report_facts.node_id.clone(),
        repo_root: parts.repo_root.clone(),
        journal_path: parts.journal_path.clone(),
        stop_after: parts.run_shape.stop_after,
        outcome: report_facts.outcome.clone(),
        node_status: report_facts.node_status,
        workspace_root: report_facts.workspace_root.clone(),
        binary_path: report_facts.binary_path.clone(),
        child_runtime: report_facts.child_runtime.clone(),
        successor_runtime: report_facts.successor_runtime.clone(),
        successor_pid: report_facts.successor_pid,
        successor_ready_path: report_facts.successor_ready_path.clone(),
    };
    let report_path = prototype1_state_report_path(&parts.manifest_path, parent_identity);
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let report_json = serde_json::to_vec_pretty(&report).map_err(PrepareError::Serialize)?;
    fs::write(&report_path, report_json).map_err(|source| PrepareError::WriteManifest {
        path: report_path,
        source,
    })?;

    #[cfg(not(feature = "demo"))]
    {
        match parts.command.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => {
                println!("prototype1 state");
                println!("{}", "-".repeat(40));
                println!("campaign_id: {}", report.campaign_id);
                println!("node_id: {}", report.node_id);
                println!("repo_root: {}", report.repo_root.display());
                println!("journal_path: {}", report.journal_path.display());
                println!("stop_after: {:?}", report.stop_after);
                println!("outcome: {}", report.outcome);
                println!("node_status: {:?}", report.node_status);
                println!("workspace_root: {}", report.workspace_root.display());
                println!("binary_path: {}", report.binary_path.display());
                println!(
                    "child_runtime: {}",
                    report.child_runtime.as_deref().unwrap_or("-")
                );
                println!(
                    "successor_runtime: {}",
                    report.successor_runtime.as_deref().unwrap_or("-")
                );
                println!(
                    "successor_pid: {}",
                    report
                        .successor_pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "successor_ready_path: {}",
                    report
                        .successor_ready_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }
    }
    #[cfg(feature = "demo")]
    let _ = report;
    if let Some(invocation) = parts.handoff_invocation.as_ref() {
        let _ = record_prototype1_successor_completion(
            invocation,
            &parts.manifest_path,
            SuccessorCompletionStatus::Succeeded,
            None,
            None,
        )?;
    }
    Ok(parts.into_collected())
}
// ANCHOR_END: prototype1_live_edge_emit_final_report_from_parts

/// Finalize the stopped or handoff path after report emission.
// ANCHOR: prototype1_live_edge_r13_to_r14
pub(crate) fn r13_to_r14(
    r13: typestate::R12ContinuationBranch<Prototype1StateRunShape, ResolvedCampaignConfig>,
) -> Result<typestate::R14FinalBranch<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError>
{
    match r13 {
        typestate::R12ContinuationBranch::Stopped(r13a) => {
            let typestate::SelectableParts { collected, parent } = r13a.into_parts();
            let collected = emit_final_report_from_parts(collected.into_parts())?;
            Ok(typestate::R14FinalBranch::Stopped(
                typestate::R14aFinalStopped::from_collected_parent(collected, parent),
            ))
        }
        typestate::R12ContinuationBranch::HandoffCommitted(r13b) => {
            let typestate::RetiredParts { collected, parent } = r13b.into_parts();
            let collected = emit_final_report_from_parts(collected.into_parts())?;
            Ok(typestate::R14FinalBranch::Handoff(
                typestate::R14bFinalHandoff::from_collected_parent(collected, parent),
            ))
        }
    }
}
// ANCHOR_END: prototype1_live_edge_r13_to_r14
// ANCHOR_END: prototype1_live_edges
