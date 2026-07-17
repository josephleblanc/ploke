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
    ResolvedCampaignConfig,
    campaign::resolve_explicit_campaign,
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1StateCommand, Prototype1StateStopAfter,
        prototype1_process::validate_prototype1_successor_continuation,
        prototype1_state::{
            backend::GitWorktreeBackend,
            cli_facing::{
                ParentSelection, ParentSelectionOutcome, Prototype1StateRunShape,
                child_plan_message_path_for_parent, load_existing_child_plan_for_id,
                load_parent_baseline, preview_no_selection_continuation,
                preview_successor_continuation, prototype1_state_transition_error,
                reconstruct_child_outcomes_from_store, resolve_parent_policy_budget,
                same_existing_path, validate_existing_child_plan_for_id,
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
    R8(typestate::R8<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R9(typestate::R9<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R10(typestate::R10<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R11a(typestate::R11aRejectedOnly<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R11(typestate::R11FanoutComplete<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R12(typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R13a(typestate::R13aStopped<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R13b(typestate::R13bHandoffCommitted<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R13c(typestate::R13cHandoffIncomplete<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14a(typestate::R14aFinalStopped<Prototype1StateRunShape, ResolvedCampaignConfig>),
    R14b(typestate::R14bFinalHandoff<Prototype1StateRunShape, ResolvedCampaignConfig>),
}

impl EarlyState {
    pub(crate) fn phase(&self) -> WalkPhase {
        match self {
            Self::R1(_) => WalkPhase::R1,
            Self::R3(_) => WalkPhase::R3,
            Self::R4a(_) => WalkPhase::R4a,
            Self::R4b(_) => WalkPhase::R4b,
            Self::R4c(_) => WalkPhase::R4c,
            Self::R5(_) => WalkPhase::R5,
            Self::R6(_) => WalkPhase::R6,
            Self::R7(_) => WalkPhase::R7,
            Self::R8(_) => WalkPhase::R8,
            Self::R9(_) => WalkPhase::R9,
            Self::R10(_) => WalkPhase::R10,
            Self::R11a(_) => WalkPhase::R11a,
            Self::R11(_) => WalkPhase::R11,
            Self::R12(_) => WalkPhase::R12,
            Self::R13a(_) => WalkPhase::R13a,
            Self::R13b(_) => WalkPhase::R13b,
            Self::R13c(_) => WalkPhase::R13c,
            Self::R14a(_) => WalkPhase::R14a,
            Self::R14b(_) => WalkPhase::R14b,
        }
    }
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
    pub(crate) stop_index: Option<usize>,
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
    reconstruct(repo_root, None, None)
}

/// Rebuild exactly the phase named by a verified controller-session cursor.
///
/// Pure projection edges such as R8 -> R9 -> R10 do not create filesystem
/// evidence of their own. The durable controller journal therefore supplies
/// the target boundary, while this function revalidates every production fact
/// required to rebuild that exact typed carrier. R1 and R2a are deliberately
/// excluded: the original command carrier is not yet durable, so rebuilding
/// either phase from fallback defaults would fabricate authority.
pub(crate) fn reconstruct_at(
    repo_root: &Path,
    target: WalkPhase,
) -> Result<EarlySnapshot, PrepareError> {
    reconstruct_exact(repo_root, target, None)
}

/// Rebuild one successor bootstrap phase using the invocation already admitted
/// into its controller-session origin. This explicit authority is required
/// before the successor can write Ready; ordinary historical reconstruction
/// continues to require the parent-observed handoff record.
pub(crate) fn reconstruct_handoff_at(
    repo_root: &Path,
    target: WalkPhase,
    invocation_path: &Path,
) -> Result<EarlySnapshot, PrepareError> {
    reconstruct_exact(repo_root, target, Some(invocation_path))
}

fn reconstruct_exact(
    repo_root: &Path,
    target: WalkPhase,
    handoff: Option<&Path>,
) -> Result<EarlySnapshot, PrepareError> {
    if matches!(target, WalkPhase::R1 | WalkPhase::R2a) {
        return Ok(EarlySnapshot {
            state: None,
            blocked: Some(ReconstructionBlocker {
                phase: target,
                detail: format!(
                    "controller cursor at {target} cannot be resumed because the original Prototype1StateCommand carrier is not durable; controller sessions begin at R3"
                ),
            }),
            campaign_id: None,
            stop_index: None,
            notes: Vec::new(),
            blockers: Vec::new(),
        });
    }
    let mut snapshot = reconstruct(repo_root, Some(target), handoff)?;
    if snapshot.blocked.is_none() && snapshot.state.as_ref().map(EarlyState::phase) != Some(target)
    {
        let reached = snapshot
            .state
            .as_ref()
            .map(EarlyState::phase)
            .map_or_else(|| "none".to_string(), |phase| phase.to_string());
        snapshot.state = None;
        snapshot.blocked = Some(ReconstructionBlocker {
            phase: target,
            detail: format!(
                "controller cursor requires {target}, but durable reconstruction reached {reached}"
            ),
        });
    }
    Ok(snapshot)
}

fn reconstruct(
    repo_root: &Path,
    target: Option<WalkPhase>,
    handoff: Option<&Path>,
) -> Result<EarlySnapshot, PrepareError> {
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
            stop_index: None,
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
    if handoff.is_none()
        && let Some(blocked) = post_checkout_blocker(repo_root, &campaign_id, &identity)?
    {
        return Ok(EarlySnapshot {
            state: None,
            blocked: Some(blocked),
            campaign_id: Some(campaign_id),
            stop_index: None,
            notes,
            blockers,
        });
    }
    let handoff_invocation = match handoff {
        Some(path) => Some(path.to_path_buf()),
        None => infer_successor_handoff_invocation(repo_root, &campaign_id, &identity)?,
    };
    if let Some(path) = handoff_invocation.as_ref() {
        let source = if handoff.is_some() {
            "controller-session successor origin"
        } else {
            "durable handoff journal"
        };
        notes.push(format!(
            "loaded successor handoff invocation from {source}: {}",
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
                stop_index: None,
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R1 from campaign manifest and admitted run profile/defaults".into());
    if target == Some(WalkPhase::R1) {
        return Ok(state_snapshot(
            EarlyState::R1(r1),
            campaign_id,
            notes,
            blockers,
        ));
    }

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
                stop_index: None,
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
                stop_index: None,
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R3 from checkout parent identity".into());
    if target == Some(WalkPhase::R3) {
        return Ok(state_snapshot(
            EarlyState::R3(r3),
            campaign_id,
            notes,
            blockers,
        ));
    }

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
                stop_index: None,
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R4a by loading Parent<Unchecked>".into());
    if target == Some(WalkPhase::R4a) {
        return Ok(state_snapshot(
            EarlyState::R4a(r4a),
            campaign_id,
            notes,
            blockers,
        ));
    }

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
                stop_index: None,
                notes,
                blockers,
            });
        }
    };

    let r4c = match startup {
        typestate::R4aStartupBranch::GenesisChecked(r4b) => {
            if target == Some(WalkPhase::R4b) {
                return Ok(state_snapshot(
                    EarlyState::R4b(r4b),
                    campaign_id,
                    notes,
                    blockers,
                ));
            }
            match r4b.advance(r4b_to_r4c_genesis) {
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
                        stop_index: None,
                        notes,
                        blockers,
                    });
                }
            }
        }
        typestate::R4aStartupBranch::PredecessorReady(r4c) => {
            notes.push("reconstructed R4c from predecessor startup validation".into());
            r4c
        }
    };
    if target == Some(WalkPhase::R4c) {
        return Ok(state_snapshot(
            EarlyState::R4c(r4c),
            campaign_id,
            notes,
            blockers,
        ));
    }

    let typestate::R4cParts { collected, parent } = r4c.into_parts();
    if parent_start_recorded(repo_root, &campaign_id, parent.identity())? {
        notes.push("reconstructed R5 from matching parent-start journal evidence".into());
        let mut parts = collected.into_parts();
        if target == Some(WalkPhase::R5) {
            return Ok(state_snapshot(
                EarlyState::R5(typestate::R5::from_collected_parent(
                    parts.into_collected(),
                    parent,
                )),
                campaign_id,
                notes,
                blockers,
            ));
        }
        if let Some(baseline) = load_parent_baseline(
            &parts.campaign_id,
            &parts.campaign_config,
            &parts.manifest_path,
            parent.identity(),
        )? {
            parts.facts.parent_baseline = Some(baseline);
            notes.push("reconstructed R6 from durable parent baseline evidence".into());
            if target == Some(WalkPhase::R6) {
                return Ok(state_snapshot(
                    EarlyState::R6(typestate::R6::from_collected_parent(
                        parts.into_collected(),
                        parent,
                    )),
                    campaign_id,
                    notes,
                    blockers,
                ));
            }
            match resolve_parent_policy_budget(
                &parts.manifest_path,
                &parts.run_shape,
                parent.identity(),
            ) {
                Ok((policy, budget)) => {
                    parts.facts.complete_search_policy = policy;
                    parts.facts.plan_child_budget = Some(budget);
                    notes.push("reconstructed R7 from run policy and child budget inputs".into());
                    if target == Some(WalkPhase::R7) {
                        return Ok(state_snapshot(
                            EarlyState::R7(typestate::R7::from_collected_parent(
                                parts.into_collected(),
                                parent,
                            )),
                            campaign_id,
                            notes,
                            blockers,
                        ));
                    }
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
                                let mut stop_index = None;
                                let state = reconstruct_after_r8(
                                    r8,
                                    target,
                                    &mut notes,
                                    &mut blockers,
                                    &mut stop_index,
                                )?;
                                return Ok(EarlySnapshot {
                                    state: Some(state),
                                    blocked: None,
                                    campaign_id: Some(campaign_id),
                                    stop_index,
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
                        stop_index: None,
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
                        stop_index: None,
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
            stop_index: None,
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
            stop_index: None,
            notes,
            blockers,
        })
    }
}
// ANCHOR_END: prototype1_reconstruct_early

fn state_snapshot(
    state: EarlyState,
    campaign_id: CampaignId,
    notes: Vec<String>,
    blockers: Vec<String>,
) -> EarlySnapshot {
    EarlySnapshot {
        state: Some(state),
        blocked: None,
        campaign_id: Some(campaign_id),
        stop_index: None,
        notes,
        blockers,
    }
}

fn reconstruct_after_r8(
    r8: typestate::R8<Prototype1StateRunShape, ResolvedCampaignConfig>,
    target: Option<WalkPhase>,
    notes: &mut Vec<String>,
    blockers: &mut Vec<String>,
    boundary: &mut Option<usize>,
) -> Result<EarlyState, PrepareError> {
    if target == Some(WalkPhase::R8) {
        return Ok(EarlyState::R8(r8));
    }
    let r9 = r8.advance(r8_to_r9)?;
    notes.push("reconstructed R9 by shaping existing child-plan schedule".into());
    if target == Some(WalkPhase::R9) {
        return Ok(EarlyState::R9(r9));
    }
    let r10 = r9.advance(r9_to_r10)?;
    notes.push("reconstructed R10 by resolving selection strategy inputs".into());
    if target == Some(WalkPhase::R10) {
        return Ok(EarlyState::R10(r10));
    }

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
    let selection_strategy = match parts.facts.selection_strategy.clone() {
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
        if target == Some(WalkPhase::R11a) {
            return Ok(EarlyState::R11a(r11a));
        }
        let r12 = r11_to_r12(typestate::R10FanoutBranch::RejectedOnly(r11a))?;
        notes.push("reconstructed R12 report facts from rejected-only evidence".into());
        return reconstruct_after_r12(r12, target, notes, blockers, boundary);
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
        let result = if parts.run_shape.eval_storage_backend
            == crate::cli::prototype1_state::profile::EvalStorageBackend::DualStrict
        {
            load_selection_outcome(
                &parts.manifest_path,
                &parts.campaign_id,
                parent_identity.parent_id(),
            )
        } else {
            ParentSelection::new(
                &parts.manifest_path,
                &parent_identity,
                &child_outcomes,
                &rejected_surface_attempts,
            )
            .select_successor_attempt(parts.run_shape.successor_selection_seed, selection_strategy)
        };
        match result {
            Ok(selection) => Some(selection),
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
    if parts.run_shape.eval_storage_backend
        != crate::cli::prototype1_state::profile::EvalStorageBackend::DualStrict
        && let Some(outcome) = selection.as_ref()
        && let Err(error) = validate_selection_hash(
            &parts.manifest_path,
            &parts.campaign_id,
            parent_identity.parent_id(),
            parts.run_shape.eval_storage_backend,
            outcome,
        )
    {
        blockers.push(format!(
            "blocked edge r10 -> r11: reconstructed selection does not match its durable owner-DB receipt: {error}"
        ));
        return Ok(EarlyState::R10(typestate::R10::from_collected_parent(
            parts.into_collected(),
            parent,
        )));
    }
    let outcome_count = child_outcomes.len();
    parts.facts.child_outcomes = Some(child_outcomes);
    parts.facts.selection = selection;
    parts.facts.rejected_attempt_payloads = None;
    let r11 = typestate::R11FanoutComplete::from_collected_parent(parts.into_collected(), parent);
    notes.push(format!(
        "reconstructed R11 from {outcome_count} channel-derived child outcomes"
    ));
    if target == Some(WalkPhase::R11) {
        return Ok(EarlyState::R11(r11));
    }
    let r12 = r11_to_r12(typestate::R10FanoutBranch::FanoutComplete(r11))?;
    notes.push("reconstructed R12 report facts from child outcomes".into());
    reconstruct_after_r12(r12, target, notes, blockers, boundary)
}

fn load_selection_outcome(
    manifest_path: &Path,
    campaign_id: &CampaignId,
    parent_id: &str,
) -> Result<ParentSelectionOutcome, PrepareError> {
    let db_path =
        crate::cli::prototype1_state::eval_store::prototype1_eval_store_db_path(manifest_path);
    if !db_path.is_file() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "dual-strict selection reconstruction requires owner eval DB at '{}'",
                db_path.display()
            ),
        });
    }
    let entry = crate::cli::prototype1_state::eval_store::load_selection_receipt(
        &db_path,
        campaign_id,
        parent_id,
    )
    .map_err(|error| PrepareError::DatabaseSetup {
        phase: "eval_selection_receipt_read",
        detail: format!(
            "failed to load selection receipt for campaign '{campaign_id}' parent '{parent_id}' from '{}': {error}",
            db_path.display()
        ),
    })?
    .ok_or_else(|| PrepareError::InvalidBatchSelection {
        detail: format!(
            "dual-strict selection reconstruction found no typed owner-DB receipt for campaign '{campaign_id}' parent '{parent_id}'"
        ),
    })?;
    ParentSelectionOutcome::from_entry(entry)
}

fn validate_selection_hash(
    manifest_path: &Path,
    campaign_id: &CampaignId,
    parent_id: &str,
    backend: crate::cli::prototype1_state::profile::EvalStorageBackend,
    outcome: &ParentSelectionOutcome,
) -> Result<(), PrepareError> {
    if !backend.mirrors_owner_db() {
        return Ok(());
    }
    let db_path =
        crate::cli::prototype1_state::eval_store::prototype1_eval_store_db_path(manifest_path);
    if !db_path.is_file() {
        return Ok(());
    }
    let persisted = crate::cli::prototype1_state::eval_store::load_selection_hash(
        &db_path,
        campaign_id,
        parent_id,
    )
    .map_err(|error| PrepareError::DatabaseSetup {
        phase: "eval_selection_decision_read",
        detail: format!(
            "failed to load selection receipt for campaign '{campaign_id}' parent '{parent_id}' from '{}': {error}",
            db_path.display()
        ),
    })?;
    let Some(persisted) = persisted else {
        return Ok(());
    };
    let reconstructed =
        outcome
            .entry()?
            .decision_hash()
            .map_err(|error| PrepareError::InvalidBatchSelection {
                detail: format!("failed to hash reconstructed selection receipt: {error}"),
            })?;
    if persisted != reconstructed.as_str() {
        let replayed = outcome.selected().map_or_else(
            || "no_selection".to_string(),
            |(decision, _)| {
                format!(
                    "node={},outcome={:?},disposition={}",
                    decision.candidate_node_id,
                    decision.outcome,
                    decision
                        .selected_branch_disposition()
                        .unwrap_or("<not-selected>")
                )
            },
        );
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selection receipt hash mismatch for campaign '{campaign_id}' parent '{parent_id}': persisted={persisted}, reconstructed={}, replayed={replayed}",
                reconstructed.as_str()
            ),
        });
    }
    Ok(())
}

fn reconstruct_after_r12(
    r12: typestate::R12<Prototype1StateRunShape, ResolvedCampaignConfig>,
    target: Option<WalkPhase>,
    notes: &mut Vec<String>,
    blockers: &mut Vec<String>,
    boundary: &mut Option<usize>,
) -> Result<EarlyState, PrepareError> {
    if target == Some(WalkPhase::R12) {
        return Ok(EarlyState::R12(r12));
    }
    let typestate::SelectableParts { collected, parent } = r12.into_parts();
    let mut parts = collected.into_parts();
    if let Some((selection_decision, selection_material)) = parts
        .facts
        .selection
        .as_ref()
        .and_then(ParentSelectionOutcome::selected)
    {
        let selected_artifact = selection_material.selected_artifact()?;
        let node = selected_artifact.node();
        let policy = parts.facts.complete_search_policy.as_ref().ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail: "R12 handoff reconstruction missing search policy".to_string(),
            }
        })?;
        let decision = preview_successor_continuation(
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
            let Some((evidence_index, evidence)) = evidence else {
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
                    let complete_recorded = parent_complete_after(
                        &parts.repo_root,
                        &parts.campaign_id,
                        parent.identity(),
                        evidence_index,
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
                    if target == Some(WalkPhase::R13b) || !complete_recorded {
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
                    let complete_recorded = parent_complete_after(
                        &parts.repo_root,
                        &parts.campaign_id,
                        parent.identity(),
                        evidence_index,
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
        let Some(stop_index) = selected_stop(
            &parts.campaign_id,
            parent.identity(),
            &selection_decision.candidate_node_id,
            &decision,
            selection_decision,
        )?
        else {
            blockers.push(format!(
                "blocked edge r12 -> r13a: selected successor '{}' stopped by policy but no exact turn-scoped stopped successor record exists",
                selection_decision.candidate_node_id
            ));
            return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        };
        report.outcome.push_str(&format!(
            ";successor_handoff=skipped:{:?}",
            decision.disposition
        ));
        let complete_recorded = parent_complete_after(
            &parts.repo_root,
            &parts.campaign_id,
            parent.identity(),
            stop_index,
        )?;
        *boundary = Some(stop_index);
        let r13a = typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent);
        notes.push("reconstructed R13a selected-successor stopped continuation from durable successor record".into());
        if target == Some(WalkPhase::R13a) || !complete_recorded {
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

    let expected_receipt = match parts.facts.selection.as_ref() {
        Some(ParentSelectionOutcome::NoSelection { entry }) => {
            successor::SelectionReceipt::Completed {
                hash: entry.receipt_hash().map_err(|error| {
                    PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "failed to hash reconstructed no-selection receipt: {error}"
                        ),
                    }
                })?,
            }
        }
        None if parts.facts.rejected_attempt_payloads.is_some() => {
            successor::SelectionReceipt::NotRun
        }
        None => {
            blockers.push(format!(
                "blocked edge r12 -> r13a: parent '{}' has neither a successor-selection result nor rejected-only procedure evidence",
                parent.identity().parent_id()
            ));
            return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
                parts.into_collected(),
                parent,
            )));
        }
        Some(ParentSelectionOutcome::Selected { .. }) => unreachable!(
            "selected successor outcome was handled by the selected reconstruction branch"
        ),
    };
    let expected_decision =
        preview_no_selection_continuation(&parts.manifest_path, parent.identity())?;
    let stop_index = no_selection_stop(
        &parts.campaign_id,
        parent.identity(),
        &expected_decision,
        &expected_receipt,
    )?;
    let Some(stop_index) = stop_index else {
        blockers.push(format!(
            "blocked edge r12 -> r13a: parent '{}' has no durable stopped record bound to the reconstructed selection receipt",
            parent.identity().parent_id()
        ));
        return Ok(EarlyState::R12(typestate::R12::from_collected_parent(
            parts.into_collected(),
            parent,
        )));
    };
    if parts.run_shape.stop_after == Prototype1StateStopAfter::Complete {
        let report =
            parts
                .facts
                .report
                .as_mut()
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "R12 no-selection reconstruction missing report facts".to_string(),
                })?;
        report.outcome.push_str(match expected_receipt {
            successor::SelectionReceipt::Completed { .. } => ";selection=none",
            successor::SelectionReceipt::NotRun => ";selection=not_run",
        });
        report.outcome.push_str(&format!(
            ";successor_handoff=skipped:{:?}",
            expected_decision.disposition
        ));
    }
    let complete_recorded = parent_complete_after(
        &parts.repo_root,
        &parts.campaign_id,
        parent.identity(),
        stop_index,
    )?;
    *boundary = Some(stop_index);
    parts.facts.parent_identity = Some(parent.identity().clone());
    let r13a = typestate::R13aStopped::from_collected_parent(parts.into_collected(), parent);
    notes.push(
        "reconstructed R13a stopped continuation from a durable selection-receipt-bound successor record"
            .into(),
    );
    if target == Some(WalkPhase::R13a) || !complete_recorded {
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
    let config = resolve_explicit_campaign(campaign_id)?;
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
) -> Result<Option<(usize, JournalEntry)>, PrepareError> {
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
    Ok(Some((index, evidence.clone())))
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
        Some((_, JournalEntry::SuccessorHandoff(_))) => Ok(None),
        Some((_, JournalEntry::Successor(record))) => {
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
        Some((_, _)) => Err(PrepareError::InvalidBatchSelection {
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
        successor::State::Ready {
            pid, ready_path, ..
        } => Ok((
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

fn active_turn_start(
    entries: &[JournalEntry],
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
) -> Option<usize> {
    entries.iter().rposition(|entry| {
        matches!(
            entry,
            JournalEntry::ParentStarted(started)
                if started.campaign_id == *campaign_id
                    && started.parent_identity == *identity
        )
    })
}

fn selected_stop(
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
    node_id: &str,
    expected_decision: &crate::intervention::Prototype1ContinuationDecision,
    expected_selection: &crate::successor_selection::SuccessorDecision,
) -> Result<Option<usize>, PrepareError> {
    let entries = journal_entries(campaign_id)?;
    let Some(turn_start) = active_turn_start(&entries, campaign_id, identity) else {
        return Ok(None);
    };
    let mut found = None;
    for (index, entry) in entries.into_iter().enumerate().skip(turn_start + 1) {
        let JournalEntry::Successor(record) = entry else {
            continue;
        };
        if record.campaign_id != *campaign_id || record.node_id != node_id {
            continue;
        }
        let successor::State::Stopped {
            decision,
            selection_decision,
            selection_receipt,
        } = record.state
        else {
            continue;
        };
        found = Some(index);
        if record.runtime_id.is_some()
            || decision != *expected_decision
            || selection_decision.as_ref() != Some(expected_selection)
            || selection_receipt.is_some()
        {
            return Ok(None);
        }
    }
    Ok(found)
}

fn no_selection_stop(
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
    expected_decision: &crate::intervention::Prototype1ContinuationDecision,
    expected: &successor::SelectionReceipt,
) -> Result<Option<usize>, PrepareError> {
    let entries = journal_entries(campaign_id)?;
    let Some(turn_start) = active_turn_start(&entries, campaign_id, identity) else {
        return Ok(None);
    };
    let mut found = None;
    for (index, entry) in entries.into_iter().enumerate().skip(turn_start + 1) {
        let JournalEntry::Successor(record) = entry else {
            continue;
        };
        if record.campaign_id != *campaign_id || record.node_id != identity.node_id() {
            continue;
        }
        let successor::State::Stopped {
            decision,
            selection_decision,
            selection_receipt,
        } = record.state
        else {
            continue;
        };
        found = Some(index);
        if record.runtime_id.is_some()
            || decision != *expected_decision
            || selection_decision.is_some()
            || selection_receipt.as_ref() != Some(expected)
        {
            return Ok(None);
        }
    }
    Ok(found)
}

fn journal_entries(campaign_id: &CampaignId) -> Result<Vec<JournalEntry>, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })
}

fn parent_complete_after(
    repo_root: &Path,
    campaign_id: &CampaignId,
    identity: &ParentIdentity,
    boundary: usize,
) -> Result<bool, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
    let entries = journal.load_entries().map_err(|error| {
        prototype1_state_transition_error("prototype1_reconstruct_journal", error.to_string())
    })?;
    let Some(turn_start) = active_turn_start(&entries, campaign_id, identity) else {
        return Ok(false);
    };
    if boundary <= turn_start || boundary >= entries.len() {
        return Ok(false);
    }
    let target_dir = repo_root.join("target");
    Ok(entries
        .into_iter()
        .skip(boundary + 1)
        .any(|entry| match entry {
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
        stop_after: None,
        successor_selection: None,
        successor_selection_seed: None,
        successor_selection_metrics: None,
        candidate_generator: None,
        format: InspectOutputFormat::Table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::prototype1_state::{
            event::RecordedAt,
            history::HistoryHash,
            invocation::SuccessorCompletionStatus,
            journal::{Streams, SuccessorHandoffEntry},
        },
        intervention::{
            CommitPhase, Prototype1ContinuationDecision, Prototype1ContinuationDisposition,
            RecordStore,
        },
        loop_graph::RuntimeId,
    };
    use std::ffi::OsString;
    use uuid::Uuid;

    fn runtime(value: u128) -> RuntimeId {
        RuntimeId(Uuid::from_u128(value))
    }

    #[test]
    fn command_phase_cursor_is_not_reconstructed_from_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");

        for phase in [WalkPhase::R1, WalkPhase::R2a] {
            let snapshot = reconstruct_at(tmp.path(), phase).expect("blocked snapshot");

            assert!(snapshot.state.is_none());
            let blocked = snapshot.blocked.expect("structured blocker");
            assert_eq!(blocked.phase, phase);
            assert!(blocked.detail.contains("original Prototype1StateCommand"));
        }
    }

    #[test]
    fn requested_phase_mismatch_is_not_promotable() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let snapshot = reconstruct_at(tmp.path(), WalkPhase::R3).expect("blocked snapshot");

        assert!(snapshot.state.is_none());
        let blocked = snapshot.blocked.expect("structured blocker");
        assert_eq!(blocked.phase, WalkPhase::R3);
        assert!(
            blocked
                .detail
                .contains("durable reconstruction reached none")
        );
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
            acceptance: None,
        })
    }

    fn spawned(runtime_id: RuntimeId) -> JournalEntry {
        successor(
            runtime_id,
            successor::State::Spawned {
                pid: 42,
                incarnation: None,
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

    fn no_selection_decision(total_nodes: u32) -> Prototype1ContinuationDecision {
        Prototype1ContinuationDecision {
            disposition: Prototype1ContinuationDisposition::StopNoSelectedBranch,
            selected_next_branch_id: None,
            selected_branch_disposition: None,
            next_generation: 1,
            total_nodes_after_continue: total_nodes,
        }
    }

    fn selected_continuation(total_nodes: u32) -> Prototype1ContinuationDecision {
        Prototype1ContinuationDecision {
            disposition: Prototype1ContinuationDisposition::StopMaxGenerations,
            selected_next_branch_id: Some("branch-selected".to_string()),
            selected_branch_disposition: Some("keep".to_string()),
            next_generation: 2,
            total_nodes_after_continue: total_nodes,
        }
    }

    fn selected_decision() -> crate::successor_selection::SuccessorDecision {
        crate::successor_selection::SuccessorDecision {
            procedure_id: crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID.to_string(),
            candidate_node_id: "node-selected".to_string(),
            selected_branch_id: Some("branch-selected".to_string()),
            branch_disposition: "keep".to_string(),
            outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        }
    }

    fn no_selection_parent() -> ParentIdentity {
        ParentIdentity::from_record_for_test(ploke_records::identity::ParentIdentityRecord {
            schema_version: crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION
                .to_string(),
            campaign_id: CampaignId::from("campaign-reconstruct-test"),
            parent_id: "node-parent".to_string(),
            node_id: "node-parent".to_string(),
            generation: 1,
            instance_id: Some("BurntSushi__ripgrep-2209".to_string()),
            previous_parent_id: Some("node-predecessor".to_string()),
            parent_node_id: Some("node-predecessor".to_string()),
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("prototype1-node-parent".to_string()),
            created_at: "2026-07-17T00:00:00Z".to_string(),
        })
    }

    fn parent_started_record(parent: &ParentIdentity) -> JournalEntry {
        JournalEntry::ParentStarted(journal::ParentStartedEntry {
            recorded_at: RecordedAt(6),
            campaign_id: parent.campaign_id().clone(),
            parent_identity: parent.clone(),
            repo_root: PathBuf::from("/tmp/repo"),
            handoff_runtime_id: Some(runtime(9)),
            pid: 42,
        })
    }

    fn no_selection_matches(
        mut entries: Vec<JournalEntry>,
        expected_decision: &Prototype1ContinuationDecision,
        expected_receipt: &successor::SelectionReceipt,
    ) -> bool {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _guard = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign_id = CampaignId::from("campaign-reconstruct-test");
        let manifest_path = campaign_manifest_path(&campaign_id).expect("campaign path");
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
        let parent = no_selection_parent();
        if !entries
            .iter()
            .any(|entry| matches!(entry, JournalEntry::ParentStarted(_)))
        {
            entries.insert(0, parent_started_record(&parent));
        }
        for entry in entries {
            journal.append(entry).expect("persist journal entry");
        }
        no_selection_stop(&campaign_id, &parent, expected_decision, expected_receipt)
            .expect("match stopped record")
            .is_some()
    }

    fn selected_matches(
        mut entries: Vec<JournalEntry>,
        expected_decision: &Prototype1ContinuationDecision,
        expected_selection: &crate::successor_selection::SuccessorDecision,
    ) -> bool {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _guard = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign_id = CampaignId::from("campaign-reconstruct-test");
        let manifest_path = campaign_manifest_path(&campaign_id).expect("campaign path");
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
        let parent = no_selection_parent();
        if !entries
            .iter()
            .any(|entry| matches!(entry, JournalEntry::ParentStarted(_)))
        {
            entries.insert(0, parent_started_record(&parent));
        }
        for entry in entries {
            journal.append(entry).expect("persist journal entry");
        }
        selected_stop(
            &campaign_id,
            &parent,
            "node-selected",
            expected_decision,
            expected_selection,
        )
        .expect("match selected stopped record")
        .is_some()
    }

    fn parent_complete_record(parent: &ParentIdentity) -> JournalEntry {
        JournalEntry::Resource(journal::resource::Sample {
            recorded_at: RecordedAt(8),
            campaign_id: parent.campaign_id().clone(),
            parent_id: parent.parent_id().to_string(),
            node_id: parent.node_id().to_string(),
            generation: parent.generation(),
            runtime_id: Some(runtime(9)),
            subject: journal::resource::Subject::CargoTarget,
            phase: journal::resource::Phase::ParentComplete,
            path: PathBuf::from("/tmp/repo/target"),
            status: journal::resource::Status::Measured,
            bytes: Some(1),
            error: None,
        })
    }

    fn no_selection_record(
        decision: Prototype1ContinuationDecision,
        hash: HistoryHash,
    ) -> JournalEntry {
        JournalEntry::Successor(successor::Record::stopped_without_selection(
            CampaignId::from("campaign-reconstruct-test"),
            "node-parent".to_string(),
            decision,
            hash,
        ))
    }

    fn no_attempt_record(decision: Prototype1ContinuationDecision) -> JournalEntry {
        JournalEntry::Successor(successor::Record::stopped_without_attempt(
            CampaignId::from("campaign-reconstruct-test"),
            "node-parent".to_string(),
            decision,
        ))
    }

    fn selected_record(
        decision: Prototype1ContinuationDecision,
        selection: crate::successor_selection::SuccessorDecision,
    ) -> JournalEntry {
        JournalEntry::Successor(successor::Record::stopped(
            CampaignId::from("campaign-reconstruct-test"),
            "node-selected".to_string(),
            decision,
            selection,
        ))
    }

    fn selected_lifecycle(state: successor::State) -> JournalEntry {
        JournalEntry::Successor(successor::Record {
            runtime_id: Some(runtime(9)),
            recorded_at: RecordedAt(9),
            campaign_id: CampaignId::from("campaign-reconstruct-test"),
            node_id: "node-selected".to_string(),
            state,
        })
    }

    fn prior_parent_lifecycle(state: successor::State) -> JournalEntry {
        JournalEntry::Successor(successor::Record {
            runtime_id: Some(runtime(9)),
            recorded_at: RecordedAt(5),
            campaign_id: CampaignId::from("campaign-reconstruct-test"),
            node_id: "node-parent".to_string(),
            state,
        })
    }

    #[test]
    fn no_selection_reconstruction_requires_exact_stopped_receipt() {
        let decision = no_selection_decision(1);
        let hash = HistoryHash::of_bytes(b"selection-receipt");
        let receipt = successor::SelectionReceipt::Completed { hash: hash.clone() };

        assert!(!no_selection_matches(Vec::new(), &decision, &receipt));
        assert!(no_selection_matches(
            vec![no_selection_record(decision.clone(), hash)],
            &decision,
            &receipt,
        ));
        assert!(!no_selection_matches(
            vec![no_selection_record(
                decision.clone(),
                HistoryHash::of_bytes(b"different-receipt"),
            )],
            &decision,
            &receipt,
        ));
    }

    #[test]
    fn rejected_only_reconstruction_requires_not_run_receipt() {
        let decision = no_selection_decision(1);

        assert!(no_selection_matches(
            vec![no_attempt_record(decision.clone())],
            &decision,
            &successor::SelectionReceipt::NotRun,
        ));
        assert!(!no_selection_matches(
            vec![no_selection_record(
                decision.clone(),
                HistoryHash::of_bytes(b"selection-receipt"),
            )],
            &decision,
            &successor::SelectionReceipt::NotRun,
        ));
    }

    #[test]
    fn no_selection_reconstruction_rejects_mismatched_continuation() {
        let expected = no_selection_decision(1);
        let hash = HistoryHash::of_bytes(b"selection-receipt");
        let receipt = successor::SelectionReceipt::Completed { hash: hash.clone() };

        assert!(!no_selection_matches(
            vec![no_selection_record(no_selection_decision(2), hash)],
            &expected,
            &receipt,
        ));
    }

    #[test]
    fn no_selection_reconstruction_rejects_conflicting_journal_history() {
        let expected = no_selection_decision(1);
        let hash = HistoryHash::of_bytes(b"selection-receipt");
        let receipt = successor::SelectionReceipt::Completed { hash: hash.clone() };

        assert!(!no_selection_matches(
            vec![
                no_selection_record(expected.clone(), hash.clone()),
                no_selection_record(no_selection_decision(2), hash),
            ],
            &expected,
            &receipt,
        ));
    }

    #[test]
    fn no_selection_reconstruction_allows_prior_successor_lifecycle() {
        let expected = no_selection_decision(2);
        let hash = HistoryHash::of_bytes(b"selection-receipt");
        let receipt = successor::SelectionReceipt::Completed { hash: hash.clone() };
        let parent = no_selection_parent();

        assert!(no_selection_matches(
            vec![
                prior_parent_lifecycle(successor::State::Spawned {
                    pid: 42,
                    incarnation: None,
                    active_parent_root: PathBuf::from("/tmp/repo"),
                    binary_path: PathBuf::from("/tmp/ploke-eval"),
                    invocation_path: PathBuf::from("/tmp/invocation.json"),
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                    streams: Streams {
                        stdout: PathBuf::from("/tmp/stdout"),
                        stderr: PathBuf::from("/tmp/stderr"),
                    },
                }),
                no_selection_record(
                    no_selection_decision(1),
                    HistoryHash::of_bytes(b"prior-turn-receipt"),
                ),
                parent_started_record(&parent),
                no_selection_record(expected.clone(), hash),
                JournalEntry::Resource(journal::resource::Sample {
                    recorded_at: RecordedAt(8),
                    campaign_id: parent.campaign_id().clone(),
                    parent_id: parent.parent_id().to_string(),
                    node_id: parent.node_id().to_string(),
                    generation: parent.generation(),
                    runtime_id: Some(runtime(9)),
                    subject: journal::resource::Subject::CargoTarget,
                    phase: journal::resource::Phase::ParentComplete,
                    path: PathBuf::from("/tmp/repo/target"),
                    status: journal::resource::Status::Measured,
                    bytes: Some(1),
                    error: None,
                }),
                prior_parent_lifecycle(successor::State::Completed {
                    status: SuccessorCompletionStatus::Succeeded,
                    completion_path: PathBuf::from("/tmp/completion.json"),
                    trace_path: None,
                    detail: None,
                }),
            ],
            &expected,
            &receipt,
        ));
    }

    #[test]
    fn selected_stop_reconstruction_requires_exact_active_turn_evidence() {
        let expected = selected_continuation(2);
        let selection = selected_decision();

        assert!(selected_matches(
            vec![selected_record(expected.clone(), selection.clone())],
            &expected,
            &selection,
        ));
        assert!(!selected_matches(
            vec![selected_record(selected_continuation(3), selection.clone())],
            &expected,
            &selection,
        ));

        let mut wrong_selection = selection.clone();
        wrong_selection.selected_branch_id = Some("branch-other".to_string());
        assert!(!selected_matches(
            vec![selected_record(expected.clone(), wrong_selection.clone())],
            &expected,
            &selection,
        ));

        let parent = no_selection_parent();
        assert!(selected_matches(
            vec![
                selected_record(selected_continuation(3), wrong_selection),
                parent_started_record(&parent),
                selected_record(expected.clone(), selection.clone()),
                selected_lifecycle(successor::State::Completed {
                    status: SuccessorCompletionStatus::Succeeded,
                    completion_path: PathBuf::from("/tmp/completion.json"),
                    trace_path: None,
                    detail: None,
                }),
            ],
            &expected,
            &selection,
        ));
    }

    #[test]
    fn parent_complete_requires_current_post_boundary_evidence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _guard = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign_id = CampaignId::from("campaign-reconstruct-test");
        let manifest_path = campaign_manifest_path(&campaign_id).expect("campaign path");
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
        let parent = no_selection_parent();
        let decision = no_selection_decision(1);

        for entry in [
            parent_started_record(&parent),
            parent_complete_record(&parent),
            no_attempt_record(decision.clone()),
        ] {
            journal.append(entry).expect("persist journal entry");
        }
        let boundary = journal.load_entries().expect("load current turn").len() - 1;
        assert!(
            !parent_complete_after(Path::new("/tmp/repo"), &campaign_id, &parent, boundary,)
                .expect("reject pre-boundary completion")
        );

        journal
            .append(parent_complete_record(&parent))
            .expect("persist post-boundary completion");
        assert!(
            parent_complete_after(Path::new("/tmp/repo"), &campaign_id, &parent, boundary,)
                .expect("accept post-boundary completion")
        );

        for entry in [parent_started_record(&parent), no_attempt_record(decision)] {
            journal.append(entry).expect("persist next turn entry");
        }
        let next_boundary = journal.load_entries().expect("load next turn").len() - 1;
        assert!(
            !parent_complete_after(Path::new("/tmp/repo"), &campaign_id, &parent, next_boundary,)
                .expect("reject stale prior-turn completion")
        );

        journal
            .append(parent_complete_record(&parent))
            .expect("persist next-turn completion");
        assert!(
            parent_complete_after(Path::new("/tmp/repo"), &campaign_id, &parent, next_boundary,)
                .expect("accept next-turn completion")
        );
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
                incarnation: None,
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
                controller: None,
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
                    controller: None,
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
