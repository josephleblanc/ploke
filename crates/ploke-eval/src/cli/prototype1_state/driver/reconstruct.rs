//! Side-effect-free reconstruction for early Prototype 1 parent typestates.
//!
//! This slice reconstructs setup through policy-ready parent typestates. It
//! rebuilds typed carriers from durable checkout/campaign/journal evidence
//! without calling side-effectful live edges when the evidence already exists.

use std::path::{Path, PathBuf};

use ploke_records::ids::CampaignId;

use crate::{
    CampaignOverrides, ResolvedCampaignConfig,
    cli::{
        InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
        Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
        prototype1_process::validate_prototype1_successor_continuation,
        prototype1_state::{
            backend::GitWorktreeBackend,
            cli_facing::{
                ParentSelection, Prototype1StateRunShape, campaign_manifest_path_for_id,
                child_plan_message_path_for_parent, load_existing_child_plan_for_id,
                load_parent_baseline_for_id, prototype1_state_transition_error,
                reconstruct_child_outcomes_from_store, resolve_campaign_config_for_id,
                resolve_parent_policy_budget, same_existing_path,
                validate_existing_child_plan_for_id,
            },
            identity::{ParentIdentity, load_parent_identity_optional, parent_identity_path},
            invocation::{self, InvocationAuthority},
            journal::{self, JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            live_edges::{
                r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis, r8_to_r9,
                r9_to_r10, r11_to_r12,
            },
            parent::{Predecessor, Startup},
            successor,
            typestate::{self, StepInput},
        },
    },
    spec::PrepareError,
};

/// Concrete early parent typestate reconstructed from durable evidence.
pub(crate) enum EarlyState {
    R1(typestate::R1<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R3(typestate::R3<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R4a(typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R4b(typestate::R4bGenesisChecked<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R4c(typestate::R4cReady<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R5(typestate::R5<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R6(typestate::R6<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R7(typestate::R7<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R10(typestate::R10<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R12(typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R13a(typestate::R13aStopped<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R13b(typestate::R13bHandoffCommitted<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14a(typestate::R14aFinalStopped<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14b(typestate::R14bFinalHandoff<Prototype1StateRunShape, ResolvedCampaignConfig>),
}

/// Result of an early durable reconstruction attempt.
pub(crate) struct EarlySnapshot {
    pub(crate) state: Option<EarlyState>,
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) notes: Vec<String>,
    pub(crate) blockers: Vec<String>,
}

/// Reconstruct the most advanced R0-R5 state supported by durable evidence.
pub(crate) fn reconstruct_early(repo_root: &Path) -> Result<EarlySnapshot, PrepareError> {
    let mut notes = Vec::new();
    let mut blockers = Vec::new();
    let identity_path = parent_identity_path(repo_root);
    let Some(identity) = load_parent_identity_optional(repo_root)? else {
        notes.push(format!(
            "no parent identity found at '{}'",
            identity_path.display()
        ));
        return Ok(EarlySnapshot {
            state: None,
            campaign_id: None,
            notes,
            blockers,
        });
    };

    let campaign_id = identity.campaign_id().clone();
    notes.push(format!(
        "parent identity: {} (node={}, generation={}, branch={})",
        identity_path.display(),
        identity.node_id(),
        identity.generation(),
        identity.branch_id()
    ));
    let handoff_invocation =
        infer_successor_handoff_invocation(repo_root, &campaign_id, &identity)?;
    if let Some(path) = handoff_invocation.as_ref() {
        notes.push(format!(
            "inferred successor handoff invocation from durable journal: {}",
            path.display()
        ));
    }

    let r1 = match reconstruct_r1(
        repo_root.to_path_buf(),
        &campaign_id,
        handoff_invocation.clone(),
    ) {
        Ok(r1) => r1,
        Err(error) => {
            blockers.push(format!("r1 reconstruction blocked: {error}"));
            return Ok(EarlySnapshot {
                state: None,
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R1 from campaign manifest and admitted run profile/defaults".into());

    let r3 = match r1.advance(r1_to_r2a_or_r3) {
        Ok(typestate::R1Branch::R2a(r2a)) => {
            let _ = r2a;
            blockers.push("durable reconstruction unexpectedly entered R2a init branch".into());
            return Ok(EarlySnapshot {
                state: Some(
                    reconstruct_r1(
                        repo_root.to_path_buf(),
                        &campaign_id,
                        handoff_invocation.clone(),
                    )
                    .map(EarlyState::R1)?,
                ),
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
        Ok(typestate::R1Branch::R3(r3)) => r3,
        Err(error) => {
            blockers.push(format!("r1_to_r2a_or_r3 blocked: {error}"));
            return Ok(EarlySnapshot {
                state: Some(
                    reconstruct_r1(
                        repo_root.to_path_buf(),
                        &campaign_id,
                        handoff_invocation.clone(),
                    )
                    .map(EarlyState::R1)?,
                ),
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R3 from checkout parent identity".into());

    let r4a = match r3.advance(r3_to_r4a) {
        Ok(r4a) => r4a,
        Err(error) => {
            blockers.push(format!("r3_to_r4a blocked: {error}"));
            return Ok(EarlySnapshot {
                state: Some(
                    reconstruct_r3(repo_root, &campaign_id, handoff_invocation.clone())
                        .map(EarlyState::R3)?,
                ),
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R4a by loading Parent<Unchecked>".into());

    let startup = match reconstruct_startup(r4a, handoff_invocation.as_deref()) {
        Ok(startup) => startup,
        Err(error) => {
            blockers.push(format_r4a_blocker(repo_root, &error));
            return Ok(EarlySnapshot {
                state: Some(
                    reconstruct_r4a(repo_root, &campaign_id, handoff_invocation.clone())
                        .map(EarlyState::R4a)?,
                ),
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
    };

    let r4c = match startup {
        typestate::R4aStartupBranch::GenesisChecked(r4b) => match r4b.advance(r4b_to_r4c_genesis) {
            Ok(r4c) => {
                notes.push("reconstructed R4c from genesis startup validation".into());
                r4c
            }
            Err(error) => {
                blockers.push(format!("r4b_to_r4c_genesis blocked: {error}"));
                return Ok(EarlySnapshot {
                    state: Some(reconstruct_r4b(repo_root, &campaign_id).map(EarlyState::R4b)?),
                    campaign_id: Some(campaign_id),
                    notes,
                    blockers,
                });
            }
        },
        typestate::R4aStartupBranch::PredecessorReady(r4c) => {
            notes.push("reconstructed R4c from predecessor startup validation".into());
            r4c
        }
    };

    let typestate::R4cParts { collected, parent } = r4c.into_parts();
    if parent_start_recorded(repo_root, &campaign_id, parent.identity())? {
        notes.push("reconstructed R5 from matching parent-start journal evidence".into());
        let mut parts = collected.into_parts();
        if let Some(baseline) = load_parent_baseline_for_id(
            &parts.campaign_id,
            &parts.campaign_config,
            &parts.manifest_path,
            parent.identity(),
        )? {
            parts.facts.parent_baseline = Some(baseline);
            notes.push("reconstructed R6 from durable parent baseline evidence".into());
            match resolve_parent_policy_budget(
                &parts.manifest_path,
                &parts.run_shape,
                parent.identity(),
            ) {
                Ok((policy, budget)) => {
                    parts.facts.complete_search_policy = policy;
                    parts.facts.plan_child_budget = Some(budget);
                    notes.push("reconstructed R7 from run policy and child budget inputs".into());
                    let plan_path =
                        child_plan_message_path_for_parent(&parts.manifest_path, parent.identity());
                    if plan_path.exists() {
                        match validate_existing_child_plan_for_id(
                            &parts.manifest_path,
                            parent.identity(),
                        ) {
                            Ok(()) => {
                                let planned =
                                    load_existing_child_plan_for_id(&parts.manifest_path, parent)?;
                                let parent = planned.parent;
                                parts.facts.child_plan = Some(typestate::context::ChildPlanFacts {
                                    plan: planned.plan,
                                    children: planned.children,
                                    rejected_surface_attempts: planned.rejected_surface_attempts,
                                });
                                notes.push(
                                    "reconstructed R8 from existing child-plan message evidence"
                                        .into(),
                                );
                                let r8 = typestate::R8::from_collected_parent(
                                    parts.into_collected(),
                                    parent,
                                );
                                let state = reconstruct_after_r8(r8, &mut notes, &mut blockers)?;
                                return Ok(EarlySnapshot {
                                    state: Some(state),
                                    campaign_id: Some(campaign_id),
                                    notes,
                                    blockers,
                                });
                            }
                            Err(error) => {
                                blockers.push(format!("blocked edge r7 -> r8: {error}"));
                            }
                        }
                    }
                    return Ok(EarlySnapshot {
                        state: Some(EarlyState::R7(typestate::R7::from_collected_parent(
                            parts.into_collected(),
                            parent,
                        ))),
                        campaign_id: Some(campaign_id),
                        notes,
                        blockers,
                    });
                }
                Err(error) => {
                    blockers.push(format!("blocked edge r6 -> r7: {error}"));
                    return Ok(EarlySnapshot {
                        state: Some(EarlyState::R6(typestate::R6::from_collected_parent(
                            parts.into_collected(),
                            parent,
                        ))),
                        campaign_id: Some(campaign_id),
                        notes,
                        blockers,
                    });
                }
            }
        }
        Ok(EarlySnapshot {
            state: Some(EarlyState::R5(typestate::R5::from_collected_parent(
                parts.into_collected(),
                parent,
            ))),
            campaign_id: Some(campaign_id),
            notes,
            blockers,
        })
    } else {
        notes.push("no matching parent-start journal evidence; stopping at R4c".into());
        Ok(EarlySnapshot {
            state: Some(EarlyState::R4c(typestate::R4cReady::from_collected_parent(
                collected, parent,
            ))),
            campaign_id: Some(campaign_id),
            notes,
            blockers,
        })
    }
}

fn reconstruct_after_r8(
    r8: typestate::R8<Prototype1StateRunShape, ResolvedCampaignConfig>,
    notes: &mut Vec<String>,
    blockers: &mut Vec<String>,
) -> Result<EarlyState, PrepareError> {
    let r9 = r8.advance(r8_to_r9)?;
    notes.push("reconstructed R9 by shaping existing child-plan schedule".into());
    let r10 = r9.advance(r9_to_r10)?;
    notes.push("reconstructed R10 by resolving selection strategy inputs".into());

    let typestate::SelectableParts { collected, parent } = r10.into_parts();
    let mut parts = collected.into_parts();
    let child_plan = match parts.facts.child_plan.as_ref() {
        Some(child_plan) => child_plan,
        None => {
            blockers.push("blocked edge r10 -> r11: missing child-plan facts".into());
            return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        }
    };
    let children = child_plan.children.clone();
    let rejected_surface_attempts = child_plan.rejected_surface_attempts.clone();
    let parent_identity = parent.identity().clone();
    let selection_strategy = match parts.facts.selection_strategy {
        Some(strategy) => strategy,
        None => {
            blockers.push("blocked edge r10 -> r11: missing selection strategy".into());
            return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        }
    };

    let rejected_only_plan = parts.run_shape.stop_after == Prototype1StateStopAfter::Complete
        && children.is_empty()
        && !rejected_surface_attempts.is_empty();
    if rejected_only_plan {
        let projection = match ParentSelection::new(
            &parts.manifest_path,
            &parent_identity,
            &[],
            &rejected_surface_attempts,
        )
        .current_generation_candidates()
        {
            Ok(projection) => projection,
            Err(error) => {
                blockers.push(format!(
                    "blocked edge r10 -> r11a: rejected-only projection failed: {error}"
                ));
                return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
                    parts.into_collected(),
                    parent,
                )));
            }
        };
        parts.facts.child_outcomes = Some(Vec::new());
        parts.facts.selection = None;
        parts.facts.rejected_attempt_payloads = Some(projection.considered.len());
        let r11a =
            typestate::R11aRejectedOnly::from_collected_parent(parts.into_collected(), parent);
        notes.push("reconstructed R11a from rejected surface-attempt payloads".into());
        let r12 = r11_to_r12(typestate::R10FanoutBranch::RejectedOnly(r11a))?;
        notes.push("reconstructed R12 report facts from rejected-only evidence".into());
        return reconstruct_after_r12(r12, notes, blockers);
    }

    let child_outcomes = match reconstruct_child_outcomes_from_store(
        &parts.campaign_id,
        &parts.manifest_path,
        &children,
    ) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            blockers.push(format!(
                "blocked edge r10 -> r11: durable child outcome reconstruction failed: {error}"
            ));
            return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        }
    };
    let selection = if parts.run_shape.stop_after == Prototype1StateStopAfter::Complete {
        match ParentSelection::new(
            &parts.manifest_path,
            &parent_identity,
            &child_outcomes,
            &rejected_surface_attempts,
        )
        .select_successor(parts.run_shape.successor_selection_seed, selection_strategy)
        {
            Ok(selection) => selection,
            Err(error) => {
                blockers.push(format!(
                    "blocked edge r10 -> r11: successor selection reconstruction failed: {error}"
                ));
                return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
                    parts.into_collected(),
                    parent,
                )));
            }
        }
    } else {
        None
    };
    let outcome_count = child_outcomes.len();
    parts.facts.child_outcomes = Some(child_outcomes);
    parts.facts.selection = selection;
    parts.facts.rejected_attempt_payloads = None;
    let r11 = typestate::R11FanoutComplete::from_collected_parent(parts.into_collected(), parent);
    notes.push(format!(
        "reconstructed R11 from {outcome_count} channel-derived child outcomes"
    ));
    let r12 = r11_to_r12(typestate::R10FanoutBranch::FanoutComplete(r11))?;
    notes.push("reconstructed R12 report facts from child outcomes".into());
    reconstruct_after_r12(r12, notes, blockers)
}

fn reconstruct_after_r12(
    r12: typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>,
    notes: &mut Vec<String>,
    blockers: &mut Vec<String>,
) -> Result<EarlyState, PrepareError> {
    let typestate::SelectableParts { collected, parent } = r12.into_parts();
    let mut parts = collected.into_parts();
    if let Some((selection_decision, selection_material)) = parts.facts.selection.as_ref() {
        let selected_artifact = selection_material.selected_artifact()?;
        let node = selected_artifact.node();
        let policy = parts.facts.complete_search_policy.as_ref().ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail: "R12 handoff reconstruction missing search policy".to_string(),
            }
        })?;
        let decision =
            crate::cli::prototype1_state::cli_facing::live_successor_continuation_decision(
                &parts.manifest_path,
                parent.identity(),
                policy,
                selection_decision,
                selection_material,
                node,
            )?;
        let report =
            parts
                .facts
                .report
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R12 handoff reconstruction missing report facts".to_string(),
                })?;
        report.outcome.push_str(&format!(
            ";selection={:?};successor={}",
            selection_decision.outcome, selection_decision.candidate_node_id
        ));
        parts.facts.parent_identity = Some(parent.identity().clone());
        if decision.disposition.allows_successor() {
            let Some(handoff) = successor_handoff_evidence(
                &parts.repo_root,
                &parts.campaign_id,
                &selection_decision.candidate_node_id,
            )?
            else {
                blockers.push(format!(
                    "blocked edge r12 -> r13b: selected successor '{}' has no durable handoff/ready evidence for repo_root '{}'",
                    selection_decision.candidate_node_id,
                    parts.repo_root.display()
                ));
                return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
                    parts.into_collected(),
                    parent,
                )));
            };
            report.successor_runtime = Some(handoff.runtime_id.to_string());
            report.successor_pid = Some(handoff.pid);
            report.successor_ready_path = Some(handoff.ready_path);
            report.outcome.push_str(";successor_handoff=acknowledged");
            let complete_recorded =
                parent_complete_recorded(&parts.repo_root, &parts.campaign_id, parent.identity())?;
            let (retired, _lineage) = parent.into_retired_and_lineage();
            let r13b = typestate::R13bHandoffCommitted::from_collected_parent(
                parts.into_collected(),
                retired,
            );
            notes.push(
                "reconstructed R13b successor handoff from checkout/handoff journal evidence"
                    .into(),
            );
            if !complete_recorded {
                return Ok(EarlyState::R13b(r13b));
            }
            let typestate::RetiredParts { collected, parent } = r13b.into_parts();
            notes.push(
                "reconstructed R14b final handoff report from parent-complete resource evidence"
                    .into(),
            );
            return Ok(EarlyState::R14b(
                typestate::R14bFinalHandoff::from_collected_parent(collected, parent),
            ));
        }
        if !successor_stopped_recorded(&parts.campaign_id, &selection_decision.candidate_node_id)? {
            blockers.push(format!(
                "blocked edge r12 -> r13a: selected successor '{}' stopped by policy but no durable stopped successor record exists",
                selection_decision.candidate_node_id
            ));
            return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        }
        report.outcome.push_str(&format!(
            ";successor_handoff=skipped:{:?}",
            decision.disposition
        ));
        let complete_recorded =
            parent_complete_recorded(&parts.repo_root, &parts.campaign_id, parent.identity())?;
        let r13a = typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent);
        notes.push("reconstructed R13a selected-successor stopped continuation from durable successor record".into());
        if !complete_recorded {
            return Ok(EarlyState::R13a(r13a));
        }
        let typestate::SelectableParts { collected, parent } = r13a.into_parts();
        notes.push(
            "reconstructed R14a stopped final report from parent-complete resource evidence".into(),
        );
        return Ok(EarlyState::R14a(
            typestate::R14aFinalStopped::from_collected_parent(collected, parent),
        ));
    }

    if parts.run_shape.stop_after == Prototype1StateStopAfter::Complete {
        parts
            .facts
            .report
            .as_mut()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R12 no-selection reconstruction missing report facts".to_string(),
            })?
            .outcome
            .push_str(";selection=none");
    }
    let complete_recorded =
        parent_complete_recorded(&parts.repo_root, &parts.campaign_id, parent.identity())?;
    parts.facts.parent_identity = Some(parent.identity().clone());
    let r13a = typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent);
    notes.push(
        "reconstructed R13a no-selection stopped continuation from absent successor selection"
            .into(),
    );
    if !complete_recorded {
        return Ok(EarlyState::R13a(r13a));
    }

    let typestate::SelectableParts { collected, parent } = r13a.into_parts();
    notes.push(
        "reconstructed R14a stopped final report from parent-complete resource evidence".into(),
    );
    Ok(EarlyState::R14a(
        typestate::R14aFinalStopped::from_collected_parent(collected, parent),
    ))
}

fn infer_successor_handoff_invocation(
    repo_root: &Path,
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
) -> Result<Option<PathBuf>, PrepareError> {
    if identity.generation() == 0 {
        return Ok(None);
    }
    let manifest_path = campaign_manifest_path_for_id(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let entries = journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })?;
    for entry in entries.into_iter().rev() {
        let JournalEntry::SuccessorHandoff(handoff) = entry else {
            continue;
        };
        if handoff.campaign_id != *campaign_id
            || handoff.node_id != identity.node_id()
            || !same_existing_path(&handoff.active_parent_root, repo_root)
        {
            continue;
        }
        let invocation = match invocation::load_authority(&handoff.invocation_path)? {
            InvocationAuthority::Successor(invocation) => invocation,
            InvocationAuthority::Child(_) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor handoff journal references child invocation '{}', expected successor",
                        handoff.invocation_path.display()
                    ),
                });
            }
        };
        if invocation.campaign_id() != campaign_id
            || invocation.node_id() != identity.node_id()
            || invocation.runtime_id() != handoff.runtime_id
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor handoff invocation '{}' does not match active parent identity/runtime",
                    handoff.invocation_path.display()
                ),
            });
        }
        let active_parent_root =
            invocation
                .active_parent_root()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor handoff invocation '{}' is missing active_parent_root",
                        handoff.invocation_path.display()
                    ),
                })?;
        if !same_existing_path(active_parent_root, repo_root) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor handoff invocation active_parent_root '{}' does not match repo_root '{}'",
                    active_parent_root.display(),
                    repo_root.display()
                ),
            });
        }
        return Ok(Some(handoff.invocation_path));
    }
    Ok(None)
}

