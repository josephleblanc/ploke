//! Eval-owned harness I/O carriers, evidence projection, and durable records.

use std::{
    collections::HashMap,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use ploke_records::{
    agent_turn::{
        AgentTurnArtifactRecord, MessageSnapshotRecord, ModelRouteRecord, ObservedTurnEventRecord,
        PatchArtifactRecord, ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
        TurnFinishedRecord,
    },
    llm_response::RawFullResponseRecord,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::spec::PrepareError;

use super::super::{
    ArtifactDelta,
    harness_request::{EvidenceRoot, request},
    surface, tui,
};
use super::{
    MAX_DEBUG_RELAY_EVENT_CHARS, MAX_DEBUG_RELAY_EVENTS, MAX_EVIDENCE_EVENT_CHARS,
    MAX_PROMPT_MESSAGE_PREVIEW_CHARS, MAX_PROMPT_MESSAGE_PREVIEWS, MAX_RAG_PART_PREVIEWS, state,
};

#[derive(Debug, Clone)]
pub(crate) struct HeadlessRun {
    pub(super) attempts: Vec<HeadlessAttempt>,
    pub(super) events: Vec<Event>,
    pub(super) validations: Vec<CargoValidationObservation>,
    pub(super) debug_relay: DebugRelay,
    pub(super) prompt_diagnostics: Vec<PromptDiagnostic>,
    pub(super) full_response_records: Vec<RawFullResponseRecord>,
    pub(super) next_response_index: usize,
    pub(super) terminal: Option<HeadlessTerminal>,
    pub(super) model_route: Option<ModelRouteRecord>,
}

#[derive(Debug, Clone, Copy)]
struct ToolRequestContext<'a> {
    request_id: &'a str,
    parent_id: &'a str,
    tool: &'a str,
}

impl HeadlessRun {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn new() -> Self {
        Self {
            attempts: Vec::new(),
            events: Vec::new(),
            validations: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            full_response_records: Vec::new(),
            next_response_index: 0,
            terminal: None,
            model_route: None,
        }
    }

    pub(crate) fn attempts(&self) -> &[HeadlessAttempt] {
        &self.attempts
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.events
    }

    pub(crate) fn validations(&self) -> &[CargoValidationObservation] {
        &self.validations
    }

    pub(crate) fn terminal(&self) -> Option<&HeadlessTerminal> {
        self.terminal.as_ref()
    }

    pub(crate) fn setup_unavailable(phase: &'static str, reason: impl Into<String>) -> Self {
        let mut run = Self::new();
        run.terminal = Some(HeadlessTerminal::SetupUnavailable {
            phase,
            reason: reason.into(),
        });
        run
    }

    pub(crate) fn debug_relay(&self) -> &DebugRelay {
        &self.debug_relay
    }

    pub(crate) fn prompt_diagnostics(&self) -> &[PromptDiagnostic] {
        &self.prompt_diagnostics
    }

    pub(crate) fn full_response_records(&self) -> &[RawFullResponseRecord] {
        &self.full_response_records
    }

    pub(crate) fn evidence(&self) -> evidence::Summary {
        evidence::Summary::from(self)
    }

    pub(crate) fn agent_turn_artifact_record(
        &self,
        task_id: &str,
        selected_model: &str,
        issue_prompt: &str,
    ) -> AgentTurnArtifactRecord {
        let mut tool_requests = HashMap::<&str, ToolRequestContext<'_>>::new();
        let mut events = Vec::new();
        let mut terminal_record = None;
        let mut final_assistant_message = None;

        for event in &self.events {
            match event {
                Event::ToolRequest {
                    request_id,
                    parent_id,
                    call_id,
                    tool,
                    arguments,
                } => {
                    let context = ToolRequestContext {
                        request_id,
                        parent_id,
                        tool,
                    };
                    tool_requests.insert(call_id.as_str(), context);
                    events.push(ObservedTurnEventRecord::ToolRequested(tool_request_record(
                        context, call_id, arguments,
                    )));
                }
                Event::Tool { call_id, result } => {
                    if let Some(request) = tool_requests.get(call_id.as_str()) {
                        match result {
                            Tool::Completed { content } => {
                                events.push(ObservedTurnEventRecord::ToolCompleted(
                                    tool_completed_record(*request, call_id, content),
                                ));
                            }
                            Tool::Failed { error } => {
                                events.push(ObservedTurnEventRecord::ToolFailed(
                                    tool_failed_record(*request, call_id, error),
                                ));
                            }
                        }
                    }
                }
                Event::AssistantMessage {
                    id,
                    status,
                    content,
                } => {
                    let record = assistant_message_snapshot_record(id, status, content);
                    final_assistant_message = Some(record.clone());
                    events.push(ObservedTurnEventRecord::MessageUpdated(record));
                }
                Event::Turn {
                    session_id,
                    request_id,
                    parent_id,
                    assistant_message_id,
                    outcome,
                    error_id,
                    attempts,
                    summary,
                } => {
                    let record = turn_finished_record(TurnRecordParts {
                        session_id,
                        request_id,
                        parent_id,
                        assistant_message_id,
                        outcome,
                        error_id: error_id.as_deref(),
                        summary,
                        attempts: *attempts,
                    });
                    terminal_record = Some(record.clone());
                    events.push(ObservedTurnEventRecord::TurnFinished(record));
                }
                Event::Proposal { .. } | Event::Outcome(_) => {}
            }
        }

        AgentTurnArtifactRecord {
            task_id: task_id.to_string(),
            selected_model: selected_model.to_string(),
            model_route: self.model_route.clone(),
            issue_prompt: issue_prompt.to_string(),
            user_message_id: self.observed_user_message_id(),
            events,
            prompt_debug: None,
            terminal_record,
            final_assistant_message,
            patch_artifact: self.patch_artifact_record(),
            llm_prompt: Vec::new(),
            llm_response: None,
        }
    }

    fn observed_user_message_id(&self) -> String {
        observed_user_message_id(&self.events)
            .unwrap_or_default()
            .to_string()
    }

    fn patch_artifact_record(&self) -> PatchArtifactRecord {
        let applied = self.applied_edit().is_some();
        let mut saw_proposal = false;
        let mut all_proposals_applied = true;
        for attempt in self
            .attempts
            .iter()
            .filter(|attempt| attempt.proposal_id.is_some())
        {
            saw_proposal = true;
            all_proposals_applied &=
                matches!(attempt.result, HeadlessAttemptResult::Applied { .. });
        }
        PatchArtifactRecord {
            edit_proposals: Vec::new(),
            create_proposals: Vec::new(),
            applied,
            all_proposals_applied: saw_proposal && all_proposals_applied,
            expected_file_changes: Vec::new(),
            any_expected_file_changed: false,
            all_expected_files_changed: false,
        }
    }

    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn has_observed_activity(
        &self,
    ) -> bool {
        !self.attempts.is_empty() || !self.events.is_empty() || !self.prompt_diagnostics.is_empty()
    }

    pub(crate) fn applied_edit(&self) -> Option<AppliedEdit> {
        let mut proposal_ids = Vec::new();
        let mut changed_paths = Vec::new();
        for attempt in &self.attempts {
            let HeadlessAttemptResult::Applied { paths } = &attempt.result else {
                continue;
            };
            let Some(proposal_id) = attempt.proposal_id else {
                continue;
            };
            proposal_ids.push(proposal_id);
            push_changed_paths(&mut changed_paths, paths.clone());
        }
        proposal_ids.last().copied().map(|proposal_id| AppliedEdit {
            proposal_id,
            proposal_ids,
            changed_paths,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_parts_for_test(
        attempts: Vec<HeadlessAttempt>,
        terminal: Option<HeadlessTerminal>,
    ) -> Self {
        Self {
            attempts,
            events: Vec::new(),
            validations: Vec::new(),
            debug_relay: DebugRelay::new(),
            prompt_diagnostics: Vec::new(),
            full_response_records: Vec::new(),
            next_response_index: 0,
            terminal,
            model_route: None,
        }
    }
}

fn observed_user_message_id(events: &[Event]) -> Option<&str> {
    events.iter().find_map(|event| match event {
        Event::Turn { parent_id, .. } | Event::ToolRequest { parent_id, .. } => {
            Some(parent_id.as_str())
        }
        _ => None,
    })
}

fn tool_request_record(
    context: ToolRequestContext<'_>,
    call_id: &str,
    arguments: &str,
) -> ToolRequestRecord {
    ToolRequestRecord {
        request_id: context.request_id.to_owned(),
        parent_id: context.parent_id.to_owned(),
        call_id: call_id.to_owned(),
        tool: context.tool.to_owned(),
        arguments: arguments.to_owned().into(),
    }
}

fn tool_completed_record(
    context: ToolRequestContext<'_>,
    call_id: &str,
    content: &str,
) -> ToolCompletedRecord {
    ToolCompletedRecord {
        request_id: context.request_id.to_owned(),
        parent_id: context.parent_id.to_owned(),
        call_id: call_id.to_owned(),
        tool: context.tool.to_owned(),
        content: content.to_owned(),
        ui_payload: None,
        latency_ms: 0,
    }
}

fn tool_failed_record(
    context: ToolRequestContext<'_>,
    call_id: &str,
    error: &str,
) -> ToolFailedRecord {
    ToolFailedRecord {
        request_id: context.request_id.to_owned(),
        parent_id: context.parent_id.to_owned(),
        call_id: call_id.to_owned(),
        tool: Some(context.tool.to_owned()),
        error: error.to_owned(),
        ui_payload: None,
        latency_ms: 0,
    }
}

fn assistant_message_snapshot_record(
    id: &str,
    status: &str,
    content: &str,
) -> MessageSnapshotRecord {
    MessageSnapshotRecord {
        id: id.to_owned(),
        kind: "assistant".to_string(),
        status: status.to_owned(),
        tool_call_id: None,
        content_len: content.chars().count(),
        content_preview: truncate_chars(content, MAX_EVIDENCE_EVENT_CHARS),
    }
}

struct TurnRecordParts<'a> {
    session_id: &'a str,
    request_id: &'a str,
    parent_id: &'a str,
    assistant_message_id: &'a str,
    outcome: &'a str,
    error_id: Option<&'a str>,
    summary: &'a str,
    attempts: u32,
}

fn turn_finished_record(parts: TurnRecordParts<'_>) -> TurnFinishedRecord {
    TurnFinishedRecord {
        session_id: parts.session_id.to_owned(),
        request_id: parts.request_id.to_owned(),
        parent_id: parts.parent_id.to_owned(),
        assistant_message_id: parts.assistant_message_id.to_owned(),
        outcome: parts.outcome.to_owned(),
        error_id: parts.error_id.map(str::to_owned),
        summary: parts.summary.to_owned(),
        attempts: parts.attempts,
    }
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn observed_headless_error(
    source: Error,
) -> String {
    format!("headless runtime failed after observed activity: {source}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoValidationObservation {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) call_id: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) command: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) display_command: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) ok: bool,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) status_reason: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) exit_code: Option<i32>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) manifest_path: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) errors: u32,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) warnings: u32,
}

impl CargoValidationObservation {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn failure_feedback(
        &self,
    ) -> Option<String> {
        if self.ok {
            return None;
        }
        Some(format!(
            "Cargo validation failed after applying edits: `{}` exited {:?} with status `{}` (errors: {}, warnings: {}). Repair the failure before claiming success.",
            self.display_command, self.exit_code, self.status_reason, self.errors, self.warnings
        ))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) struct CargoRequestArgs {
    command: Option<String>,
    package: Option<String>,
    all_features: Option<bool>,
    no_default_features: Option<bool>,
    features: Option<Vec<String>>,
    target: Option<String>,
    profile: Option<String>,
    release: Option<bool>,
    lib: Option<bool>,
    tests: Option<bool>,
    bins: Option<bool>,
    examples: Option<bool>,
    benches: Option<bool>,
    test_args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CargoResultProjection {
    ok: bool,
    status_reason: String,
    command: String,
    manifest_path: String,
    exit_code: Option<i32>,
    summary: CargoSummaryProjection,
}

#[derive(Debug, Deserialize)]
struct CargoSummaryProjection {
    errors: u32,
    warnings: u32,
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn observe_cargo_validation(
    run: &mut HeadlessRun,
    call_id: &str,
    arguments: &str,
    content: &str,
) -> Option<CargoValidationObservation> {
    let result = serde_json::from_str::<CargoResultProjection>(content).ok()?;
    let args = serde_json::from_str::<CargoRequestArgs>(arguments).unwrap_or_default();
    let observation = CargoValidationObservation {
        call_id: call_id.to_string(),
        command: result.command.clone(),
        display_command: display_cargo_command(&args, &result.command),
        ok: result.ok,
        status_reason: result.status_reason,
        exit_code: result.exit_code,
        manifest_path: result.manifest_path,
        errors: result.summary.errors,
        warnings: result.summary.warnings,
    };
    run.validations.push(observation.clone());
    Some(observation)
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn display_cargo_command(
    args: &CargoRequestArgs,
    result_command: &str,
) -> String {
    let mut parts = vec![
        "cargo".to_string(),
        args.command
            .clone()
            .unwrap_or_else(|| result_command.to_string()),
    ];
    if let Some(package) = &args.package {
        parts.push("-p".to_string());
        parts.push(package.clone());
    }
    if args.all_features.unwrap_or(false) {
        parts.push("--all-features".to_string());
    }
    if args.no_default_features.unwrap_or(false) {
        parts.push("--no-default-features".to_string());
    }
    if let Some(features) = &args.features
        && !features.is_empty()
    {
        parts.push("--features".to_string());
        parts.push(features.join(","));
    }
    if let Some(target) = &args.target {
        parts.push("--target".to_string());
        parts.push(target.clone());
    }
    if let Some(profile) = &args.profile {
        parts.push("--profile".to_string());
        parts.push(profile.clone());
    }
    if args.release.unwrap_or(false) {
        parts.push("--release".to_string());
    }
    for (enabled, flag) in [
        (args.lib, "--lib"),
        (args.tests, "--tests"),
        (args.bins, "--bins"),
        (args.examples, "--examples"),
        (args.benches, "--benches"),
    ] {
        if enabled.unwrap_or(false) {
            parts.push(flag.to_string());
        }
    }
    if let Some(test_args) = &args.test_args
        && !test_args.is_empty()
    {
        parts.push("--".to_string());
        parts.extend(test_args.iter().cloned());
    }
    parts.join(" ")
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn latest_failed_cargo_validation_feedback(
    run: &HeadlessRun,
) -> Option<String> {
    run.validations()
        .last()
        .and_then(CargoValidationObservation::failure_feedback)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedEdit {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) proposal_id: Uuid,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) proposal_ids: Vec<Uuid>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) changed_paths: Vec<PathBuf>,
}

impl AppliedEdit {
    pub(crate) fn proposal_id(&self) -> Uuid {
        self.proposal_id
    }

    pub(crate) fn proposal_ids(&self) -> &[Uuid] {
        &self.proposal_ids
    }

    pub(crate) fn changed_paths(&self) -> &[PathBuf] {
        &self.changed_paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptDiagnostic {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) parent_id: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) workspace: WorkspaceDiagnostic,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) bm25: Option<Bm25Diagnostic>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) context_mode: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) max_leased_tokens: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) estimated_total_tokens: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) message_count: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) message_previews:
        Vec<MessagePreview>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) included_rag_parts: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) rag_part_previews:
        Vec<RagPartPreview>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) rag_stats:
        Option<ContextStatsDiagnostic>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fallback_notice: Option<String>,
}

