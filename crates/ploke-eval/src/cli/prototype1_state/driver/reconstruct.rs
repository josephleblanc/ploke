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
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            live_edges::{
                r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis, r8_to_r9,
                r9_to_r10, r11_to_r12,
            },
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

    let r1 = match reconstruct_r1(repo_root.to_path_buf(), &campaign_id) {
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
                    reconstruct_r1(repo_root.to_path_buf(), &campaign_id).map(EarlyState::R1)?,
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
                    reconstruct_r1(repo_root.to_path_buf(), &campaign_id).map(EarlyState::R1)?,
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
                state: Some(reconstruct_r3(repo_root, &campaign_id).map(EarlyState::R3)?),
                campaign_id: Some(campaign_id),
                notes,
                blockers,
            });
        }
    };
    notes.push("reconstructed R4a by loading Parent<Unchecked>".into());

    let startup = match r4a.advance(r4a_to_r4b_or_r4c) {
        Ok(startup) => startup,
        Err(error) => {
            blockers.push(format_r4a_blocker(repo_root, &error));
            return Ok(EarlySnapshot {
                state: Some(reconstruct_r4a(repo_root, &campaign_id).map(EarlyState::R4a)?),
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
        return Ok(EarlyState::R12(r12));
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
    Ok(EarlyState::R12(r12))
}

fn reconstruct_r1(
    repo_root: PathBuf,
    campaign_id: &CampaignId,
) -> Result<typestate::R1<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    let command = default_command(repo_root.clone(), campaign_id.clone());
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
) -> Result<typestate::R3<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    match reconstruct_r1(repo_root.to_path_buf(), campaign_id)?.advance(r1_to_r2a_or_r3)? {
        typestate::R1Branch::R2a(_) => Err(PrepareError::InvalidBatchSelection {
            detail: "durable reconstruction unexpectedly entered R2a init branch".to_string(),
        }),
        typestate::R1Branch::R3(r3) => Ok(r3),
    }
}

fn reconstruct_r4a(
    repo_root: &Path,
    campaign_id: &CampaignId,
) -> Result<typestate::R4a<Prototype1StateRunShape, ResolvedCampaignConfig>, PrepareError> {
    reconstruct_r3(repo_root, campaign_id)?.advance(r3_to_r4a)
}

fn reconstruct_r4b(
    repo_root: &Path,
    campaign_id: &CampaignId,
) -> Result<
    typestate::R4bGenesisChecked<Prototype1StateRunShape, ResolvedCampaignConfig>,
    PrepareError,
> {
    match reconstruct_r4a(repo_root, campaign_id)?.advance(r4a_to_r4b_or_r4c)? {
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

fn default_command(repo_root: PathBuf, campaign_id: CampaignId) -> Prototype1StateCommand {
    Prototype1StateCommand {
        campaign: Some(campaign_id),
        node_id: None,
        repo_root: Some(repo_root),
        init_parent_identity: false,
        identity_branch: None,
        identity_instance: None,
        handoff_invocation: None,
        stop_after: Prototype1StateStopAfter::Complete,
        successor_selection: Prototype1SuccessorSelection::HistoryScoreChildProp,
        successor_selection_seed: 0,
        successor_selection_metrics: Prototype1TraversalMetrics::Operational,
        candidate_generator: Prototype1CandidateGenerator::BroadHarnessRequest,
        format: InspectOutputFormat::Table,
    }
}