fn reconstruct_startup(
    r4a: typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>,
    handoff_invocation: Option<&Path>,
) -> Result<
    typestate::R4aStartupBranch<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    let Some(invocation_path) = handoff_invocation else {
        return r4a.advance(r4a_to_r4b_or_r4c);
    };

    let typestate::R4aParts { collected, parent } = r4a.into_parts();
    let parts = collected.into_parts();
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
    Ok(typestate::R4aStartupBranch::PredecessorReady(
        typestate::R4cReady::from_collected_parent(
            parts
                .into_collected()
                .with_handoff_invocation(Some(invocation)),
            parent,
        ),
    ))
}

fn reconstruct_r1(
    repo_root: PathBuf,
    campaign_id: &CampaignId,
    handoff_invocation: Option<PathBuf>,
) -> Result<typestate::R1<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let command = default_command(repo_root.clone(), campaign_id.clone(), handoff_invocation);
    let manifest_path = campaign_manifest_path_for_id(campaign_id)?;
    let run_shape = Prototype1StateRunShape::resolve(&command, &manifest_path)?;
    let config = resolve_campaign_config_for_id(campaign_id, &CampaignOverrides::default())?;
    let journal_path = prototype1_transition_journal_path(&manifest_path);
    let journal = PrototypeJournal::new(journal_path.clone());

    Ok(typestate::R1::from_collected(
        typestate::context::Collected::new(
            command,
            repo_root,
            campaign_id.clone(),
            manifest_path,
            run_shape,
            config,
            journal_path,
            journal,
        ),
    ))
}

