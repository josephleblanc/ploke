use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use ploke_records::agent_turn::{AgentTurnSummaryRecord, AgentTurnTraceRecord};
use ploke_records::branch::{BranchLogBody, BranchLogRecord, Prototype1BranchRegistry};
use ploke_records::channel::{Envelope, ToChild, ToParent};
use ploke_records::child_plan::ChildPlanRecord;
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::history::{EntryPayloadRecord, SealedBlockRecord};
use ploke_records::identity::ParentIdentityRecord;
use ploke_records::invocation::{
    InvocationRecord, Role, SuccessorCompletionRecord, SuccessorReadyRecord,
};
use ploke_records::journal::JournalEntry;
use ploke_records::protocol::{
    Artifact as ProtocolArtifact, TOOL_CALL_INTENT_SEGMENTATION, TOOL_CALL_REVIEW,
    TOOL_CALL_SEGMENT_REVIEW,
};
use ploke_records::run_profile::{RunProfileCommitmentRecord, RunProfileRecord};
use ploke_records::run_record::{RunRecord as EvalRunRecord, read_compressed_record};
use ploke_records::scheduler::{
    NodeRecord, RunnerRequestRecord, RunnerResultRecord, SchedulerStateRecord,
};
use serde::Deserialize;

use crate::{RunForest, assemble_run_forest};

use super::{
    AgentTurnArtifactEvidence, AgentTurnEvidence, AgentTurnEvidenceSummary, BranchRegistryEvidence,
    BranchRunRecordRef, ChannelEvidence, ChildPlanEvidence, ChildPlanSummary, ComparedRunArm,
    EvaluationArtifactSummary, EvaluationEvidence, HistoryEvidence, JsonlEvidence, JsonlRecord,
    PassiveEvidence, ProtocolArtifactSummary, ProtocolArtifactsEvidence, RunAttemptEvidence,
    RunAttemptSummary, RunForestInput, RunProfileEvidence, RunRecordEvidence, RunRecordSet,
    RunRecordStats, RunRecordSummary, RunRootSummary, TransitionJournal,
};

/// Read-only filesystem loader for one Prototype 1 run root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsRunStore {
    run_root: PathBuf,
    parent_identity_path: Option<PathBuf>,
    protocol_artifacts_dirs: Vec<PathBuf>,
}

impl FsRunStore {
    pub fn new(run_root: impl Into<PathBuf>) -> Self {
        Self {
            run_root: run_root.into(),
            parent_identity_path: None,
            protocol_artifacts_dirs: Vec::new(),
        }
    }

