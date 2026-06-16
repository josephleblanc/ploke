use std::path::PathBuf;

use ploke_records::ids::CampaignId;

use crate::{
    cli::Prototype1StateCommand,
    intervention::{CompleteBaseline, Prototype1ChildBudget, Prototype1SearchPolicy},
};

use super::{
    super::{
        history::surface_attempt,
        inner::Received,
        invocation::SuccessorInvocation,
        journal::PrototypeJournal,
        parent::{ChildFiles, ChildPlan},
    },
    Private,
};

/// Command payload before command-derived inputs have been collected.
#[derive(Debug)]
pub(crate) struct Command<T> {
    command: T,
    _private: Private,
}

impl<T> Command<T> {
    pub(super) fn new(command: T) -> Self {
        Self {
            command,
            _private: Private,
        }
    }

    pub(super) fn into_inner(self) -> T {
        self.command
    }
}

/// Value facts accumulated by the typed parent loop after initial setup.
///
/// These fields are value-level bookkeeping, not extra typestate axes. The type
/// aliases still describe which facts are required at a given phase; the context
/// carries the concrete values that the existing live controller needs next.
#[derive(Debug, Default)]
pub(crate) struct Facts {
    pub(crate) parent_baseline: Option<CompleteBaseline>,
    pub(crate) complete_search_policy: Option<Prototype1SearchPolicy>,
    pub(crate) plan_child_budget: Option<Prototype1ChildBudget>,
    pub(crate) child_plan: Option<ChildPlanFacts>,
}

/// Concrete child-plan values after `Parent<Ready> -> Parent<Selectable>`.
#[derive(Debug)]
pub(crate) struct ChildPlanFacts {
    pub(crate) plan: Received<ChildPlan>,
    pub(crate) children: Vec<ChildFiles>,
    pub(crate) rejected_surface_attempts: Vec<surface_attempt::Evidence>,
}

/// Repo/campaign/manifest/run-shape/journal inputs have been collected.
///
/// This is intentionally generic over `RunShape` and `CampaignConfig`
/// because the current concrete types live outside this module. The live
/// controller can use `R1<Prototype1StateRunShape, ResolvedCampaignConfig>`
/// without moving those definitions during this first wiring slice.
#[derive(Debug)]
pub(crate) struct Collected<RunShape = (), CampaignConfig = ()> {
    command: Prototype1StateCommand,
    repo_root: PathBuf,
    campaign_id: CampaignId,
    manifest_path: PathBuf,
    run_shape: RunShape,
    campaign_config: CampaignConfig,
    journal_path: PathBuf,
    journal: PrototypeJournal,
    handoff_invocation: Option<SuccessorInvocation>,
    facts: Facts,
    _private: Private,
}

/// Owned payload extracted from `Context<Collected<...>>` at the temporary
/// migration boundary back into the existing live implementation.
///
/// Once later R-states are wired, this escape hatch should shrink or move
/// behind typed transition methods.
#[derive(Debug)]
pub(crate) struct CollectedParts<RunShape, CampaignConfig> {
    pub(crate) command: Prototype1StateCommand,
    pub(crate) repo_root: PathBuf,
    pub(crate) campaign_id: CampaignId,
    pub(crate) manifest_path: PathBuf,
    pub(crate) run_shape: RunShape,
    pub(crate) campaign_config: CampaignConfig,
    pub(crate) journal_path: PathBuf,
    pub(crate) journal: PrototypeJournal,
    pub(crate) handoff_invocation: Option<SuccessorInvocation>,
    pub(crate) facts: Facts,
}

impl<RunShape, CampaignConfig> CollectedParts<RunShape, CampaignConfig> {
    pub(crate) fn into_collected(self) -> Collected<RunShape, CampaignConfig> {
        Collected::new(
            self.command,
            self.repo_root,
            self.campaign_id,
            self.manifest_path,
            self.run_shape,
            self.campaign_config,
            self.journal_path,
            self.journal,
        )
        .with_handoff_invocation(self.handoff_invocation)
        .with_facts(self.facts)
    }
}

impl<RunShape, CampaignConfig> Collected<RunShape, CampaignConfig> {
    pub(crate) fn new(
        command: Prototype1StateCommand,
        repo_root: PathBuf,
        campaign_id: CampaignId,
        manifest_path: PathBuf,
        run_shape: RunShape,
        campaign_config: CampaignConfig,
        journal_path: PathBuf,
        journal: PrototypeJournal,
    ) -> Self {
        Self {
            command,
            repo_root,
            campaign_id,
            manifest_path,
            run_shape,
            campaign_config,
            journal_path,
            journal,
            handoff_invocation: None,
            facts: Facts::default(),
            _private: Private,
        }
    }

    pub(crate) fn with_handoff_invocation(
        mut self,
        handoff_invocation: Option<SuccessorInvocation>,
    ) -> Self {
        self.handoff_invocation = handoff_invocation;
        self
    }

    pub(crate) fn with_facts(mut self, facts: Facts) -> Self {
        self.facts = facts;
        self
    }

    pub(crate) fn campaign_id(&self) -> &CampaignId {
        &self.campaign_id
    }

    pub(crate) fn into_parts(self) -> CollectedParts<RunShape, CampaignConfig> {
        CollectedParts {
            command: self.command,
            repo_root: self.repo_root,
            campaign_id: self.campaign_id,
            manifest_path: self.manifest_path,
            run_shape: self.run_shape,
            campaign_config: self.campaign_config,
            journal_path: self.journal_path,
            journal: self.journal,
            handoff_invocation: self.handoff_invocation,
            facts: self.facts,
        }
    }
}