fn reconstruct_r3(
    repo_root: &Path,
    campaign_id: &CampaignId,
    handoff_invocation: Option<PathBuf>,
) -> Result<typestate::R3<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    match reconstruct_r1(repo_root.to_path_buf(), campaign_id, handoff_invocation)?
        .advance(r1_to_r2a_or_r3)?
    {
        typestate::R1Branch::R2a(_) => Err(PrepareError::InvalidBatchSelection {
            detail: "durable reconstruction unexpectedly entered R2a init branch".to_string(),
        }),
        typestate::R1Branch::R3(r3) => Ok(r3),
    }
}

fn reconstruct_r4a(
    repo_root: &Path,
    campaign_id: &CampaignId,
    handoff_invocation: Option<PathBuf>,
) -> Result<typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    reconstruct_r3(repo_root, campaign_id, handoff_invocation)?.advance(r3_to_r4a)
}

fn reconstruct_r4b(
    repo_root: &Path,
    campaign_id: &CampaignId,
) -> Result<
    typestate::R4bGenesisChecked<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    match reconstruct_r4a(repo_root, campaign_id, None)?.advance(r4a_to_r4b_or_r4c)? {
        typestate::R4aStartupBranch::GenesisChecked(r4b) => Ok(r4b),
        typestate::R4aStartupBranch::PredecessorReady(_) => {
            Err(PrepareError::InvalidBatchSelection {
                detail: "durable reconstruction expected R4b but reached predecessor R4c"
                    .to_string(),
            })
        }
    }
}

