//! Side-effect-free durable reconstruction for Prototype 1 parent typestates.
//!
//! This module rebuilds the most advanced currently supported `Runtime<...>`
//! carrier from checkout, campaign, journal, channel, History, and handoff
//! evidence. It started as an R0-R5/R7 reconstruction seam, so some internal
//! names still say `Early*`; the current behavior is broader and can reconstruct
//! through R14a/R14b when the required durable evidence exists.
//!
//! Reconstruction must not replay side effects. When evidence is complete, it
//! rebuilds typed carriers and calls only pure/projection edges. When evidence is
//! missing or inconsistent, it returns the latest safe state plus explicit
//! blockers instead of spawning children, appending journal records, sealing
//! History, mutating checkout state, or fabricating child outcomes.

use std::path::{Path, PathBuf};

use ploke_records::ids::CampaignId;

use crate::{
    CampaignOverrides, ResolvedCampaignConfig, campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1CandidateGenerator, Prototype1StateCommand,
        Prototype1StateStopAfter, Prototype1SuccessorSelection, Prototype1TraversalMetrics,
        prototype1_process::validate_prototype1_successor_continuation,
        prototype1_state::{
            backend::GitWorktreeBackend,
            cli_facing::{
                ParentSelection, Prototype1StateRunShape, child_plan_message_path_for_parent,
                load_existing_child_plan_for_id, load_parent_baseline,
                prototype1_state_transition_error, reconstruct_child_outcomes_from_store,
                resolve_parent_policy_budget, same_existing_path,
                validate_existing_child_plan_for_id,
            },
            history::{ActorRef, BlockStore, FsBlockStore, LineageId, StoreHead},
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
            walk::phase::WalkPhase,
        },
    },
    resolve_campaign_config,
    spec::PrepareError,
};

// ANCHOR: prototype1_reconstruct_snapshot_state
/// Concrete parent typestate reconstructed from durable evidence.
///
/// The name is historical: early slices only reconstructed setup/policy phases.
/// The enum now includes all currently reconstructable walk phases, including
/// stopped and selected-successor final states when durable handoff/completion
/// evidence is present.
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
    R13c(typestate::R13cHandoffIncomplete<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14a(typestate::R14aFinalStopped<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14b(typestate::R14bFinalHandoff<Prototype1StateRunShape, ResolvedCampaignConfig>),
}

/// Result of a durable reconstruction attempt.
///
/// The state is optional because a checkout with no parent identity cannot be
/// admitted into the Prototype 1 parent typestate graph. `notes` explain which
/// evidence was accepted; `blockers` explain strict edges that could not be
/// reconstructed without weakening invariants.
pub(crate) struct EarlySnapshot {
    pub(crate) state: Option<EarlyState>,
    pub(crate) blocked: Option<ReconstructionBlocker>,
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) notes: Vec<String>,
    pub(crate) blockers: Vec<String>,
}

/// Read-only cursor at the last trustworthy durable boundary when no exact
/// typed carrier can be rebuilt. This projection does not confer authority.
pub(crate) struct ReconstructionBlocker {
    pub(crate) phase: WalkPhase,
    pub(crate) detail: String,
}
// ANCHOR_END: prototype1_reconstruct_snapshot_state