    pub fn with_parent_identity_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.parent_identity_path = Some(path.into());
        self
    }

    pub fn with_parent_root(mut self, parent_root: impl Into<PathBuf>) -> Self {
        self.parent_identity_path = Some(
            parent_root
                .into()
                .join(".ploke")
                .join("prototype1")
                .join("parent_identity.json"),
        );
        self
    }

    pub fn with_protocol_artifacts_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.protocol_artifacts_dirs.push(path.into());
        self
    }

    pub fn with_protocol_artifacts_dirs<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.protocol_artifacts_dirs
            .extend(paths.into_iter().map(Into::into));
        self
    }

    pub fn load(&self) -> Result<RunForestInput, FsRunStoreError> {
        let scheduler =
            self.read_json::<SchedulerStateRecord>(&self.run_root.join("scheduler.json"))?;
        let mut node_records = Vec::new();
        let mut successor_ready = Vec::new();
        let mut successor_completion = Vec::new();

        for node_dir in sorted_child_dirs(&self.run_root.join("nodes"))? {
            let node_path = node_dir.join("node.json");
            if node_path.is_file() {
                node_records.push(self.read_json::<NodeRecord>(&node_path)?);
            }

            successor_ready.extend(
                self.read_json_dir::<SuccessorReadyRecord>(&node_dir.join("successor-ready"))?,
            );
            successor_completion.extend(self.read_json_dir::<SuccessorCompletionRecord>(
                &node_dir.join("successor-completion"),
            )?);
        }

        let parent_identity = match &self.parent_identity_path {
            Some(path) if path.is_file() => Some(self.read_json::<ParentIdentityRecord>(path)?),
            _ => None,
        };

        Ok(RunForestInput {
            scheduler,
            node_records,
            parent_identity,
            successor_ready,
            successor_completion,
            passive_evidence: self.load_passive_evidence()?,
        })
    }

    pub fn load_record_set(&self) -> Result<RunRecordSet, FsRunStoreError> {
        Ok(RunRecordSet {
            forest_input: self.load()?,
            history_blocks: self.load_history_blocks()?,
            transition_journal: self.load_transition_journal()?,
        })
    }

    pub fn load_run_root_summary(&self) -> Result<RunRootSummary, FsRunStoreError> {
        let scheduler =
            self.read_json::<SchedulerStateRecord>(&self.run_root.join("scheduler.json"))?;
        let mut nodes = BTreeMap::new();
        for node in scheduler.nodes {
            nodes.insert(node.node_id.to_string(), node);
        }

        for node_dir in sorted_child_dirs(&self.run_root.join("nodes"))? {
            let node_path = node_dir.join("node.json");
            if node_path.is_file() {
                let node = self.read_json::<NodeRecord>(&node_path)?;
                nodes.insert(node.node_id.to_string(), node);
            }
        }

        let mut artifact_keys = BTreeSet::new();
        for node in nodes.values() {
            if let Some(artifact_id) = node.base_artifact_id.as_ref() {
                artifact_keys.insert(passive_artifact_entity_key(artifact_id.as_str()));
            }
            if let Some(artifact_id) = node.derived_artifact_id.as_ref() {
                artifact_keys.insert(passive_artifact_entity_key(artifact_id.as_str()));
            }
        }

        let mut candidate_count = 0;
        let history_blocks = self.load_history_blocks()?;
        for block in &history_blocks {
            let header = &block.state.header;
            artifact_keys.insert(
                header
                    .common
                    .opened_from_artifact
                    .graph_entity_key()
                    .to_owned(),
            );
            artifact_keys.insert(header.active_artifact.graph_entity_key().to_owned());
            artifact_keys.insert(
                header
                    .selected_successor
                    .artifact
                    .graph_entity_key()
                    .to_owned(),
            );

            for entry in &block.entries {
                if let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload {
                    candidate_count += selection.considered.len();
                    for payload in &selection.considered {
                        if let Some(artifact) = payload.artifact.as_ref() {
                            if let Some(surface) = artifact.surface.as_ref() {
                                artifact_keys.insert(passive_artifact_entity_key(
                                    surface.base.artifact_id.as_str(),
                                ));
                                artifact_keys.insert(passive_artifact_entity_key(
                                    surface.after.artifact_id.as_str(),
                                ));
                            }
                            if let Some(derived) =
                                artifact.resolved.branch.derived_artifact_id.as_ref()
                            {
                                artifact_keys.insert(passive_artifact_entity_key(derived.as_str()));
                            }
                        }
                    }
                }
            }
        }

        Ok(RunRootSummary {
            scheduler_node_count: nodes.len(),
            artifact_count: artifact_keys.len(),
            history_block_count: history_blocks.len(),
            candidate_count,
        })
    }

    pub fn load_forest(&self) -> Result<RunForest, FsRunStoreError> {
        Ok(assemble_run_forest(self.load()?))
    }

    pub fn load_history_blocks(&self) -> Result<Vec<SealedBlockRecord>, FsRunStoreError> {
        let blocks_dir = self.run_root.join("history").join("blocks");
        if !blocks_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut blocks = Vec::new();
        for path in sorted_files_with_prefix(&blocks_dir, "segment-", "jsonl")? {
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                let record =
                    serde_json::from_str::<SealedBlockRecord>(&line).map_err(|source| {
                        FsRunStoreError::Json {
                            path: path.clone(),
                            source,
                        }
                    })?;
                blocks.push(record);
            }
        }

        Ok(blocks)
    }

    pub fn load_transition_journal(&self) -> Result<TransitionJournal, FsRunStoreError> {
        let path = self.run_root.join("transition-journal.jsonl");
        if !path.is_file() {
            return Ok(TransitionJournal::default());
        }

        let entries = self.read_jsonl_records::<JournalEntry>(&path)?;
        Ok(TransitionJournal { entries })
    }

    fn load_passive_evidence(&self) -> Result<PassiveEvidence, FsRunStoreError> {
        let evaluations = self.load_evaluation_evidence()?;
        let run_records = self.load_run_record_evidence(evaluations.as_ref())?;
        let protocol_artifacts = self.load_protocol_artifacts_evidence(evaluations.as_ref())?;
        Ok(PassiveEvidence {
            branch_registry: self.load_branch_registry_evidence()?,
            transition_journal: self.load_transition_journal_evidence()?,
            history: self.load_history_evidence()?,
            channel_envelopes: self.load_channel_evidence()?,
            child_plans: self.load_child_plan_evidence()?,
            evaluations,
            protocol_artifacts,
            run_records,
            run_profile: self.load_run_profile_evidence()?,
            run_attempts: self.load_run_attempt_evidence()?,
            agent_turns: self.load_agent_turn_evidence()?,
            attempt_runner_results: self.load_attempt_runner_results()?,
        })
    }

    fn load_branch_registry_evidence(
        &self,
    ) -> Result<Option<BranchRegistryEvidence>, FsRunStoreError> {
        let path = self.run_root.join("branches.json");
        if !path.is_file() {
            return Ok(None);
        }

        if let Some(evidence) = self.load_branch_log_evidence(&path)? {
            return Ok(Some(evidence));
        }

        let registry = self.read_json::<Prototype1BranchRegistry>(&path)?;
        let source_node_count = registry.source_nodes.len();
        let branch_count = registry
            .source_nodes
            .iter()
            .map(|source| source.branches.len())
            .sum();
        let active_target_count = registry.active_targets.len();
        Ok(Some(BranchRegistryEvidence {
            source_node_count,
            branch_count,
            active_target_count,
            record_count: 1,
            registry_snapshot_count: 1,
            parent_comparison_count: 0,
            latest_campaign_id: Some(registry.campaign_id),
            latest_recorded_at: None,
        }))
    }

    fn load_branch_log_evidence(
        &self,
        path: &Path,
    ) -> Result<Option<BranchRegistryEvidence>, FsRunStoreError> {
        let records = match self.read_jsonl_records::<BranchLogRecord>(path) {
            Ok(records) => records,
            Err(FsRunStoreError::JsonLine { line_number: 1, .. }) => return Ok(None),
            Err(source) => return Err(source),
        };

        if records.is_empty() {
            return Ok(Some(BranchRegistryEvidence::default()));
        }

        let mut evidence = BranchRegistryEvidence {
            record_count: records.len(),
            ..BranchRegistryEvidence::default()
        };
        let mut latest_registry = None;
        for record in records {
            evidence.latest_recorded_at = Some(record.record.recorded_at.clone());
            match record.record.body {
                BranchLogBody::RegistrySnapshot(registry) => {
                    evidence.registry_snapshot_count += 1;
                    evidence.latest_campaign_id = Some(registry.campaign_id.clone());
                    latest_registry = Some(registry);
                }
                BranchLogBody::ParentComparison(comparison) => {
                    evidence.parent_comparison_count += 1;
                    evidence.latest_campaign_id = Some(comparison.campaign_id);
                }
            }
        }

        if let Some(registry) = latest_registry {
            evidence.source_node_count = registry.source_nodes.len();
            evidence.branch_count = registry
                .source_nodes
                .iter()
                .map(|source| source.branches.len())
                .sum();
            evidence.active_target_count = registry.active_targets.len();
        }

        Ok(Some(evidence))
    }

    fn load_transition_journal_evidence(&self) -> Result<Option<JsonlEvidence>, FsRunStoreError> {
        let path = self.run_root.join("transition-journal.jsonl");
        if !path.is_file() {
            return Ok(None);
        }

        self.count_jsonl_records::<JournalEntry>(&path).map(Some)
    }

    fn load_history_evidence(&self) -> Result<Option<HistoryEvidence>, FsRunStoreError> {
        let blocks_dir = self.run_root.join("history").join("blocks");
        if !blocks_dir.is_dir() {
            return Ok(None);
        }

        let mut evidence = HistoryEvidence::default();
        for path in sorted_files_with_prefix(&blocks_dir, "segment-", "jsonl")? {
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                evidence.line_count += 1;
                match serde_json::from_str::<SealedBlockRecord>(&line) {
                    Ok(record) => {
                        evidence.sealed_block_count += 1;
                        evidence.admitted_entry_count += record.entries.len();
                    }
                    Err(source) if source.is_data() => {
                        evidence.record_parse_error_count += 1;
                    }
                    Err(_) => evidence.json_parse_error_count += 1,
                }
            }
        }

        Ok(Some(evidence))
    }

    fn load_channel_evidence(&self) -> Result<Option<ChannelEvidence>, FsRunStoreError> {
        let paths = sorted_nested_channel_jsonl_files(&self.run_root)?;
        if paths.is_empty() {
            return Ok(None);
        }

        let mut evidence = ChannelEvidence::default();
        for path in paths {
            evidence.file_count += 1;
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                evidence.line_count += 1;
                if serde_json::from_str::<Envelope<ToParent>>(&line).is_ok()
                    || serde_json::from_str::<Envelope<ToChild>>(&line).is_ok()
                {
                    evidence.parsed_count += 1;
                } else {
                    evidence.parse_error_count += 1;
                }
            }
        }

        Ok(Some(evidence))
    }

    fn load_child_plan_evidence(&self) -> Result<Option<ChildPlanEvidence>, FsRunStoreError> {
        let dir = self.run_root.join("messages").join("child-plan");
        if !dir.is_dir() {
            return Ok(None);
        }

        let mut summary = ChildPlanSummary::default();
        let mut index = BTreeMap::new();
        for path in sorted_json_files(&dir)? {
            summary.file_count += 1;
            let plan = self.read_json::<ChildPlanRecord>(&path)?;
            summary.parsed_count += 1;
            summary.child_count += plan.children.len();
            summary.children_with_surface_count += plan
                .children
                .iter()
                .filter(|child| child.surface.is_some())
                .count();
            summary.rejected_surface_attempt_count += plan.rejected_surface_attempts.len();
            index.insert(plan.parent_node_id.as_str().to_owned(), plan);
        }

        Ok(Some(ChildPlanEvidence { summary, index }))
    }

    pub fn load_evaluation_evidence(&self) -> Result<Option<EvaluationEvidence>, FsRunStoreError> {
        let dir = self.run_root.join("evaluations");
        if !dir.is_dir() {
            return Ok(None);
        }

        let mut index = BTreeMap::new();
        let mut summary = EvaluationArtifactSummary::default();
        for path in sorted_json_files(&dir)? {
            summary.file_count += 1;
            let artifact = self.read_json::<EvaluationArtifact>(&path)?;
            summary.parsed_count += 1;
            if artifact.overall_disposition == ploke_records::branch::Disposition::Keep {
                summary.keep_count += 1;
            } else {
                summary.reject_count += 1;
            }
            index.insert(artifact.branch_id.clone(), artifact);
        }

        Ok(Some(EvaluationEvidence { summary, index }))
    }

    fn load_protocol_artifacts_evidence(
        &self,
        evaluations: Option<&EvaluationEvidence>,
    ) -> Result<Option<ProtocolArtifactsEvidence>, FsRunStoreError> {
        let dirs = self.protocol_artifact_dirs(evaluations);
        let mut summary = ProtocolArtifactSummary::default();
        let mut index = BTreeMap::new();
        let mut loaded_dir = false;

        for dir in dirs {
            if !dir.is_dir() {
                continue;
            }
            loaded_dir = true;
            for path in sorted_json_files(&dir)? {
                summary.file_count += 1;
                let artifact = self.read_json::<ProtocolArtifact>(&path)?;
                summary.parsed_count += 1;
                match artifact.procedure_name.as_str() {
                    TOOL_CALL_INTENT_SEGMENTATION => summary.intent_segmentation_count += 1,
                    TOOL_CALL_REVIEW => summary.review_count += 1,
                    TOOL_CALL_SEGMENT_REVIEW => summary.segment_review_count += 1,
                    _ => {}
                }
                if artifact.body().is_some() {
                    summary.typed_payload_count += 1;
                }
                index.insert(protocol_artifact_key(&path), artifact);
            }
        }

        if !loaded_dir {
            return Ok(None);
        }

        Ok(Some(ProtocolArtifactsEvidence { summary, index }))
    }

    fn protocol_artifact_dirs(&self, evaluations: Option<&EvaluationEvidence>) -> Vec<PathBuf> {
        let mut dirs = BTreeSet::new();
        dirs.insert(self.run_root.join("protocol-artifacts"));
        dirs.extend(self.protocol_artifacts_dirs.iter().cloned());

        if let Some(evaluations) = evaluations {
            for evaluation in evaluations.index.values() {
                for compared in &evaluation.compared_instances {
                    for record_path in [
                        compared.baseline_record_path.as_ref(),
                        compared.treatment_record_path.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let Some(run_dir) = record_path.parent() {
                            dirs.insert(run_dir.join("protocol-artifacts"));
                        }
                    }
                }
            }
        }

        dirs.into_iter().collect()
    }

    fn load_run_record_evidence(
        &self,
        evaluations: Option<&EvaluationEvidence>,
    ) -> Result<Option<RunRecordEvidence>, FsRunStoreError> {
        let Some(evaluations) = evaluations else {
            return Ok(None);
        };

        let mut summary = RunRecordSummary::default();
        let mut index: BTreeMap<String, EvalRunRecord> = BTreeMap::new();
        let mut stats: BTreeMap<String, RunRecordStats> = BTreeMap::new();
        let mut refs_by_branch: BTreeMap<String, Vec<BranchRunRecordRef>> = BTreeMap::new();

        for evaluation in evaluations.index.values() {
            for compared in &evaluation.compared_instances {
                if let Some(record_path) = compared.baseline_record_path.as_ref() {
                    self.load_compared_run_record(
                        &mut summary,
                        &mut index,
                        &mut stats,
                        &mut refs_by_branch,
                        &evaluation.branch_id,
                        &compared.instance_id,
                        ComparedRunArm::Baseline,
                        record_path,
                    )?;
                }
                if let Some(record_path) = compared.treatment_record_path.as_ref() {
                    self.load_compared_run_record(
                        &mut summary,
                        &mut index,
                        &mut stats,
                        &mut refs_by_branch,
                        &evaluation.branch_id,
                        &compared.instance_id,
                        ComparedRunArm::Treatment,
                        record_path,
                    )?;
                }
            }
        }

        if index.is_empty() {
            return Ok(None);
        }

        Ok(Some(RunRecordEvidence {
            summary,
            index,
            stats,
            refs_by_branch,
        }))
    }

    fn load_compared_run_record(
        &self,
        summary: &mut RunRecordSummary,
        index: &mut BTreeMap<String, EvalRunRecord>,
        stats: &mut BTreeMap<String, RunRecordStats>,
        refs_by_branch: &mut BTreeMap<String, Vec<BranchRunRecordRef>>,
        branch_id: &str,
        instance_id: &str,
        arm: ComparedRunArm,
        record_path: &Path,
    ) -> Result<(), FsRunStoreError> {
        if !record_path.is_file() {
            return Ok(());
        }

        let record_key = run_record_key(record_path);
        if !index.contains_key(&record_key) {
            summary.file_count += 1;
            let record =
                read_compressed_record(record_path).map_err(|source| FsRunStoreError::Io {
                    path: record_path.to_path_buf(),
                    source,
                })?;
            summary.parsed_count += 1;
            if record.phases.setup.is_some() {
                summary.records_with_setup_count += 1;
            }
            if record.phases.packaging.is_some() {
                summary.records_with_packaging_count += 1;
            }
            let record_stats = RunRecordStats::from_record(&record);
            summary.total_turn_count += record_stats.turn_count;
            summary.total_tool_call_count += record_stats.tool_call_count;
            summary.failed_tool_call_count += record_stats.failed_tool_call_count;
            stats.insert(record_key.clone(), record_stats);
            index.insert(record_key.clone(), record);
        }

        summary.branch_ref_count += 1;
        match arm {
            ComparedRunArm::Baseline => summary.baseline_ref_count += 1,
            ComparedRunArm::Treatment => summary.treatment_ref_count += 1,
        }
        refs_by_branch
            .entry(branch_id.to_owned())
            .or_default()
            .push(BranchRunRecordRef {
                branch_id: branch_id.to_owned(),
                instance_id: instance_id.to_owned(),
                arm,
                record_key,
                record_path: record_path.to_path_buf(),
            });

        Ok(())
    }

    fn load_run_profile_evidence(&self) -> Result<Option<RunProfileEvidence>, FsRunStoreError> {
        let profile_path = self.run_root.join("run-profile.toml");
        let commitment_path = self.run_root.join("run-profile.commitment.json");
        if !profile_path.is_file() && !commitment_path.is_file() {
            return Ok(None);
        }

        let profile = if profile_path.is_file() {
            Some(self.read_toml::<RunProfileRecord>(&profile_path)?)
        } else {
            None
        };
        let commitment = if commitment_path.is_file() {
            Some(self.read_json::<RunProfileCommitmentRecord>(&commitment_path)?)
        } else {
            None
        };

        Ok(Some(RunProfileEvidence {
            profile,
            commitment,
        }))
    }

    fn load_run_attempt_evidence(&self) -> Result<Option<RunAttemptEvidence>, FsRunStoreError> {
        let nodes_root = self.run_root.join("nodes");
        if !nodes_root.is_dir() {
            return Ok(None);
        }

        let mut summary = RunAttemptSummary::default();
        let mut runner_requests = BTreeMap::new();
        let mut runner_results = BTreeMap::new();
        let mut invocations = BTreeMap::new();

        for node_dir in sorted_child_dirs(&nodes_root)? {
            let request_path = node_dir.join("runner-request.json");
            if request_path.is_file() {
                summary.runner_request_file_count += 1;
                let request = self.read_json::<RunnerRequestRecord>(&request_path)?;
                summary.runner_request_parsed_count += 1;
                runner_requests.insert(run_relative_key(&self.run_root, &request_path), request);
            }

            let result_path = node_dir.join("runner-result.json");
            if result_path.is_file() {
                summary.runner_result_file_count += 1;
                let result = self.read_json::<RunnerResultRecord>(&result_path)?;
                summary.runner_result_parsed_count += 1;
                runner_results.insert(run_relative_key(&self.run_root, &result_path), result);
            }

            for invocation_path in sorted_json_files(&node_dir.join("invocations"))? {
                summary.invocation_file_count += 1;
                let invocation = self.read_json::<InvocationRecord>(&invocation_path)?;
                summary.invocation_parsed_count += 1;
                match invocation.role {
                    Role::Child => summary.child_invocation_count += 1,
                    Role::Successor => summary.successor_invocation_count += 1,
                }
                invocations.insert(
                    run_relative_key(&self.run_root, &invocation_path),
                    invocation,
                );
            }
        }

        if summary.runner_request_file_count == 0
            && summary.runner_result_file_count == 0
            && summary.invocation_file_count == 0
        {
            return Ok(None);
        }

        Ok(Some(RunAttemptEvidence {
            summary,
            runner_requests,
            runner_results,
            invocations,
        }))
    }

    fn load_agent_turn_evidence(&self) -> Result<Option<AgentTurnEvidence>, FsRunStoreError> {
        let trace_paths = expected_agent_turn_files(&self.run_root, "agent-turn-trace.json");
        let summary_paths = expected_agent_turn_files(&self.run_root, "agent-turn-summary.json");
        if trace_paths.is_empty() && summary_paths.is_empty() {
            return Ok(None);
        }

        let mut summary = AgentTurnEvidenceSummary {
            trace_file_count: trace_paths.len(),
            summary_file_count: summary_paths.len(),
            ..AgentTurnEvidenceSummary::default()
        };
        let mut traces = BTreeMap::new();
        let mut summaries = BTreeMap::new();

        for path in trace_paths {
            let record = self.read_json::<AgentTurnTraceRecord>(&path)?;
            summary.trace_parsed_count += 1;
            let artifact = AgentTurnArtifactEvidence::from_record(&record.0);
            summary.observe_artifact(&artifact);
            traces.insert(run_relative_key(&self.run_root, &path), artifact);
        }

        for path in summary_paths {
            let record = self.read_json::<AgentTurnSummaryRecord>(&path)?;
            summary.summary_parsed_count += 1;
            let artifact = AgentTurnArtifactEvidence::from_record(&record.0);
            summary.observe_artifact(&artifact);
            summaries.insert(run_relative_key(&self.run_root, &path), artifact);
        }

        Ok(Some(AgentTurnEvidence {
            summary,
            traces,
            summaries,
        }))
    }

    fn load_attempt_runner_results(
        &self,
    ) -> Result<BTreeMap<String, RunnerResultRecord>, FsRunStoreError> {
        let nodes_root = self.run_root.join("nodes");
        if !nodes_root.is_dir() {
            return Ok(BTreeMap::new());
        }

        let mut attempt_runner_results = BTreeMap::new();
        for node_dir in sorted_child_dirs(&nodes_root)? {
            for result_path in sorted_json_files(&node_dir.join("results"))? {
                let result = self.read_json::<RunnerResultRecord>(&result_path)?;
                attempt_runner_results
                    .insert(run_relative_key(&self.run_root, &result_path), result);
            }
        }

        Ok(attempt_runner_results)
    }

    fn read_json<T>(&self, path: &Path) -> Result<T, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let bytes = fs::read(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| FsRunStoreError::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    fn read_toml<T>(&self, path: &Path) -> Result<T, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let source_text = fs::read_to_string(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&source_text).map_err(|source| FsRunStoreError::Toml {
            path: path.to_path_buf(),
            source,
        })
    }

    fn read_json_dir<T>(&self, dir: &Path) -> Result<Vec<T>, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        for path in sorted_json_files(dir)? {
            records.push(self.read_json::<T>(&path)?);
        }
        Ok(records)
    }

    fn count_jsonl_records<T>(&self, path: &Path) -> Result<JsonlEvidence, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file = fs::File::open(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut evidence = JsonlEvidence::default();

        for line in io::BufReader::new(file).lines() {
            let line = line.map_err(|source| FsRunStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }

            evidence.line_count += 1;
            match serde_json::from_str::<T>(&line) {
                Ok(_) => evidence.parsed_count += 1,
                Err(_) => evidence.parse_error_count += 1,
            }
        }

        Ok(evidence)
    }

    fn read_jsonl_records<T>(&self, path: &Path) -> Result<Vec<JsonlRecord<T>>, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file = fs::File::open(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut records = Vec::new();

        for (line_number, line) in io::BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source| FsRunStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }

            let record =
                serde_json::from_str::<T>(&line).map_err(|source| FsRunStoreError::JsonLine {
                    path: path.to_path_buf(),
                    line_number: line_number + 1,
                    source,
                })?;
            records.push(JsonlRecord {
                line_number: line_number + 1,
                record,
            });
        }

        Ok(records)
    }
}