/// Render a checkout/startup guard failure as an operator-facing typed blocker.
pub(crate) fn format_r4a_blocker(repo_root: &Path, error: &PrepareError) -> String {
    let identity = load_parent_identity_optional(repo_root).ok().flatten();
    let expected = identity
        .as_ref()
        .and_then(|identity| identity.artifact_branch())
        .unwrap_or("-");
    let backend = GitWorktreeBackend;
    let active = backend
        .active_branch(repo_root)
        .unwrap_or_else(|error| format!("<unavailable: {error}>"));
    let dirty = backend.dirty_paths(repo_root).unwrap_or_default();

    let mut lines = vec![
        "blocked edge: r4a_to_r4b_or_r4c".to_string(),
        format!("reason: {error}"),
        format!("repo_root: {}", repo_root.display()),
        format!("active_branch: {active}"),
        format!("expected_artifact_branch: {expected}"),
    ];
    if !dirty.is_empty() {
        lines.push("dirty_paths:".to_string());
        for path in dirty.iter().take(12) {
            lines.push(format!("  - {}", path.display()));
        }
        if dirty.len() > 12 {
            lines.push(format!("  - ... {} more", dirty.len() - 12));
        }
    }
    lines.push("recovery:".to_string());
    lines.push("  ploke-eval loop walk use /path/to/actual/parent-worktree".to_string());
    lines.push("  ploke-eval loop walk reset".to_string());
    lines.push("  ploke-eval loop walk start".to_string());
    if expected != "-" {
        lines.push(format!("  git worktree list | rg {expected}"));
    }
    lines.join("\n")
}