// ANCHOR: prototype1_reconstruct_early
/// Reconstruct the most advanced supported parent state from durable evidence.
///
/// This is read-only with respect to loop side effects. It may read parent
/// identity, campaign manifests, run profiles, transition journals, child-plan
/// messages, channel records, runner/evaluation projections tied to channel
/// evidence, successor handoff records, and parent-complete evidence. It must
/// not create missing evidence or make permissive assumptions about schema,
/// parent identity, child terminality, branch evaluations, History, or checkout
/// state.
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
            blocked: None,
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
    if let Some(blocked) = post_checkout_blocker(repo_root, &campaign_id, &identity)? {
        return Ok(EarlySnapshot {
            state: None,
            blocked: Some(blocked),
            campaign_id: Some(campaign_id),
            notes,
            blockers,
        });
    }
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
                blocked: None,
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
                blocked: None,
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
                blocked: None,
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
                blocked: None,
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
                blocked: None,
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
                    blocked: None,
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
        if let Some(baseline) = load_parent_baseline(
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
                                let planned = load_existing_child_plan_for_id(
                                    &parts.campaign_id,
                                    &parts.manifest_path,
                                    parent,
                                )?;
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
                                    blocked: None,
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
                        blocked: None,
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
                        blocked: None,
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
            blocked: None,
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
            blocked: None,
            campaign_id: Some(campaign_id),
            notes,
            blockers,
        })
    }
}
// ANCHOR_END: prototype1_reconstruct_early

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
        parts
            .facts
            .report
            .as_mut()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "R12 handoff reconstruction missing report facts".to_string(),
            })?
            .outcome
            .push_str(&format!(
                ";selection={:?};successor={}",
                selection_decision.outcome, selection_decision.candidate_node_id
            ));
        parts.facts.parent_identity = Some(parent.identity().clone());
        if decision.disposition.allows_successor() {
            let entries = journal_entries(&parts.campaign_id)?;
            let evidence = successor_handoff_evidence(
                &entries,
                &parts.repo_root,
                &parts.campaign_id,
                parent.identity(),
                &selection_decision.candidate_node_id,
            )?;
            let Some(evidence) = evidence else {
                if let Some((runtime_id, _)) =
                    sealed_handoff(&parts.campaign_id, &selection_decision.candidate_node_id)?
                {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "selected successor '{}' has sealed History for runtime {runtime_id} but no durable acknowledgement or incomplete successor record; refusing to reconstruct retired authority as R12",
                            selection_decision.candidate_node_id
                        ),
                    });
                }
                blockers.push(format!(
                    "blocked edge r12 -> r13b/r13c: selected successor '{}' has no durable post-retirement evidence for repo_root '{}'; no sealed handoff was found, so R12 authority remains unconsumed",
                    selection_decision.candidate_node_id,
                    parts.repo_root.display()
                ));
                return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
                    parts.into_collected(),
                    parent,
                )));
            };
            match evidence {
                JournalEntry::SuccessorHandoff(handoff) => {
                    let report = parts.facts.report.as_mut().ok_or_else(|| {
                        PrepareError::InvalidBatchSelection {
                            detail: "R12 handoff reconstruction missing report facts".to_string(),
                        }
                    })?;
                    report.successor_runtime = Some(handoff.runtime_id.to_string());
                    report.successor_pid = Some(handoff.pid);
                    report.successor_ready_path = Some(handoff.ready_path);
                    report.outcome.push_str(";successor_handoff=acknowledged");
                    let complete_recorded = parent_complete_recorded(
                        &parts.repo_root,
                        &parts.campaign_id,
                        parent.identity(),
                    )?;
                    let (retired, _lineage) = parent.into_retired_and_lineage();
                    let r13b = typestate::R13bHandoffCommitted::from_collected_parent(
                        parts.into_collected(),
                        retired,
                    );
                    notes.push(
                        "reconstructed R13b successor handoff from same-runtime checkout, sealed History, invocation, and acknowledgement evidence"
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
                JournalEntry::Successor(record) => {
                    let runtime_id = record.runtime_id.ok_or_else(|| {
                        PrepareError::InvalidBatchSelection {
                            detail: format!(
                                "incomplete successor evidence for node '{}' is missing runtime_id",
                                record.node_id
                            ),
                        }
                    })?;
                    let (status, pid, ready_path) = incomplete_report(&record)?;
                    let report = parts.facts.report.as_mut().ok_or_else(|| {
                        PrepareError::InvalidBatchSelection {
                            detail: "R12 handoff reconstruction missing report facts".to_string(),
                        }
                    })?;
                    report.successor_runtime = Some(runtime_id.to_string());
                    report.successor_pid = pid;
                    report.successor_ready_path = ready_path;
                    report
                        .outcome
                        .push_str(&format!(";successor_handoff={status}"));
                    let complete_recorded = parent_complete_recorded(
                        &parts.repo_root,
                        &parts.campaign_id,
                        parent.identity(),
                    )?;
                    let (retired, _lineage) = parent.into_retired_and_lineage();
                    let r13c = typestate::R13cHandoffIncomplete::from_collected_parent(
                        parts.into_collected(),
                        retired,
                    );
                    notes.push(format!(
                        "reconstructed R13c incomplete successor handoff from same-runtime checkout, sealed History, invocation, and {status} evidence"
                    ));
                    if complete_recorded {
                        blockers.push(
                            "parent-complete resource evidence cannot promote R13c without a same-runtime SuccessorHandoff acknowledgement"
                                .into(),
                        );
                    }
                    return Ok(EarlyState::R13c(r13c));
                }
                _ => {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: "successor handoff reconstruction selected a non-successor journal entry"
                            .to_string(),
                    });
                }
            }
        }
        let report =
            parts
                .facts
                .report
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R12 stopped reconstruction missing report facts".to_string(),
                })?;
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
    let manifest_path = campaign_manifest_path(campaign_id)?;
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
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let run_shape = Prototype1StateRunShape::resolve(&command, &manifest_path)?;
    let config = resolve_campaign_config(campaign_id, &CampaignOverrides::default())?;
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

