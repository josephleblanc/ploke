use std::{fmt, path::PathBuf};

use ploke_records::ids::CampaignId;

use crate::{
    cli::Prototype1StateCommand,
    intervention::{
        CompleteBaseline, Prototype1ChildBudget, Prototype1ChildScheduleMode, Prototype1NodeStatus,
        Prototype1SearchPolicy,
    },
    successor_selection::SuccessorDecision,
};

use super::{
    super::{
        cli_facing::{ActiveSelectionStrategy, PlannedChildOutcome, SelectionSealMaterial},
        history::surface_attempt,
        identity::ParentIdentity,
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

// ANCHOR: prototype1_context_facts
// ANCHOR: prototype1_context_facts_fields
/// Value facts accumulated by the typed parent loop after initial setup.
///
/// These fields are value-level bookkeeping, not extra typestate axes. The type
/// aliases still describe which facts are required at a given phase; the context
/// carries the concrete values that the existing live controller needs next.
#[derive(Default)]
pub(crate) struct Facts {
    pub(crate) parent_baseline: Option<CompleteBaseline>,
    pub(crate) complete_search_policy: Option<Prototype1SearchPolicy>,
    pub(crate) plan_child_budget: Option<Prototype1ChildBudget>,
    pub(crate) planned_child_count: Option<usize>,
    pub(crate) child_budget: Option<Prototype1ChildBudget>,
    pub(crate) child_schedule_mode: Option<Prototype1ChildScheduleMode>,
    pub(crate) child_plan: Option<ChildPlanFacts>,
    pub(crate) selection_strategy: Option<ActiveSelectionStrategy>,
    pub(crate) child_outcomes: Option<Vec<PlannedChildOutcome>>,
    pub(crate) selection: Option<(SuccessorDecision, SelectionSealMaterial)>,
    pub(crate) rejected_attempt_payloads: Option<usize>,
    pub(crate) report: Option<ReportFacts>,
    pub(crate) parent_identity: Option<ParentIdentity>,
}
// ANCHOR_END: prototype1_context_facts_fields

/// Concrete child-plan values after `Parent<Ready> -> Parent<Selectable>`.
pub(crate) struct ChildPlanFacts {
    pub(crate) plan: Received<ChildPlan>,
    pub(crate) children: Vec<ChildFiles>,
    pub(crate) rejected_surface_attempts: Vec<surface_attempt::Evidence>,
}

pub(crate) struct ReportFacts {
    pub(crate) outcome: String,
    pub(crate) node_id: String,
    pub(crate) node_status: Prototype1NodeStatus,
    pub(crate) workspace_root: PathBuf,
    pub(crate) binary_path: PathBuf,
    pub(crate) child_runtime: Option<String>,
    pub(crate) successor_runtime: Option<String>,
    pub(crate) successor_pid: Option<u32>,
    pub(crate) successor_ready_path: Option<PathBuf>,
}
// ANCHOR_END: prototype1_context_facts

impl fmt::Debug for ChildPlanFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildPlanFacts")
            .field("plan", &self.plan)
            .field("child_count", &self.children.len())
            .field(
                "rejected_surface_attempt_count",
                &self.rejected_surface_attempts.len(),
            )
            .finish()
    }
}

impl fmt::Debug for Facts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Facts")
            .field("parent_baseline", &self.parent_baseline.is_some())
            .field(
                "complete_search_policy",
                &self.complete_search_policy.is_some(),
            )
            .field("plan_child_budget", &self.plan_child_budget)
            .field("planned_child_count", &self.planned_child_count)
            .field("child_budget", &self.child_budget)
            .field("child_schedule_mode", &self.child_schedule_mode)
            .field("child_plan", &self.child_plan)
            .field("selection_strategy", &self.selection_strategy.is_some())
            .field(
                "child_outcome_count",
                &self.child_outcomes.as_ref().map(Vec::len),
            )
            .field("selection", &self.selection.is_some())
            .field("rejected_attempt_payloads", &self.rejected_attempt_payloads)
            .field("report", &self.report.is_some())
            .field("parent_identity", &self.parent_identity.is_some())
            .finish()
    }
}

// ANCHOR: prototype1_context_collected
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
// ANCHOR_END: prototype1_context_collected

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

    /// Whether R12 facts contain a parent-selected successor coordinate.
    ///
    /// This intentionally follows selection evidence, not branch-evaluation
    /// `keep`/`reject`. A rejected branch can be successor-selected when the
    /// traversal policy admits exploration from rejected children; see
    /// docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md.
    pub(crate) fn has_successor_selection(&self) -> bool {
        self.facts.selection.is_some()
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