#[derive(Debug)]
pub enum FsRunStoreError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    JsonLine {
        path: PathBuf,
        line_number: usize,
        source: serde_json::Error,
    },
}

impl fmt::Display for FsRunStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::Toml { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::JsonLine {
                path,
                line_number,
                source,
            } => {
                write!(
                    formatter,
                    "failed to parse {} line {}: {source}",
                    path.display(),
                    line_number
                )
            }
        }
    }
}

impl Error for FsRunStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source),
            Self::JsonLine { source, .. } => Some(source),
        }
    }
}

fn sorted_child_dirs(parent: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    if !parent.is_dir() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();
    for entry in fs::read_dir(parent).map_err(|source| FsRunStoreError::Io {
        path: parent.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| FsRunStoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn sorted_json_files(parent: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    sorted_files_with_extension(parent, "json")
}

fn sorted_files_with_extension(
    parent: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, FsRunStoreError> {
    if !parent.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(parent).map_err(|source| FsRunStoreError::Io {
        path: parent.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| FsRunStoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some(extension) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn sorted_files_with_prefix(
    parent: &Path,
    prefix: &str,
    extension: &str,
) -> Result<Vec<PathBuf>, FsRunStoreError> {
    let mut paths = sorted_files_with_extension(parent, extension)?;
    paths.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix))
    });
    Ok(paths)
}

fn sorted_nested_channel_jsonl_files(run_root: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    let nodes_root = run_root.join("nodes");
    if !nodes_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for node_dir in sorted_child_dirs(&nodes_root)? {
        let channels_dir = node_dir.join("channels");
        if !channels_dir.is_dir() {
            continue;
        }

        for stream_dir in sorted_child_dirs(&channels_dir)? {
            paths.extend(sorted_files_with_extension(&stream_dir, "jsonl")?);
        }
    }
    paths.sort();
    Ok(paths)
}

fn expected_agent_turn_files(run_root: &Path, file_name: &str) -> Vec<PathBuf> {
    let path = run_root.join(file_name);
    if path.is_file() {
        vec![path]
    } else {
        Vec::new()
    }
}

fn protocol_artifact_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn run_record_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn passive_artifact_entity_key(artifact_id: &str) -> String {
    artifact_id
        .strip_prefix("artifact:")
        .unwrap_or(artifact_id)
        .to_owned()
}

fn run_relative_key(run_root: &Path, path: &Path) -> String {
    path.strip_prefix(run_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_record_set_loads_agent_turn_trace_and_summary_evidence() {
        let root = temp_run_root("agent-turn");
        fs::create_dir_all(root.join("nodes").join("child").join("output"))
            .expect("create nested output dir");
        fs::write(
            root.join("scheduler.json"),
            minimal_scheduler_json().to_string(),
        )
        .expect("write scheduler");
        fs::write(
            root.join("agent-turn-trace.json"),
            agent_turn_artifact_json("trace-task", true).to_string(),
        )
        .expect("write trace");
        fs::write(
            root.join("agent-turn-summary.json"),
            agent_turn_artifact_json("summary-task", false).to_string(),
        )
        .expect("write summary");
        fs::write(
            root.join("nodes")
                .join("child")
                .join("output")
                .join("agent-turn-trace.json"),
            agent_turn_artifact_json("nested-trace-task", false).to_string(),
        )
        .expect("write nested trace lookalike");
        fs::write(
            root.join("nodes")
                .join("child")
                .join("output")
                .join("agent-turn-summary.json"),
            agent_turn_artifact_json("nested-summary-task", true).to_string(),
        )
        .expect("write nested summary lookalike");

        let records = FsRunStore::new(&root)
            .load_record_set()
            .expect("load record set");
        let agent_turns = records
            .forest_input
            .passive_evidence
            .agent_turns
            .as_ref()
            .expect("agent-turn evidence");

        assert_eq!(agent_turns.summary.trace_file_count, 1);
        assert_eq!(agent_turns.summary.trace_parsed_count, 1);
        assert_eq!(agent_turns.summary.summary_file_count, 1);
        assert_eq!(agent_turns.summary.summary_parsed_count, 1);
        assert_eq!(agent_turns.summary.artifact_with_terminal_record_count, 2);
        assert_eq!(agent_turns.summary.artifact_with_final_message_count, 2);
        assert_eq!(agent_turns.summary.artifact_with_applied_patch_count, 1);
        assert_eq!(agent_turns.summary.tool_request_event_count, 2);
        assert_eq!(agent_turns.summary.tool_completed_event_count, 2);
        assert_eq!(agent_turns.summary.tool_failed_event_count, 2);

        let trace = agent_turns
            .traces
            .get("agent-turn-trace.json")
            .expect("trace artifact evidence");
        assert_eq!(trace.task_id, "trace-task");
        assert_eq!(trace.selected_model, "openai/gpt-5");
        assert!(trace.patch_applied);
        assert_eq!(trace.expected_file_change_count, 1);
        assert_eq!(trace.terminal_outcome.as_deref(), Some("completed"));

        let summary = agent_turns
            .summaries
            .get("agent-turn-summary.json")
            .expect("summary artifact evidence");
        assert_eq!(summary.task_id, "summary-task");
        assert!(!summary.patch_applied);
        assert_eq!(summary.llm_prompt_message_count, 1);
        assert!(summary.has_llm_response);
        assert!(
            !agent_turns
                .traces
                .contains_key("nodes/child/output/agent-turn-trace.json")
        );
        assert!(
            !agent_turns
                .summaries
                .contains_key("nodes/child/output/agent-turn-summary.json")
        );

        fs::remove_dir_all(root).expect("remove temp run");
    }

    #[test]
    fn load_run_root_summary_skips_passive_evidence() {
        let root = temp_run_root("summary");
        fs::create_dir_all(root.join("evaluations")).expect("create evaluations dir");
        fs::write(
            root.join("scheduler.json"),
            minimal_scheduler_json().to_string(),
        )
        .expect("write scheduler");
        fs::write(root.join("evaluations").join("bad.json"), "{").expect("write bad evaluation");

        let summary = FsRunStore::new(&root)
            .load_run_root_summary()
            .expect("load lightweight summary");

        assert_eq!(summary.scheduler_node_count, 0);
        assert_eq!(summary.artifact_count, 0);
        assert_eq!(summary.history_block_count, 0);
        assert_eq!(summary.candidate_count, 0);

        fs::remove_dir_all(root).expect("remove temp run");
    }

    fn temp_run_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ploke-tree-fs-store-{prefix}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn minimal_scheduler_json() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "prototype1-scheduler.v1",
            "campaign_id": "campaign-1",
            "updated_at": "2026-05-12T12:00:00Z",
            "nodes": []
        })
    }

    fn agent_turn_artifact_json(task_id: &str, patch_applied: bool) -> serde_json::Value {
        serde_json::json!({
            "task_id": task_id,
            "selected_model": "openai/gpt-5",
            "issue_prompt": "Fix the bug.",
            "user_message_id": "user-1",
            "events": [
                {"ToolRequested": {
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "call_id": "call-1",
                    "tool": "read_file",
                    "arguments": "{\"file\":\"src/lib.rs\"}"
                }},
                {"ToolCompleted": {
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "call_id": "call-1",
                    "tool": "read_file",
                    "content": "ok",
                    "ui_payload": null,
                    "latency_ms": 42
                }},
                {"ToolFailed": {
                    "request_id": "req-2",
                    "parent_id": "parent-1",
                    "call_id": "call-2",
                    "tool": "apply_code_edit",
                    "error": "no match",
                    "ui_payload": null,
                    "latency_ms": 13
                }},
                {"TurnFinished": {
                    "session_id": "session-1",
                    "request_id": "req-1",
                    "parent_id": "parent-1",
                    "assistant_message_id": "assistant-1",
                    "outcome": "completed",
                    "error_id": null,
                    "summary": "done",
                    "attempts": 1
                }}
            ],
            "prompt_debug": null,
            "terminal_record": {
                "session_id": "session-1",
                "request_id": "req-1",
                "parent_id": "parent-1",
                "assistant_message_id": "assistant-1",
                "outcome": "completed",
                "error_id": null,
                "summary": "done",
                "attempts": 1
            },
            "final_assistant_message": {
                "id": "assistant-1",
                "kind": "Assistant",
                "status": "Completed",
                "tool_call_id": null,
                "content_len": 4,
                "content_preview": "done"
            },
            "patch_artifact": {
                "edit_proposals": [],
                "create_proposals": [],
                "applied": patch_applied,
                "all_proposals_applied": patch_applied,
                "expected_file_changes": [{
                    "path": "src/lib.rs",
                    "existed_before": true,
                    "exists_after": true,
                    "before_sha256": "sha256:before",
                    "after_sha256": "sha256:after",
                    "changed": patch_applied
                }],
                "any_expected_file_changed": patch_applied,
                "all_expected_files_changed": patch_applied
            },
            "llm_prompt": [{
                "role": "user",
                "content": "Fix the bug."
            }],
            "llm_response": "done"
        })
    }
}