fn successor_handoff_evidence(
    entries: &[JournalEntry],
    repo_root: &Path,
    campaign_id: &CampaignId,
    predecessor: &ParentIdentity,
    node_id: &str,
) -> Result<Option<JournalEntry>, PrepareError> {
    let Some((runtime_id, _)) = sealed_handoff(campaign_id, node_id)? else {
        return Ok(None);
    };
    let Some((index, evidence)) = latest_handoff_entry(entries, campaign_id, node_id, runtime_id)
    else {
        return Ok(None);
    };

    let prefix = &entries[..=index];
    match evidence {
        JournalEntry::SuccessorHandoff(handoff) => {
            let (invocation_path, active_parent_root) =
                spawn_paths(&entries[..index], campaign_id, node_id, handoff.runtime_id)
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "successor handoff for runtime {} has no prior matching spawned record",
                            handoff.runtime_id
                        ),
                    })?;
            if !same_existing_path(&invocation_path, &handoff.invocation_path)
                || !same_existing_path(&active_parent_root, &handoff.active_parent_root)
            {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor handoff for runtime {} does not match its spawned invocation/root",
                        handoff.runtime_id
                    ),
                });
            }
            validate_handoff_attempt(
                prefix,
                repo_root,
                campaign_id,
                predecessor,
                node_id,
                handoff.runtime_id,
                &handoff.invocation_path,
                &handoff.active_parent_root,
            )?;
            if !handoff.ready_path.exists() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "successor handoff ready path '{}' is missing for node '{}'",
                        handoff.ready_path.display(),
                        node_id
                    ),
                });
            }
        }
        JournalEntry::Successor(record) => {
            let runtime_id =
                record
                    .runtime_id
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "successor evidence '{}' for node '{}' is missing runtime_id",
                            record.entry_kind(),
                            node_id
                        ),
                    })?;
            let (invocation_path, active_parent_root) = spawn_paths(
                prefix,
                campaign_id,
                node_id,
                runtime_id,
            )
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor evidence '{}' for runtime {runtime_id} has no matching spawned record",
                    record.entry_kind()
                ),
            })?;
            validate_handoff_attempt(
                prefix,
                repo_root,
                campaign_id,
                predecessor,
                node_id,
                runtime_id,
                &invocation_path,
                &active_parent_root,
            )?;
        }
        _ => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "latest handoff evidence was not a successor journal entry".to_string(),
            });
        }
    }
    Ok(Some(evidence.clone()))
}

