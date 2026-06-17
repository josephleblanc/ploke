//! Side-effect-free reconstruction for early Prototype 1 parent typestates.
//!
//! This slice intentionally stops at R5. It rebuilds typed carriers from durable
//! checkout/campaign/journal evidence without calling the live R4c -> R5 edge,
//! because that edge appends transition evidence.

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
                Prototype1StateRunShape, campaign_manifest_path_for_id,
                prototype1_state_transition_error, resolve_campaign_config_for_id,
                same_existing_path,
            },
            identity::{ParentIdentity, load_parent_identity_optional, parent_identity_path},
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            live_edges::{r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis},
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
        Ok(EarlySnapshot {
            state: Some(EarlyState::R5(typestate::R5::from_collected_parent(
                collected, parent,
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