struct HandoffEvidence {
    runtime_id: crate::cli::prototype1_state::event::RuntimeId,
    pid: u32,
    ready_path: PathBuf,
}

fn successor_handoff_evidence(
    repo_root: &Path,
    campaign_id: &CampaignId,
    node_id: &str,
) -> Result<Option<HandoffEvidence>, PrepareError> {
    for entry in journal_entries(campaign_id)?.into_iter().rev() {
        let JournalEntry::SuccessorHandoff(handoff) = entry else {
            continue;
        };
        if handoff.campaign_id != *campaign_id
            || handoff.node_id != node_id
            || !same_existing_path(&handoff.active_parent_root, repo_root)
        {
            continue;
        }
        let invocation = match invocation::load_authority(&handoff.invocation_path)? {
            InvocationAuthority::Successor(invocation) => invocation,
            InvocationAuthority::Child(_) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor handoff journal references child invocation '{}', expected successor",
                        handoff.invocation_path.display()
                    ),
                });
            }
        };
        if invocation.campaign_id() != campaign_id
            || invocation.node_id() != node_id
            || invocation.runtime_id() != handoff.runtime_id
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor handoff invocation '{}' does not match handoff journal entry for node '{}'",
                    handoff.invocation_path.display(),
                    node_id
                ),
            });
        }
        if !handoff.ready_path.exists() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor handoff ready path '{}' is missing for node '{}'",
                    handoff.ready_path.display(),
                    node_id
                ),
            });
        }
        return Ok(Some(HandoffEvidence {
            runtime_id: handoff.runtime_id,
            pid: handoff.pid,
            ready_path: handoff.ready_path,
        }));
    }
    Ok(None)
}