fn post_checkout_blocker(
    repo_root: &Path,
    campaign_id: &CampaignId,
    active: &ParentIdentity,
) -> Result<Option<ReconstructionBlocker>, PrepareError> {
    let entries = journal_entries(campaign_id)?;
    let Some(checkout) = entries.iter().rev().find_map(|entry| {
        let JournalEntry::ActiveCheckoutAdvanced(checkout) = entry else {
            return None;
        };
        (checkout.campaign_id == *campaign_id
            && checkout.selected_parent_identity == *active
            && same_existing_path(&checkout.active_parent_root, repo_root))
        .then_some(checkout)
    }) else {
        return Ok(None);
    };
    let predecessor = checkout.previous_parent_identity.clone().ok_or_else(|| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "active checkout advance to successor '{}' is missing previous_parent_identity; refusing to infer predecessor authority",
                active.node_id()
            ),
        }
    })?;

    let attempted = has_handoff_attempt(&entries, campaign_id, active.node_id());
    let sealed = match sealed_handoff(campaign_id, active.node_id()) {
        Ok(sealed) => sealed.is_some(),
        Err(error) => {
            let phase = blocked_phase(attempted);
            return Ok(Some(ReconstructionBlocker {
                phase,
                detail: format!(
                    "post-checkout authority from predecessor '{}' to successor '{}' cannot validate History: {error}",
                    predecessor.node_id(),
                    active.node_id()
                ),
            }));
        }
    };
    let retired = attempted || sealed;

    let evidence = match successor_handoff_evidence(
        &entries,
        repo_root,
        campaign_id,
        &predecessor,
        active.node_id(),
    ) {
        Ok(evidence) => evidence,
        Err(error) => {
            let phase = blocked_phase(retired);
            return Ok(Some(ReconstructionBlocker {
                phase,
                detail: format!(
                    "post-checkout authority from predecessor '{}' to successor '{}' is indeterminate at {phase} and cannot be reconstructed as an exact typed carrier: {error}",
                    predecessor.node_id(),
                    active.node_id()
                ),
            }));
        }
    };
    match evidence {
        Some(JournalEntry::SuccessorHandoff(_)) => Ok(None),
        Some(JournalEntry::Successor(record)) => {
            let (status, _, _) = incomplete_report(&record)?;
            Ok(Some(ReconstructionBlocker {
                phase: WalkPhase::R13c,
                detail: format!(
                    "durable phase is R13c ({status}) for retired predecessor '{}', but the active checkout now contains successor '{}'; exact typed predecessor carrier reconstruction is unavailable, so mutation is blocked rather than reporting R4c or R12",
                    predecessor.node_id(),
                    active.node_id()
                ),
            }))
        }
        Some(_) => Err(PrepareError::InvalidBatchSelection {
            detail: "post-checkout reconstruction selected a non-successor journal entry"
                .to_string(),
        }),
        None => {
            let phase = blocked_phase(sealed);
            Ok(Some(ReconstructionBlocker {
                phase,
                detail: if sealed {
                    format!(
                        "durable phase is R13c for retired predecessor '{}' because History sealed successor '{}', but no successor acknowledgement or incomplete record is durable; exact typed recovery is unavailable, so mutation is blocked",
                        predecessor.node_id(),
                        active.node_id()
                    )
                } else {
                    format!(
                        "durable checkout evidence remains at R12 for predecessor '{}' and selected successor '{}', but the active checkout no longer contains the predecessor Artifact; exact typed predecessor carrier reconstruction is unavailable, so mutation is blocked rather than reporting the successor as R4c",
                        predecessor.node_id(),
                        active.node_id()
                    )
                },
            }))
        }
    }
}