impl PromptDiagnostic {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) async fn capture(
        state: &std::sync::Arc<ploke_tui::app_state::AppState>,
        parent_id: Uuid,
        formatted_prompt: &[ploke_tui::llm::RequestMessage],
        context_plan: &ploke_tui::llm::ContextPlan,
    ) -> Self {
        let (root, member_roots, focused_root) = state
            .with_system_read(|sys| {
                (
                    sys.loaded_workspace_root(),
                    sys.loaded_workspace_member_roots(),
                    sys.focused_crate_root(),
                )
            })
            .await;
        let (context_mode, max_leased_tokens) = {
            let cfg = state.config.read().await;
            (
                format!("{:?}", cfg.context_management.mode),
                cfg.context_management.max_leased_tokens,
            )
        };
        let bm25 = match state.rag.as_ref() {
            Some(rag) => match rag.bm25_status().await {
                Ok(status) => Some(Bm25Diagnostic::from(status)),
                Err(err) => Some(Bm25Diagnostic {
                    status: "status_error".to_string(),
                    docs: None,
                    error: Some(err.to_string()),
                }),
            },
            None => None,
        };
        Self {
            parent_id: parent_id.to_string(),
            workspace: WorkspaceDiagnostic {
                loaded: root.is_some(),
                root,
                member_count: member_roots.len(),
                focused_root,
            },
            bm25,
            context_mode,
            max_leased_tokens,
            estimated_total_tokens: context_plan.estimated_total_tokens,
            message_count: formatted_prompt.len(),
            message_previews: formatted_prompt
                .iter()
                .take(MAX_PROMPT_MESSAGE_PREVIEWS)
                .map(MessagePreview::from)
                .collect(),
            included_rag_parts: context_plan.included_rag_parts.len(),
            rag_part_previews: context_plan
                .included_rag_parts
                .iter()
                .take(MAX_RAG_PART_PREVIEWS)
                .map(RagPartPreview::from)
                .collect(),
            rag_stats: context_plan
                .rag_stats
                .as_ref()
                .map(ContextStatsDiagnostic::from),
            fallback_notice: fallback_notice(formatted_prompt),
        }
    }

    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn context_unavailable_reason(
        &self,
    ) -> Option<String> {
        if self.context_mode == "Off" {
            return None;
        }
        self.fallback_notice
            .as_ref()
            .filter(|notice| notice.contains("without code context"))
            .cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceDiagnostic {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) loaded: bool,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) root: Option<PathBuf>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) member_count: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) focused_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bm25Diagnostic {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) status: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) docs: Option<usize>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) error: Option<String>,
}