fn successor_stopped_recorded(
    campaign_id: &CampaignId,
    node_id: &str,
) -> Result<bool, PrepareError> {
    Ok(journal_entries(campaign_id)?
        .into_iter()
        .any(|entry| match entry {
            JournalEntry::Successor(record) => {
                record.campaign_id == *campaign_id
                    && record.node_id == node_id
                    && matches!(record.state, successor::State::Stopped { .. })
            }
            _ => false,
        }))
}

fn journal_entries(campaign_id: &CampaignId) -> Result<Vec<JournalEntry>, PrepareError> {
    let manifest_path = campaign_manifest_path_for_id(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })
}

fn parent_complete_recorded(
    repo_root: &Path,
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
) -> Result<bool, PrepareError> {
    let manifest_path = campaign_manifest_path_for_id(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let entries = journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })?;
    let target_dir = repo_root.join("target");
    Ok(entries.into_iter().any(|entry| match entry {
        JournalEntry::Resource(sample) => {
            sample.campaign_id == *campaign_id
                && sample.parent_id == identity.parent_id()
                && sample.node_id == identity.node_id()
                && sample.generation == identity.generation()
                && sample.phase == journal::resource::Phase::ParentComplete
                && same_existing_path(&sample.path, &target_dir)
        }
        _ => false,
    }))
}

fn parent_start_recorded(
    repo_root: &Path,
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
) -> Result<bool, PrepareError> {
    let manifest_path = campaign_manifest_path_for_id(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let entries = journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })?;
    Ok(entries.into_iter().any(|entry| match entry {
        JournalEntry::ParentStarted(entry) => {
            entry.campaign_id == *campaign_id
                && entry.parent_identity == *identity
                && same_existing_path(&entry.repo_root, repo_root)
        }
        _ => false,
    }))
}

fn default_command(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    handoff_invocation: Option<PathBuf>,
) -> Prototype1StateCommand {
    Prototype1StateCommand {
        campaign: Some(campaign_id),
        node_id: None,
        repo_root: Some(repo_root),
        init_parent_identity: false,
        identity_branch: None,
        identity_instance: None,
        handoff_invocation,
        stop_after: Prototype1StateStopAfter::Complete,
        successor_selection: Prototype1SuccessorSelection::HistoryScoreChildProp,
        successor_selection_seed: 0,
        successor_selection_metrics: Prototype1TraversalMetrics::Operational,
        candidate_generator: Prototype1CandidateGenerator::BroadHarnessRequest,
        format: InspectOutputFormat::Table,
    }
}