fn blocked_phase(retired: bool) -> WalkPhase {
    if retired {
        WalkPhase::R13c
    } else {
        WalkPhase::R12
    }
}

fn latest_handoff_entry<'a>(
    entries: &'a [JournalEntry],
    campaign_id: &CampaignId,
    node_id: &str,
    runtime_id: crate::loop_graph::RuntimeId,
) -> Option<(usize, &'a JournalEntry)> {
    let mut spawned = false;
    let mut latest = None;
    let mut ack = None;
    let mut failed = None;
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            JournalEntry::SuccessorHandoff(handoff)
                if handoff.campaign_id == *campaign_id
                    && handoff.node_id == node_id
                    && handoff.runtime_id == runtime_id =>
            {
                if spawned && failed.is_none() {
                    ack = Some((index, entry));
                }
            }
            JournalEntry::Successor(record)
                if record.campaign_id == *campaign_id
                    && record.node_id == node_id
                    && record.runtime_id == Some(runtime_id) =>
            {
                match &record.state {
                    successor::State::Spawned { .. } => {
                        spawned = true;
                        latest = Some((index, entry));
                    }
                    successor::State::Ready { .. } | successor::State::Completed { .. } => {
                        latest = Some((index, entry));
                    }
                    successor::State::TimedOut { .. }
                    | successor::State::ExitedBeforeReady { .. } => {
                        failed = Some((index, entry));
                    }
                    successor::State::Selected { .. }
                    | successor::State::Stopped { .. }
                    | successor::State::Checkout { .. } => {}
                }
            }
            _ => {}
        }
    }
    failed.or(ack).or(latest)
}

fn has_handoff_attempt(entries: &[JournalEntry], campaign_id: &CampaignId, node_id: &str) -> bool {
    entries.iter().any(|entry| match entry {
        JournalEntry::SuccessorHandoff(handoff) => {
            handoff.campaign_id == *campaign_id && handoff.node_id == node_id
        }
        JournalEntry::Successor(record) => {
            record.campaign_id == *campaign_id
                && record.node_id == node_id
                && record.runtime_id.is_some()
                && matches!(
                    &record.state,
                    successor::State::Spawned { .. }
                        | successor::State::Ready { .. }
                        | successor::State::TimedOut { .. }
                        | successor::State::ExitedBeforeReady { .. }
                        | successor::State::Completed { .. }
                )
        }
        _ => false,
    })
}

fn spawn_paths(
    entries: &[JournalEntry],
    campaign_id: &CampaignId,
    node_id: &str,
    runtime_id: crate::loop_graph::RuntimeId,
) -> Option<(PathBuf, PathBuf)> {
    entries.iter().rev().find_map(|entry| {
        let JournalEntry::Successor(record) = entry else {
            return None;
        };
        if record.campaign_id != *campaign_id
            || record.node_id != node_id
            || record.runtime_id != Some(runtime_id)
        {
            return None;
        }
        let successor::State::Spawned {
            invocation_path,
            active_parent_root,
            ..
        } = &record.state
        else {
            return None;
        };
        Some((invocation_path.clone(), active_parent_root.clone()))
    })
}