impl From<ploke_db::bm25_index::bm25_service::Bm25Status> for Bm25Diagnostic {
    fn from(value: ploke_db::bm25_index::bm25_service::Bm25Status) -> Self {
        match value {
            ploke_db::bm25_index::bm25_service::Bm25Status::Uninitialized => Self {
                status: "uninitialized".to_string(),
                docs: None,
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Building => Self {
                status: "building".to_string(),
                docs: None,
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Ready { docs } => Self {
                status: "ready".to_string(),
                docs: Some(docs),
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Empty => Self {
                status: "empty".to_string(),
                docs: Some(0),
                error: None,
            },
            ploke_db::bm25_index::bm25_service::Bm25Status::Error(error) => Self {
                status: "error".to_string(),
                docs: None,
                error: Some(error),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessagePreview {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) role: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) chars: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) preview: String,
}

impl From<&ploke_tui::llm::RequestMessage> for MessagePreview {
    fn from(value: &ploke_tui::llm::RequestMessage) -> Self {
        Self {
            role: format!("{:?}", value.role),
            chars: value.content.chars().count(),
            preview: truncate_chars(&value.content, MAX_PROMPT_MESSAGE_PREVIEW_CHARS),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RagPartPreview {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) file_path: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) kind: String,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) estimated_tokens: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) score: f32,
}

impl Eq for RagPartPreview {}

impl From<&ploke_tui::llm::ContextPlanRagPart> for RagPartPreview {
    fn from(value: &ploke_tui::llm::ContextPlanRagPart) -> Self {
        Self {
            file_path: value.file_path.clone(),
            kind: format!("{:?}", value.kind),
            estimated_tokens: value.estimated_tokens,
            score: value.score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextStatsDiagnostic {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) total_tokens: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) files: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) parts: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) truncated_parts: usize,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) dedup_removed: usize,
}

impl From<&ploke_core::rag_types::ContextStats> for ContextStatsDiagnostic {
    fn from(value: &ploke_core::rag_types::ContextStats) -> Self {
        Self {
            total_tokens: value.total_tokens,
            files: value.files,
            parts: value.parts,
            truncated_parts: value.truncated_parts,
            dedup_removed: value.dedup_removed,
        }
    }
}

fn fallback_notice(messages: &[ploke_tui::llm::RequestMessage]) -> Option<String> {
    messages
        .first()
        .filter(|message| {
            let content = message.content.as_str();
            content.contains("proceeding without code context")
                || content.starts_with("Context mode is Off:")
        })
        .map(|message| message.content.clone())
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn truncate_chars(
    input: &str,
    max_chars: usize,
) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebugRelay {
    retained: Vec<String>,
    dropped: u64,
    truncated: u64,
}

impl DebugRelay {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn new() -> Self {
        Self {
            retained: Vec::new(),
            dropped: 0,
            truncated: 0,
        }
    }

    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn push(
        &mut self,
        message: &str,
    ) {
        let mut chars = message.chars();
        let retained = chars
            .by_ref()
            .take(MAX_DEBUG_RELAY_EVENT_CHARS)
            .collect::<String>();
        if chars.next().is_some() {
            self.truncated += 1;
        }

        if self.retained.len() == MAX_DEBUG_RELAY_EVENTS {
            self.retained.remove(0);
            self.dropped += 1;
        }
        self.retained.push(retained);
    }

    pub(crate) fn retained(&self) -> &[String] {
        &self.retained
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn truncated(&self) -> u64 {
        self.truncated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessAttempt {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) turn: u32,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) proposal_id: Option<Uuid>,
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) result: HeadlessAttemptResult,
}

impl HeadlessAttempt {
    pub(crate) fn turn(&self) -> u32 {
        self.turn
    }

    pub(crate) fn proposal_id(&self) -> Option<Uuid> {
        self.proposal_id
    }

    pub(crate) fn result(&self) -> &HeadlessAttemptResult {
        &self.result
    }

    #[cfg(test)]
    pub(crate) fn applied_for_test(turn: u32, proposal_id: Uuid, paths: Vec<PathBuf>) -> Self {
        Self {
            turn,
            proposal_id: Some(proposal_id),
            result: HeadlessAttemptResult::Applied { paths },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadlessAttemptResult {
    Applied { paths: Vec<PathBuf> },
    Rejected { reason: String },
    PostApprovalIndeterminate { paths: Vec<PathBuf>, error: String },
    NoEdit { summary: String },
    ToolFailed { error: String },
}

#[derive(Debug, Clone)]
pub(crate) struct AttemptOutcome {
    pub(crate) run: HeadlessRun,
    pub(crate) terminal: HeadlessTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadlessTerminal {
    Applied {
        proposal_id: Uuid,
        applied_proposal_ids: Vec<Uuid>,
        request_id: Uuid,
        changed_paths: Vec<PathBuf>,
    },
    Exhausted {
        attempts: u32,
        last: String,
    },
    CompletedWithoutEdit {
        outcome: String,
        summary: String,
    },
    ToolFailed {
        error: String,
    },
    NoEdit,
    ContextUnavailable {
        reason: String,
    },
    ProviderUnavailable {
        reason: String,
    },
    SetupUnavailable {
        phase: &'static str,
        reason: String,
    },
    AppliedValidationFailed {
        applied: AppliedEdit,
        feedback: String,
    },
    AppliedValidationMissing {
        applied: AppliedEdit,
        missing: Vec<String>,
    },
    AppliedTurnAborted {
        applied: AppliedEdit,
        outcome: String,
        summary: String,
    },
    AppliedTimedOut {
        secs: u64,
        applied: AppliedEdit,
    },
    TimedOut {
        secs: u64,
    },
}

impl HeadlessTerminal {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn live_summary(
        &self,
    ) -> String {
        match self {
            Self::Applied {
                proposal_id,
                applied_proposal_ids,
                changed_paths,
                ..
            } => format!(
                "applied proposal_id={} applied_proposals={} changed_paths={}",
                proposal_id,
                applied_proposal_ids.len(),
                join_paths(changed_paths)
            ),
            Self::Exhausted { attempts, last } => format!(
                "exhausted attempts={} last={}",
                attempts,
                truncate_chars(last, 240)
            ),
            Self::CompletedWithoutEdit { outcome, summary } => format!(
                "completed_without_edit outcome={} summary={}",
                outcome,
                truncate_chars(summary, 240)
            ),
            Self::ToolFailed { error } => {
                format!("tool_failed error={}", truncate_chars(error, 240))
            }
            Self::NoEdit => "no_edit".to_string(),
            Self::ContextUnavailable { reason } => {
                format!("context_unavailable reason={}", truncate_chars(reason, 240))
            }
            Self::ProviderUnavailable { reason } => {
                format!(
                    "provider_unavailable reason={}",
                    truncate_chars(reason, 240)
                )
            }
            Self::SetupUnavailable { phase, reason } => {
                format!(
                    "setup_unavailable phase={} reason={}",
                    phase,
                    truncate_chars(reason, 240)
                )
            }
            Self::AppliedValidationFailed { applied, feedback } => format!(
                "applied_validation_failed proposal_id={} changed_paths={} feedback={}",
                applied.proposal_id(),
                join_paths(applied.changed_paths()),
                truncate_chars(feedback, 240)
            ),
            Self::AppliedValidationMissing { applied, missing } => format!(
                "applied_validation_missing proposal_id={} changed_paths={} missing={}",
                applied.proposal_id(),
                join_paths(applied.changed_paths()),
                missing.join(", ")
            ),
            Self::AppliedTurnAborted {
                applied,
                outcome,
                summary,
            } => format!(
                "applied_turn_aborted proposal_id={} changed_paths={} outcome={} summary={}",
                applied.proposal_id(),
                join_paths(applied.changed_paths()),
                outcome,
                truncate_chars(summary, 240)
            ),
            Self::AppliedTimedOut { secs, applied } => format!(
                "applied_timed_out secs={} proposal_id={} changed_paths={}",
                secs,
                applied.proposal_id(),
                join_paths(applied.changed_paths())
            ),
            Self::TimedOut { secs } => format!("timed_out secs={secs}"),
        }
    }
}

pub(crate) mod evidence {
    use std::path::PathBuf;

    use ploke_records::agent_turn::ModelRouteRecord;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use super::{
        CargoValidationObservation, DebugRelay, HeadlessAttemptResult, HeadlessRun,
        HeadlessTerminal, MAX_EVIDENCE_EVENT_CHARS, truncate_chars,
    };

    /// Compact executor observations. Backend admission must still validate the
    /// workspace diff before any loop state advances.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Summary {
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
        #[serde(default)]
        pub(crate) events: Vec<Event>,
        #[serde(default)]
        pub(crate) validations: Vec<CargoValidation>,
        #[serde(default)]
        pub(crate) debug_relay: DebugRelaySummary,
        #[serde(default)]
        pub(crate) prompt_diagnostics: Vec<Prompt>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) model_route: Option<ModelRouteRecord>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub(crate) struct DebugRelaySummary {
        pub(crate) retained: Vec<String>,
        pub(crate) dropped: u64,
        pub(crate) truncated: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Text {
        pub(crate) chars: usize,
        pub(crate) preview: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct CargoValidation {
        pub(crate) call_id: String,
        pub(crate) command: String,
        pub(crate) display_command: String,
        pub(crate) ok: bool,
        pub(crate) status_reason: String,
        pub(crate) exit_code: Option<i32>,
        pub(crate) manifest_path: String,
        pub(crate) errors: u32,
        pub(crate) warnings: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum Event {
        Proposal {
            id: String,
            edit_count: usize,
            paths: Vec<PathBuf>,
        },
        ToolRequest {
            request_id: String,
            parent_id: String,
            call_id: String,
            tool: String,
            arguments: Text,
        },
        ToolCompleted {
            call_id: String,
            content: Text,
        },
        ToolFailed {
            call_id: String,
            error: Text,
        },
        AssistantMessage {
            id: String,
            status: String,
            content: Text,
        },
        Turn {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            session_id: Option<String>,
            request_id: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            parent_id: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            assistant_message_id: Option<String>,
            outcome: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            error_id: Option<String>,
            attempts: u32,
            summary: Text,
        },
        Outcome {
            outcome: super::record::Outcome,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub(crate) struct Prompt {
        pub(crate) parent_id: String,
        pub(crate) workspace: Workspace,
        pub(crate) bm25: Option<Bm25>,
        pub(crate) context_mode: String,
        pub(crate) max_leased_tokens: usize,
        pub(crate) estimated_total_tokens: usize,
        pub(crate) message_count: usize,
        pub(crate) message_previews: Vec<Message>,
        pub(crate) included_rag_parts: usize,
        pub(crate) rag_part_previews: Vec<RagPart>,
        pub(crate) rag_stats: Option<ContextStats>,
        pub(crate) fallback_notice: Option<String>,
    }

    impl Eq for Prompt {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Workspace {
        pub(crate) loaded: bool,
        pub(crate) root: Option<PathBuf>,
        pub(crate) member_count: usize,
        pub(crate) focused_root: Option<PathBuf>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Bm25 {
        pub(crate) status: String,
        pub(crate) docs: Option<usize>,
        pub(crate) error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Message {
        pub(crate) role: String,
        pub(crate) chars: usize,
        pub(crate) preview: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub(crate) struct RagPart {
        pub(crate) file_path: String,
        pub(crate) kind: String,
        pub(crate) estimated_tokens: usize,
        pub(crate) score: f32,
    }

    impl Eq for RagPart {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct ContextStats {
        pub(crate) total_tokens: usize,
        pub(crate) files: usize,
        pub(crate) parts: usize,
        pub(crate) truncated_parts: usize,
        pub(crate) dedup_removed: usize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Attempt {
        pub(crate) turn: u32,
        pub(crate) proposal_id: Option<String>,
        pub(crate) result: Result,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "result", rename_all = "snake_case")]
    pub(crate) enum Result {
        Applied { paths: Vec<PathBuf> },
        Rejected { feedback: String },
        PostApprovalIndeterminate { paths: Vec<PathBuf>, error: String },
        NoEdit { summary: String },
        ToolFailed { error: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "terminal", rename_all = "snake_case")]
    pub(crate) enum Terminal {
        Applied {
            proposal_id: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
            request_id: String,
            changed_paths: Vec<PathBuf>,
        },
        Exhausted {
            attempts: u32,
            last_feedback: String,
        },
        CompletedWithoutEdit {
            outcome: String,
            summary: String,
        },
        ToolFailed {
            error: String,
        },
        NoEdit,
        ContextUnavailable {
            reason: String,
        },
        ProviderUnavailable {
            reason: String,
        },
        SetupUnavailable {
            phase: String,
            reason: String,
        },
        AppliedValidationFailed {
            proposal_id: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
            changed_paths: Vec<PathBuf>,
            feedback: String,
        },
        AppliedValidationMissing {
            proposal_id: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
            changed_paths: Vec<PathBuf>,
            missing: Vec<String>,
        },
        AppliedTurnAborted {
            proposal_id: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
            changed_paths: Vec<PathBuf>,
            outcome: String,
            summary: String,
        },
        AppliedTimedOut {
            secs: u64,
            proposal_id: String,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            applied_proposal_ids: Vec<Uuid>,
            changed_paths: Vec<PathBuf>,
        },
        TimedOut {
            secs: u64,
        },
    }

    impl From<&HeadlessRun> for Summary {
        fn from(value: &HeadlessRun) -> Self {
            Self {
                attempts: value.attempts.iter().map(Attempt::from).collect(),
                terminal: value.terminal.as_ref().map(Terminal::from),
                events: value.events.iter().map(Event::from).collect(),
                validations: value
                    .validations
                    .iter()
                    .map(CargoValidation::from)
                    .collect(),
                debug_relay: DebugRelaySummary::from(&value.debug_relay),
                prompt_diagnostics: value.prompt_diagnostics.iter().map(Prompt::from).collect(),
                model_route: value.model_route.clone(),
            }
        }
    }

    impl From<&CargoValidationObservation> for CargoValidation {
        fn from(value: &CargoValidationObservation) -> Self {
            Self {
                call_id: value.call_id.clone(),
                command: value.command.clone(),
                display_command: value.display_command.clone(),
                ok: value.ok,
                status_reason: value.status_reason.clone(),
                exit_code: value.exit_code,
                manifest_path: value.manifest_path.clone(),
                errors: value.errors,
                warnings: value.warnings,
            }
        }
    }

    impl From<&super::Event> for Event {
        fn from(value: &super::Event) -> Self {
            match value {
                super::Event::Proposal {
                    id,
                    edit_count,
                    paths,
                } => Self::Proposal {
                    id: id.clone(),
                    edit_count: *edit_count,
                    paths: paths.clone(),
                },
                super::Event::ToolRequest {
                    request_id,
                    parent_id,
                    call_id,
                    tool,
                    arguments,
                } => Self::ToolRequest {
                    request_id: request_id.clone(),
                    parent_id: parent_id.clone(),
                    call_id: call_id.clone(),
                    tool: tool.clone(),
                    arguments: Text::from(arguments.as_str()),
                },
                super::Event::Tool { call_id, result } => match result {
                    super::Tool::Completed { content } => Self::ToolCompleted {
                        call_id: call_id.clone(),
                        content: Text::from(content.as_str()),
                    },
                    super::Tool::Failed { error } => Self::ToolFailed {
                        call_id: call_id.clone(),
                        error: Text::from(error.as_str()),
                    },
                },
                super::Event::AssistantMessage {
                    id,
                    status,
                    content,
                } => Self::AssistantMessage {
                    id: id.clone(),
                    status: status.clone(),
                    content: Text::from(content.as_str()),
                },
                super::Event::Turn {
                    session_id,
                    request_id,
                    parent_id,
                    assistant_message_id,
                    outcome,
                    error_id,
                    attempts,
                    summary,
                } => Self::Turn {
                    session_id: Some(session_id.clone()),
                    request_id: request_id.clone(),
                    parent_id: Some(parent_id.clone()),
                    assistant_message_id: Some(assistant_message_id.clone()),
                    outcome: outcome.clone(),
                    error_id: error_id.clone(),
                    attempts: *attempts,
                    summary: Text::from(summary.as_str()),
                },
                super::Event::Outcome(outcome) => Self::Outcome {
                    outcome: outcome.clone(),
                },
            }
        }
    }

    impl From<&str> for Text {
        fn from(value: &str) -> Self {
            Self {
                chars: value.chars().count(),
                preview: truncate_chars(value, MAX_EVIDENCE_EVENT_CHARS),
            }
        }
    }

    impl From<&super::PromptDiagnostic> for Prompt {
        fn from(value: &super::PromptDiagnostic) -> Self {
            Self {
                parent_id: value.parent_id.clone(),
                workspace: Workspace::from(&value.workspace),
                bm25: value.bm25.as_ref().map(Bm25::from),
                context_mode: value.context_mode.clone(),
                max_leased_tokens: value.max_leased_tokens,
                estimated_total_tokens: value.estimated_total_tokens,
                message_count: value.message_count,
                message_previews: value.message_previews.iter().map(Message::from).collect(),
                included_rag_parts: value.included_rag_parts,
                rag_part_previews: value.rag_part_previews.iter().map(RagPart::from).collect(),
                rag_stats: value.rag_stats.as_ref().map(ContextStats::from),
                fallback_notice: value.fallback_notice.clone(),
            }
        }
    }

    impl From<&super::WorkspaceDiagnostic> for Workspace {
        fn from(value: &super::WorkspaceDiagnostic) -> Self {
            Self {
                loaded: value.loaded,
                root: value.root.clone(),
                member_count: value.member_count,
                focused_root: value.focused_root.clone(),
            }
        }
    }

    impl From<&super::Bm25Diagnostic> for Bm25 {
        fn from(value: &super::Bm25Diagnostic) -> Self {
            Self {
                status: value.status.clone(),
                docs: value.docs,
                error: value.error.clone(),
            }
        }
    }

    impl From<&super::MessagePreview> for Message {
        fn from(value: &super::MessagePreview) -> Self {
            Self {
                role: value.role.clone(),
                chars: value.chars,
                preview: value.preview.clone(),
            }
        }
    }

    impl From<&super::RagPartPreview> for RagPart {
        fn from(value: &super::RagPartPreview) -> Self {
            Self {
                file_path: value.file_path.clone(),
                kind: value.kind.clone(),
                estimated_tokens: value.estimated_tokens,
                score: value.score,
            }
        }
    }

    impl From<&super::ContextStatsDiagnostic> for ContextStats {
        fn from(value: &super::ContextStatsDiagnostic) -> Self {
            Self {
                total_tokens: value.total_tokens,
                files: value.files,
                parts: value.parts,
                truncated_parts: value.truncated_parts,
                dedup_removed: value.dedup_removed,
            }
        }
    }

    impl From<&DebugRelay> for DebugRelaySummary {
        fn from(value: &DebugRelay) -> Self {
            Self {
                retained: value.retained().to_vec(),
                dropped: value.dropped(),
                truncated: value.truncated(),
            }
        }
    }

    impl From<&super::HeadlessAttempt> for Attempt {
        fn from(value: &super::HeadlessAttempt) -> Self {
            Self {
                turn: value.turn,
                proposal_id: value.proposal_id.map(|id| id.to_string()),
                result: Result::from(&value.result),
            }
        }
    }

    impl From<&HeadlessAttemptResult> for Result {
        fn from(value: &HeadlessAttemptResult) -> Self {
            match value {
                HeadlessAttemptResult::Applied { paths } => Self::Applied {
                    paths: paths.clone(),
                },
                HeadlessAttemptResult::Rejected { reason } => Self::Rejected {
                    feedback: reason.clone(),
                },
                HeadlessAttemptResult::PostApprovalIndeterminate { paths, error } => {
                    Self::PostApprovalIndeterminate {
                        paths: paths.clone(),
                        error: error.clone(),
                    }
                }
                HeadlessAttemptResult::NoEdit { summary } => Self::NoEdit {
                    summary: summary.clone(),
                },
                HeadlessAttemptResult::ToolFailed { error } => Self::ToolFailed {
                    error: error.clone(),
                },
            }
        }
    }

    impl From<&HeadlessTerminal> for Terminal {
        fn from(value: &HeadlessTerminal) -> Self {
            match value {
                HeadlessTerminal::Applied {
                    proposal_id,
                    applied_proposal_ids,
                    request_id,
                    changed_paths,
                } => Self::Applied {
                    proposal_id: proposal_id.to_string(),
                    applied_proposal_ids: applied_proposal_ids.clone(),
                    request_id: request_id.to_string(),
                    changed_paths: changed_paths.clone(),
                },
                HeadlessTerminal::Exhausted { attempts, last } => Self::Exhausted {
                    attempts: *attempts,
                    last_feedback: last.clone(),
                },
                HeadlessTerminal::CompletedWithoutEdit { outcome, summary } => {
                    Self::CompletedWithoutEdit {
                        outcome: outcome.clone(),
                        summary: summary.clone(),
                    }
                }
                HeadlessTerminal::ToolFailed { error } => Self::ToolFailed {
                    error: error.clone(),
                },
                HeadlessTerminal::NoEdit => Self::NoEdit,
                HeadlessTerminal::ContextUnavailable { reason } => Self::ContextUnavailable {
                    reason: reason.clone(),
                },
                HeadlessTerminal::ProviderUnavailable { reason } => Self::ProviderUnavailable {
                    reason: reason.clone(),
                },
                HeadlessTerminal::SetupUnavailable { phase, reason } => Self::SetupUnavailable {
                    phase: phase.to_string(),
                    reason: reason.clone(),
                },
                HeadlessTerminal::AppliedValidationFailed { applied, feedback } => {
                    Self::AppliedValidationFailed {
                        proposal_id: applied.proposal_id().to_string(),
                        applied_proposal_ids: applied.proposal_ids().to_vec(),
                        changed_paths: applied.changed_paths().to_vec(),
                        feedback: feedback.clone(),
                    }
                }
                HeadlessTerminal::AppliedValidationMissing { applied, missing } => {
                    Self::AppliedValidationMissing {
                        proposal_id: applied.proposal_id().to_string(),
                        applied_proposal_ids: applied.proposal_ids().to_vec(),
                        changed_paths: applied.changed_paths().to_vec(),
                        missing: missing.clone(),
                    }
                }
                HeadlessTerminal::AppliedTurnAborted {
                    applied,
                    outcome,
                    summary,
                } => Self::AppliedTurnAborted {
                    proposal_id: applied.proposal_id().to_string(),
                    applied_proposal_ids: applied.proposal_ids().to_vec(),
                    changed_paths: applied.changed_paths().to_vec(),
                    outcome: outcome.clone(),
                    summary: summary.clone(),
                },
                HeadlessTerminal::AppliedTimedOut { secs, applied } => Self::AppliedTimedOut {
                    secs: *secs,
                    proposal_id: applied.proposal_id().to_string(),
                    applied_proposal_ids: applied.proposal_ids().to_vec(),
                    changed_paths: applied.changed_paths().to_vec(),
                },
                HeadlessTerminal::TimedOut { secs } => Self::TimedOut { secs: *secs },
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    reference: request::Reference<request::Broad, request::Published>,
    request_path: PathBuf,
    prompt_path: PathBuf,
    workspace_path: PathBuf,
    evidence_roots: Vec<EvidenceRoot>,
    grant: surface::Grant,
    bounds: tui::Bounds,
    budget: Budget,
}

impl Request {
    pub(crate) fn from_published(
        published: &request::Request<request::Broad, request::Published>,
        grant: surface::Grant,
        bounds: tui::Bounds,
        budget: Budget,
    ) -> Self {
        Self {
            reference: published.reference(),
            request_path: published.request_path().to_path_buf(),
            prompt_path: published.prompt_path().to_path_buf(),
            workspace_path: published.workspace_path().to_path_buf(),
            evidence_roots: published.request().evidence_roots.clone(),
            grant,
            bounds,
            budget,
        }
    }

    pub(crate) fn reference(&self) -> &request::Reference<request::Broad, request::Published> {
        &self.reference
    }

    pub(crate) fn request_path(&self) -> &Path {
        &self.request_path
    }

    pub(crate) fn prompt_path(&self) -> &Path {
        &self.prompt_path
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub(crate) fn evidence_roots(&self) -> &[EvidenceRoot] {
        &self.evidence_roots
    }

    pub(crate) fn grant(&self) -> &surface::Grant {
        &self.grant
    }

    pub(crate) fn bounds(&self) -> &tui::Bounds {
        &self.bounds
    }

    pub(crate) fn budget(&self) -> &Budget {
        &self.budget
    }

    pub(crate) fn attempt(&self, number: u32) -> Attempt<state::Ready> {
        Attempt {
            core: Core {
                id: Id::for_request(self.reference.request_id(), number),
                number,
                request: self.clone(),
                events: Vec::new(),
            },
            state: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
    max_attempts: u32,
    timeout_secs: u64,
}

impl Budget {
    pub(crate) fn new(max_attempts: u32, timeout_secs: u64) -> Result<Self, Error> {
        if max_attempts == 0 {
            return Err(Error::EmptyBudget);
        }
        if timeout_secs == 0 {
            return Err(Error::EmptyTimeout);
        }
        Ok(Self {
            max_attempts,
            timeout_secs,
        })
    }

    pub(crate) fn max_attempts(self) -> u32 {
        self.max_attempts
    }

    pub(crate) fn timeout_secs(self) -> u64 {
        self.timeout_secs
    }

    pub(crate) fn retry(self) -> Retry {
        Retry {
            max_attempts: self.max_attempts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Id(String);

impl Id {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn for_request(request_id: &str, number: u32) -> Self {
        Self(format!("{request_id}:attempt-{number}"))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Core {
    id: Id,
    number: u32,
    request: Request,
    events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attempt<S> {
    core: Core,
    state: PhantomData<fn() -> S>,
}

impl Attempt<state::Ready> {
    pub(crate) fn start(self) -> Attempt<state::Running> {
        Attempt {
            core: self.core,
            state: PhantomData,
        }
    }
}

impl Attempt<state::Running> {
    pub(crate) fn observe(mut self, event: Event) -> Self {
        self.core.events.push(event);
        self
    }

    pub(crate) fn stage(&self, proposal: Proposal) -> Result<surface::Check, Error> {
        proposal
            .check(self.core.request.grant())
            .map_err(Error::Surface)
    }

    pub(crate) fn finish(self, outcome: Outcome) -> Attempt<state::Done> {
        let mut core = self.core;
        core.events.push(Event::Outcome(outcome.to_record()));
        Attempt {
            core,
            state: PhantomData,
        }
    }
}

impl<S> Attempt<S> {
    pub(crate) fn id(&self) -> &Id {
        &self.core.id
    }

    pub(crate) fn number(&self) -> u32 {
        self.core.number
    }

    pub(crate) fn request(&self) -> &Request {
        &self.core.request
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.core.events
    }
}

impl Attempt<state::Done> {
    pub(crate) fn outcome(&self) -> Option<Outcome> {
        self.core.events.iter().rev().find_map(Event::outcome)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    id: String,
    base: surface::Ref,
    after: surface::Ref,
    touches: Vec<surface::Touch>,
}

impl Proposal {
    pub(crate) fn new(
        id: impl Into<String>,
        base: surface::Ref,
        after: surface::Ref,
        touches: Vec<surface::Touch>,
    ) -> Self {
        Self {
            id: id.into(),
            base,
            after,
            touches,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn touches(&self) -> &[surface::Touch] {
        &self.touches
    }

    pub(crate) fn check(&self, grant: &surface::Grant) -> Result<surface::Check, surface::Error> {
        grant.check(surface::Draft {
            proposal: &self.id,
            base: &self.base,
            after: &self.after,
            touches: &self.touches,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BroadAttemptError {
    #[error("broad headless-tui setup failed during '{phase}': {detail}")]
    Setup { phase: &'static str, detail: String },
    #[error("broad headless-tui adapter failed: {0}")]
    Adapter(#[from] Error),
    #[error("broad headless-tui attempt ended without a terminal outcome")]
    MissingTerminal,
    #[error("broad headless-tui attempt failed during admission: {detail}")]
    Admission { detail: String },
    #[error("broad headless-tui attempt failed: {0}")]
    Prepare(#[from] PrepareError),
}

impl From<BroadAttemptError> for PrepareError {
    fn from(source: BroadAttemptError) -> Self {
        match source {
            BroadAttemptError::Setup { phase, detail } => Self::DatabaseSetup { phase, detail },
            BroadAttemptError::Adapter(err) => {
                if let Some((phase, detail)) = err.setup_failure() {
                    Self::DatabaseSetup {
                        phase,
                        detail: detail.to_string(),
                    }
                } else {
                    Self::InvalidBatchSelection {
                        detail: err.to_string(),
                    }
                }
            }
            BroadAttemptError::MissingTerminal => Self::InvalidBatchSelection {
                detail: "broad headless-tui attempt ended without a terminal outcome".to_string(),
            },
            BroadAttemptError::Admission { detail } => Self::InvalidBatchSelection { detail },
            BroadAttemptError::Prepare(err) => err,
        }
    }
}

impl BroadAttemptError {
    pub(crate) fn setup_phase(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::Setup { phase, detail } => Some((*phase, detail.as_str())),
            Self::Adapter(source) => source.setup_failure(),
            Self::Prepare(PrepareError::DatabaseSetup { phase, detail }) => {
                Some((*phase, detail.as_str()))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Applied(Applied),
    Rejected(Reject),
    NoEdit(NoEdit),
    Tool(Fail),
    TimedOut(Timeout),
    Validation(Fail),
}

impl Outcome {
    pub(crate) fn applied(check: surface::Check, validation: Validation) -> Self {
        Self::Applied(Applied {
            delta: ArtifactDelta::from_check(check),
            validation,
        })
    }

    pub(crate) fn retryable(&self) -> bool {
        match self {
            Self::Applied(_) => false,
            Self::Rejected(_)
            | Self::NoEdit(_)
            | Self::Tool(_)
            | Self::TimedOut(_)
            | Self::Validation(_) => true,
        }
    }

    fn to_record(&self) -> record::Outcome {
        record::Outcome::from(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Applied {
    delta: ArtifactDelta,
    validation: Validation,
}

impl Applied {
    pub(crate) fn delta(&self) -> &ArtifactDelta {
        &self.delta
    }

    pub(crate) fn validation(&self) -> &Validation {
        &self.validation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reject {
    Protected { paths: Vec<PathBuf> },
    Outside { paths: Vec<PathBuf> },
    Empty,
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoEdit {
    summary: String,
}

impl NoEdit {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
        }
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fail {
    kind: FailKind,
    detail: String,
}

impl Fail {
    pub(crate) fn tool(detail: impl Into<String>) -> Self {
        Self {
            kind: FailKind::Tool,
            detail: detail.into(),
        }
    }

    pub(crate) fn validation(detail: impl Into<String>) -> Self {
        Self {
            kind: FailKind::Validation,
            detail: detail.into(),
        }
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailKind {
    Tool,
    Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timeout {
    secs: u64,
}

impl Timeout {
    pub(crate) fn new(secs: u64) -> Self {
        Self { secs }
    }

    pub(crate) fn secs(self) -> u64 {
        self.secs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Validation {
    NotRun,
    Passed { commands: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Event {
    Proposal {
        id: String,
        edit_count: usize,
        paths: Vec<PathBuf>,
    },
    ToolRequest {
        request_id: String,
        parent_id: String,
        call_id: String,
        tool: String,
        arguments: String,
    },
    Tool {
        call_id: String,
        result: Tool,
    },
    AssistantMessage {
        id: String,
        status: String,
        content: String,
    },
    Turn {
        session_id: String,
        request_id: String,
        parent_id: String,
        assistant_message_id: String,
        outcome: String,
        error_id: Option<String>,
        attempts: u32,
        summary: String,
    },
    Outcome(record::Outcome),
}

impl Event {
    fn outcome(&self) -> Option<Outcome> {
        match self {
            Self::Outcome(record) => Some(Outcome::from(record)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Tool {
    Completed { content: String },
    Failed { error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Retry {
    max_attempts: u32,
}

impl Retry {
    pub(crate) fn decide_outcome(&self, attempt_number: u32, outcome: &Outcome) -> Step {
        if !outcome.retryable() {
            return Step::Terminal(Terminal::Invalid {
                reason: "finished attempt recorded a non-retryable outcome".to_string(),
            });
        }
        if attempt_number >= self.max_attempts {
            Step::Terminal(Terminal::Exhausted {
                attempts: attempt_number,
                last: outcome.clone(),
            })
        } else {
            Step::Retry {
                next_attempt: attempt_number + 1,
                feedback: Feedback::from_outcome(outcome),
            }
        }
    }

    pub(crate) fn decide(&self, attempt: &Attempt<state::Done>) -> Step {
        let Some(outcome) = attempt.outcome() else {
            return Step::Terminal(Terminal::Invalid {
                reason: "finished attempt did not record an outcome".to_string(),
            });
        };
        if !outcome.retryable() {
            return Step::Terminal(Terminal::Accepted(match outcome {
                Outcome::Applied(applied) => applied,
                _ => unreachable!("non-retryable outcome must be applied"),
            }));
        }
        if attempt.number() >= self.max_attempts {
            Step::Terminal(Terminal::Exhausted {
                attempts: attempt.number(),
                last: outcome,
            })
        } else {
            Step::Retry {
                next_attempt: attempt.number() + 1,
                feedback: Feedback::from_outcome(&outcome),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Retry {
        next_attempt: u32,
        feedback: Feedback,
    },
    Terminal(Terminal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Terminal {
    Accepted(Applied),
    Exhausted { attempts: u32, last: Outcome },
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Feedback {
    message: String,
}

impl Feedback {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn from_outcome(
        outcome: &Outcome,
    ) -> Self {
        let message = match outcome {
            Outcome::Applied(_) => "candidate accepted".to_string(),
            Outcome::Rejected(Reject::Protected { paths }) => {
                format!("Rejected protected paths: {}", join_paths(paths))
            }
            Outcome::Rejected(Reject::Outside { paths }) => {
                format!("Rejected out-of-surface paths: {}", join_paths(paths))
            }
            Outcome::Rejected(Reject::Empty) => {
                "No material patch was produced; try a concrete edit.".to_string()
            }
            Outcome::Rejected(Reject::Invalid { reason }) => reason.clone(),
            Outcome::NoEdit(no_edit) => no_edit.summary().to_string(),
            Outcome::Tool(fail) | Outcome::Validation(fail) => fail.detail().to_string(),
            Outcome::TimedOut(timeout) => format!(
                "The headless TUI attempt timed out after {} seconds; try again with a smaller edit.",
                timeout.secs()
            ),
        };
        Self { message }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("adapter attempt budget must allow at least one attempt")]
    EmptyBudget,
    #[error("adapter timeout must be nonzero")]
    EmptyTimeout,
    #[error("failed to start headless ploke-tui harness: {0}")]
    HeadlessStart(String),
    #[error(
        "failed to start headless ploke-tui harness: database setup failed during '{phase}': {detail}"
    )]
    HeadlessSetup { phase: &'static str, detail: String },
    #[error("headless ploke-tui event stream failed: {0}")]
    HeadlessEvent(String),
    #[error("proposal failed surface check: {0}")]
    Surface(#[from] surface::Error),
}

impl Error {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn from_headless_start(
        source: PrepareError,
    ) -> Self {
        match source {
            PrepareError::DatabaseSetup { phase, detail } => Self::HeadlessSetup { phase, detail },
            PrepareError::Timeout { phase, secs } => Self::HeadlessSetup {
                phase,
                detail: format!("timed out after {secs} seconds"),
            },
            other => Self::HeadlessStart(other.to_string()),
        }
    }

    pub(crate) fn setup_failure(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::HeadlessSetup { phase, detail } => Some((*phase, detail.as_str())),
            _ => None,
        }
    }
}

pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn join_paths(
    paths: &[PathBuf],
) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) mod record {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use super::{Applied, Fail, NoEdit, Outcome as ActiveOutcome, Reject, Timeout, Validation};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Run {
        pub(crate) request_id: String,
        pub(crate) request_hash: String,
        pub(crate) attempts: Vec<Attempt>,
        pub(crate) terminal: Option<Terminal>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Attempt {
        pub(crate) id: String,
        pub(crate) number: u32,
        pub(crate) outcome: Option<Outcome>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum Terminal {
        Accepted { changed_paths: Vec<PathBuf> },
        Exhausted { attempts: u32, last: Outcome },
        Invalid { reason: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "outcome", rename_all = "snake_case")]
    pub(crate) enum Outcome {
        Applied {
            base_hash: String,
            after_hash: String,
            changed_paths: Vec<PathBuf>,
            validation: Validation,
        },
        Rejected {
            reason: RejectRecord,
        },
        NoEdit {
            summary: String,
        },
        Failed {
            fail_kind: super::FailKind,
            detail: String,
        },
        TimedOut {
            secs: u64,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum RejectRecord {
        Protected { paths: Vec<PathBuf> },
        Outside { paths: Vec<PathBuf> },
        Empty,
        Invalid { reason: String },
    }

    impl From<&ActiveOutcome> for Outcome {
        fn from(value: &ActiveOutcome) -> Self {
            match value {
                ActiveOutcome::Applied(applied) => Outcome::from(applied),
                ActiveOutcome::Rejected(reject) => Self::Rejected {
                    reason: RejectRecord::from(reject),
                },
                ActiveOutcome::NoEdit(no_edit) => Outcome::from(no_edit),
                ActiveOutcome::Tool(fail) | ActiveOutcome::Validation(fail) => Outcome::from(fail),
                ActiveOutcome::TimedOut(timeout) => Outcome::from(timeout),
            }
        }
    }

    impl From<&Applied> for Outcome {
        fn from(value: &Applied) -> Self {
            let delta = value.delta();
            Self::Applied {
                base_hash: delta.base().hash().as_str().to_string(),
                after_hash: delta.after().hash().as_str().to_string(),
                changed_paths: delta
                    .touches()
                    .iter()
                    .map(|touch| touch.span().path().clone())
                    .collect(),
                validation: value.validation().clone(),
            }
        }
    }

    impl From<&Reject> for RejectRecord {
        fn from(value: &Reject) -> Self {
            match value {
                Reject::Protected { paths } => Self::Protected {
                    paths: paths.clone(),
                },
                Reject::Outside { paths } => Self::Outside {
                    paths: paths.clone(),
                },
                Reject::Empty => Self::Empty,
                Reject::Invalid { reason } => Self::Invalid {
                    reason: reason.clone(),
                },
            }
        }
    }

    impl From<&NoEdit> for Outcome {
        fn from(value: &NoEdit) -> Self {
            Self::NoEdit {
                summary: value.summary().to_string(),
            }
        }
    }

    impl From<&Fail> for Outcome {
        fn from(value: &Fail) -> Self {
            Self::Failed {
                fail_kind: value.kind,
                detail: value.detail().to_string(),
            }
        }
    }

    impl From<&Timeout> for Outcome {
        fn from(value: &Timeout) -> Self {
            Self::TimedOut { secs: value.secs() }
        }
    }
}
pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn push_changed_paths(
    changed_paths: &mut Vec<PathBuf>,
    paths: Vec<PathBuf>,
) {
    for path in paths {
        if !changed_paths.contains(&path) {
            changed_paths.push(path);
        }
    }
}

impl From<&record::Outcome> for Outcome {
    fn from(value: &record::Outcome) -> Self {
        match value {
            record::Outcome::Applied { .. } => Self::Rejected(Reject::Invalid {
                reason: "durable applied record cannot reconstruct active artifact delta"
                    .to_string(),
            }),
            record::Outcome::Rejected { reason } => Self::Rejected(Reject::from(reason)),
            record::Outcome::NoEdit { summary } => Self::NoEdit(NoEdit::new(summary.clone())),
            record::Outcome::Failed { fail_kind, detail } => match fail_kind {
                FailKind::Tool => Self::Tool(Fail::tool(detail.clone())),
                FailKind::Validation => Self::Validation(Fail::validation(detail.clone())),
            },
            record::Outcome::TimedOut { secs } => Self::TimedOut(Timeout::new(*secs)),
        }
    }
}

impl From<&record::RejectRecord> for Reject {
    fn from(value: &record::RejectRecord) -> Self {
        match value {
            record::RejectRecord::Protected { paths } => Self::Protected {
                paths: paths.clone(),
            },
            record::RejectRecord::Outside { paths } => Self::Outside {
                paths: paths.clone(),
            },
            record::RejectRecord::Empty => Self::Empty,
            record::RejectRecord::Invalid { reason } => Self::Invalid {
                reason: reason.clone(),
            },
        }
    }
}