fn validate_handoff_attempt(
    entries: &[JournalEntry],
    repo_root: &Path,
    campaign_id: &CampaignId,
    predecessor: &ParentIdentity,
    node_id: &str,
    runtime_id: crate::loop_graph::RuntimeId,
    invocation_path: &Path,
    active_parent_root: &Path,
) -> Result<(), PrepareError> {
    if !same_existing_path(active_parent_root, repo_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor runtime {runtime_id} active_parent_root '{}' does not match repo_root '{}'",
                active_parent_root.display(),
                repo_root.display()
            ),
        });
    }
    let invocation = match invocation::load_authority(invocation_path)? {
        InvocationAuthority::Successor(invocation) => invocation,
        InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor handoff journal references child invocation '{}', expected successor",
                    invocation_path.display()
                ),
            });
        }
    };
    if invocation.campaign_id() != campaign_id
        || invocation.node_id() != node_id
        || invocation.runtime_id() != runtime_id
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor invocation '{}' does not match runtime {runtime_id} for node '{}'",
                invocation_path.display(),
                node_id
            ),
        });
    }
    let invocation_root =
        invocation
            .active_parent_root()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor invocation '{}' is missing active_parent_root",
                    invocation_path.display()
                ),
            })?;
    if !same_existing_path(invocation_root, repo_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor invocation active_parent_root '{}' does not match repo_root '{}'",
                invocation_root.display(),
                repo_root.display()
            ),
        });
    }
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let selected = validate_prototype1_successor_continuation(&invocation, &manifest_path)?;
    let checkout = entries.iter().rev().any(|entry| match entry {
        JournalEntry::ActiveCheckoutAdvanced(checkout) => {
            checkout.campaign_id == *campaign_id
                && checkout.previous_parent_identity.as_ref() == Some(predecessor)
                && checkout.selected_parent_identity == selected
                && same_existing_path(&checkout.active_parent_root, repo_root)
        }
        _ => false,
    });
    if !checkout {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor runtime {runtime_id} has no matching checkout advance from predecessor '{}' to selected node '{}'",
                predecessor.node_id(),
                selected.node_id()
            ),
        });
    }
    Ok(())
}

fn sealed_handoff(
    campaign_id: &CampaignId,
    node_id: &str,
) -> Result<Option<(crate::loop_graph::RuntimeId, ParentIdentity)>, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let store = FsBlockStore::for_campaign_manifest(&manifest_path);
    let lineage = LineageId::new(campaign_id.clone());
    let state = store
        .lineage_state(&lineage)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_reconstruct_history",
            detail: source.to_string(),
        })?;
    let head = match state.head() {
        StoreHead::Absent { .. } => return Ok(None),
        StoreHead::Present(head) => head,
    };
    let sealed = store
        .sealed_head_block(head)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_reconstruct_history",
            detail: source.to_string(),
        })?;
    let selected = sealed.selected_parent_identity();
    if selected.node_id() != node_id {
        return Ok(None);
    }
    selected.validate_for_command(campaign_id, Some(node_id))?;
    if sealed.selected_successor().artifact() != sealed.active_artifact() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "sealed successor Artifact '{}' does not match sealed active Artifact '{}' for node '{}'",
                sealed.selected_successor().artifact().as_str(),
                sealed.active_artifact().as_str(),
                node_id
            ),
        });
    }
    let ActorRef::Runtime(runtime_id) = sealed.selected_successor().runtime() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("sealed successor for node '{node_id}' is not identified by a runtime"),
        });
    };
    Ok(Some((*runtime_id, selected.clone())))
}

fn incomplete_report(
    record: &successor::Record,
) -> Result<(&'static str, Option<u32>, Option<PathBuf>), PrepareError> {
    match &record.state {
        successor::State::Spawned {
            pid, ready_path, ..
        } => Ok((
            "spawned_without_parent_ack",
            Some(*pid),
            Some(ready_path.clone()),
        )),
        successor::State::Ready { pid, ready_path } => Ok((
            "ready_without_parent_ack",
            Some(*pid),
            Some(ready_path.clone()),
        )),
        successor::State::TimedOut { ready_path, .. } => {
            Ok(("timed_out", None, Some(ready_path.clone())))
        }
        successor::State::ExitedBeforeReady { .. } => Ok(("exited_before_ready", None, None)),
        successor::State::Completed { .. } => Ok(("completed_without_parent_ack", None, None)),
        state => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "R13c reconstruction received pre-retirement successor state {state:?}"
            ),
        }),
    }
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
    let manifest_path = campaign_manifest_path(campaign_id)?;
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
    let manifest_path = campaign_manifest_path(campaign_id)?;
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
    let manifest_path = campaign_manifest_path(campaign_id)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::prototype1_state::{
            event::RecordedAt,
            invocation::SuccessorCompletionStatus,
            journal::{Streams, SuccessorHandoffEntry},
        },
        intervention::CommitPhase,
        loop_graph::RuntimeId,
    };
    use std::ffi::OsString;
    use uuid::Uuid;

    fn runtime(value: u128) -> RuntimeId {
        RuntimeId(Uuid::from_u128(value))
    }

    fn successor(runtime_id: RuntimeId, state: successor::State) -> JournalEntry {
        JournalEntry::Successor(successor::Record {
            runtime_id: Some(runtime_id),
            recorded_at: RecordedAt(10),
            campaign_id: CampaignId::from("campaign-reconstruct-test"),
            node_id: "node-successor".to_string(),
            state,
        })
    }

    fn handoff(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
            recorded_at: RecordedAt(20),
            campaign_id: CampaignId::from("campaign-reconstruct-test"),
            node_id: "node-successor".to_string(),
            runtime_id,
            active_parent_root: PathBuf::from("/tmp/repo"),
            binary_path: PathBuf::from("/tmp/repo/target/debug/ploke-eval"),
            invocation_path: PathBuf::from("/tmp/invocation.json"),
            ready_path: PathBuf::from("/tmp/ready.jsonl"),
            streams: None,
            pid: 42,
        })
    }

    fn spawned(runtime_id: RuntimeId) -> JournalEntry {
        successor(
            runtime_id,
            successor::State::Spawned {
                pid: 42,
                active_parent_root: PathBuf::from("/tmp/repo"),
                binary_path: PathBuf::from("/tmp/ploke-eval"),
                invocation_path: PathBuf::from("/tmp/invocation.json"),
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
                streams: Streams {
                    stdout: PathBuf::from("/tmp/stdout"),
                    stderr: PathBuf::from("/tmp/stderr"),
                },
            },
        )
    }

    fn latest(entries: &[JournalEntry], runtime_id: RuntimeId) -> &JournalEntry {
        latest_handoff_entry(
            entries,
            &CampaignId::from("campaign-reconstruct-test"),
            "node-successor",
            runtime_id,
        )
        .expect("expected successor evidence")
        .1
    }

    fn load_persisted(entries: Vec<JournalEntry>) -> Vec<JournalEntry> {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _guard = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign_id = CampaignId::from("campaign-reconstruct-test");
        let manifest_path = campaign_manifest_path(&campaign_id).expect("campaign path");
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
        for entry in entries {
            journal
                .append_with_receipt(entry)
                .expect("persist journal entry");
        }
        journal_entries(&campaign_id).expect("load reconstruction journal")
    }

    #[test]
    fn pre_retirement_checkout_is_not_handoff_evidence() {
        let entries = [successor(
            runtime(1),
            successor::State::Checkout {
                phase: CommitPhase::After,
                active_parent_root: PathBuf::from("/tmp/repo"),
                selected_branch: "artifact-successor".to_string(),
                installed_commit: Some("abc123".to_string()),
            },
        )];

        assert!(
            latest_handoff_entry(
                &entries,
                &CampaignId::from("campaign-reconstruct-test"),
                "node-successor",
                runtime(1),
            )
            .is_none()
        );
    }

    #[test]
    fn blocked_cursor_distinguishes_checkout_from_retirement() {
        assert_eq!(blocked_phase(false), WalkPhase::R12);
        assert_eq!(blocked_phase(true), WalkPhase::R13c);
    }

    #[test]
    fn runtime_attempts_do_not_override_each_other() {
        let acknowledged = runtime(1);
        let incomplete = runtime(2);
        let entries = [
            spawned(acknowledged),
            handoff(acknowledged),
            spawned(incomplete),
            successor(
                incomplete,
                successor::State::TimedOut {
                    waited_ms: 10_000,
                    ready_path: PathBuf::from("/tmp/ready-2.jsonl"),
                },
            ),
        ];

        assert!(matches!(
            latest(&entries, acknowledged),
            JournalEntry::SuccessorHandoff(handoff)
                if handoff.runtime_id == acknowledged
        ));
        assert!(matches!(
            latest(&entries, incomplete),
            JournalEntry::Successor(record)
                if record.runtime_id == Some(incomplete)
                    && matches!(record.state, successor::State::TimedOut { .. })
        ));
    }

    #[test]
    fn completion_uses_only_same_runtime_handoff() {
        let acknowledged = runtime(1);
        let other = runtime(2);
        let completion = |runtime_id| {
            successor(
                runtime_id,
                successor::State::Completed {
                    status: SuccessorCompletionStatus::Succeeded,
                    completion_path: PathBuf::from("/tmp/completion.json"),
                    trace_path: None,
                    detail: None,
                },
            )
        };

        let same = [
            spawned(acknowledged),
            handoff(acknowledged),
            completion(acknowledged),
        ];
        assert!(matches!(
            latest(&same, acknowledged),
            JournalEntry::SuccessorHandoff(handoff)
                if handoff.runtime_id == acknowledged
        ));

        let different = [
            spawned(acknowledged),
            handoff(acknowledged),
            completion(other),
        ];
        assert!(matches!(
            latest(&different, other),
            JournalEntry::Successor(record)
                if record.runtime_id == Some(other)
                    && matches!(record.state, successor::State::Completed { .. })
        ));
    }

    #[test]
    fn spawned_and_ready_without_ack_are_incomplete_evidence() {
        let runtime_id = runtime(1);
        let spawned = successor(
            runtime_id,
            successor::State::Spawned {
                pid: 42,
                active_parent_root: PathBuf::from("/tmp/repo"),
                binary_path: PathBuf::from("/tmp/ploke-eval"),
                invocation_path: PathBuf::from("/tmp/invocation.json"),
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
                streams: Streams {
                    stdout: PathBuf::from("/tmp/stdout"),
                    stderr: PathBuf::from("/tmp/stderr"),
                },
            },
        );
        assert!(matches!(
            latest(&[spawned], runtime_id),
            JournalEntry::Successor(record)
                if matches!(record.state, successor::State::Spawned { .. })
        ));

        let ready = successor(
            runtime_id,
            successor::State::Ready {
                pid: 42,
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
            },
        );
        assert!(matches!(
            latest(&[ready], runtime_id),
            JournalEntry::Successor(record)
                if matches!(record.state, successor::State::Ready { .. })
        ));
    }

    #[test]
    fn persisted_reconstruct_selection_preserves_ack_and_failure_precedence() {
        let runtime_id = runtime(1);
        let acknowledged = load_persisted(vec![
            spawned(runtime_id),
            handoff(runtime_id),
            successor(
                runtime_id,
                successor::State::Ready {
                    pid: 42,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
        ]);
        assert!(matches!(
            latest(&acknowledged, runtime_id),
            JournalEntry::SuccessorHandoff(entry) if entry.runtime_id == runtime_id
        ));

        let failed = load_persisted(vec![
            spawned(runtime_id),
            successor(
                runtime_id,
                successor::State::TimedOut {
                    waited_ms: 10_000,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
            handoff(runtime_id),
        ]);
        assert!(matches!(
            latest(&failed, runtime_id),
            JournalEntry::Successor(record)
                if matches!(record.state, successor::State::TimedOut { .. })
        ));
    }

    #[test]
    fn orphan_handoff_is_not_reconstruction_authority() {
        let runtime_id = runtime(1);
        let entries = [handoff(runtime_id)];

        assert!(
            latest_handoff_entry(
                &entries,
                &CampaignId::from("campaign-reconstruct-test"),
                "node-successor",
                runtime_id,
            )
            .is_none()
        );
    }
}
