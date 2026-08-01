//! Typed, read-only observation of mutable LLM/tool-loop evidence.
//!
//! This projection keeps debugger checkpoints, the enclosing agent turn, and
//! the outer headless result as separate evidence layers. It never converts
//! their independent lifecycle states into one synthetic status.

use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use ploke_records::{
    agent_turn::{
        AgentTurnSummaryRecord, ObservedTurnEventRecord, RequestMessageRecord, ToolRequestRecord,
    },
    ids::CampaignId,
    llm_response::RawFullResponseRecord,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    campaign_manifest_path,
    cli::prototype1_state::{
        edit_surface::harness_request::PublishedBroadHarnessRequest,
        edit_surface::tui_adapter::evidence::Terminal, identity,
    },
    replay::tool_loop::{
        ToolLoopOutcome, ToolLoopResult, ToolLoopResume, ToolLoopSession, ToolLoopStatus,
        ToolLoopStep, WorkspaceState, decode_resume, decode_session, decode_step,
    },
    runner::request_message_record,
    spec::PrepareError,
};

use super::{
    epoch::ServerEpoch,
    protocol::SessionVersion,
    trace::{TraceEvidence, TraceSource, TraceSourceKind},
};

const ERROR_PHASE: &str = "prototype1_state_walk_llm_trace";
const READ_ATTEMPTS: usize = 3;
const MAX_TRACE_STEPS: usize = 1_024;

pub use crate::{
    cli::prototype1_state::edit_surface::tui_adapter::evidence::Terminal as LlmHeadlessTerminal,
    replay::tool_loop::{
        ToolLoopOutcome as LlmStepOutcome, ToolLoopResult as LlmToolResult,
        ToolLoopStatus as LlmSessionStatus, WorkspaceState as LlmWorkspaceState,
    },
};

/// Stable coordinate for one exact debugger session and optional response step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmTraceCoordinate {
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<usize>,
}

/// Authority surface used for this mutable observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmTraceAuthority {
    ToolLoopCheckpoint,
}

/// Explicit state of one expected evidence file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LlmArtifact<T> {
    Present { evidence: TraceEvidence<T> },
    Missing { path: PathBuf },
    Unreadable { path: PathBuf, detail: String },
    Invalid { source: TraceSource, detail: String },
}

/// Public debugger-session projection. The source remains the persisted
/// session manifest; storage schema fields are intentionally not public wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSession {
    pub session_id: String,
    pub lane_id: String,
    pub workspace: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub status: ToolLoopStatus,
}

/// Compact publication frontier from `resume.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmResume {
    pub next_step: usize,
    pub assistant_message_id: String,
    pub parent_id: String,
    pub request_id: String,
    pub attempts: u32,
    pub terminal: bool,
}

/// One valid debugger session retained under its fanout lane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSessionSummary {
    pub session: TraceEvidence<LlmSession>,
    pub resume: LlmArtifact<LlmResume>,
}

/// All exact sessions that share one lane identity. No representative or
/// chronological "latest" session is synthesized.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmLane {
    pub lane_id: String,
    pub sessions: Vec<LlmSessionSummary>,
}

/// A missing or malformed manifest remains visible without hiding healthy
/// siblings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LlmTraceIssue {
    Missing {
        session_id: String,
        path: PathBuf,
    },
    Invalid {
        session_id: String,
        source: TraceSource,
        detail: String,
    },
    Unreadable {
        session_id: String,
        path: PathBuf,
        detail: String,
    },
}

/// Campaign-scoped inventory of all debugger sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmTraceIndex {
    pub version: SessionVersion,
    pub epoch: ServerEpoch,
    pub campaign: CampaignId,
    pub root: PathBuf,
    pub authority: LlmTraceAuthority,
    pub lanes: Vec<LlmLane>,
    pub issues: Vec<LlmTraceIssue>,
}

/// Compact chronological row for one published provider-response checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmStepSummary {
    pub step: usize,
    pub outcome: ToolLoopOutcome,
    pub tool_requests: usize,
    pub tool_results: usize,
    pub terminal: bool,
}

/// A published timeline position or its exact integrity failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LlmStepEntry {
    Present {
        summary: TraceEvidence<LlmStepSummary>,
    },
    Invalid {
        step: usize,
        source: TraceSource,
        detail: String,
    },
    Missing {
        step: usize,
        path: PathBuf,
    },
    Unreadable {
        step: usize,
        path: PathBuf,
        detail: String,
    },
}

/// Selected full checkpoint detail. Growing request history appears only once,
/// for this exact selected step, so an index cannot exceed the IPC frame merely
/// by repeating transcripts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmStepDetail {
    pub step: usize,
    pub request_messages: Vec<RequestMessageRecord>,
    pub response: RawFullResponseRecord,
    pub outcome: ToolLoopOutcome,
    pub tool_requests: Vec<ToolRequestRecord>,
    pub tool_results: Vec<ToolLoopResult>,
    pub events: Vec<ObservedTurnEventRecord>,
    pub workspace_before: WorkspaceState,
    pub workspace_after: WorkspaceState,
    pub terminal: bool,
}

/// Evidence derived from one typed published broad-harness request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmOuterEvidence {
    pub headless: LlmArtifact<Option<Terminal>>,
    pub turn: LlmArtifact<AgentTurnSummaryRecord>,
}

/// Exact-session observation with independent debugger and outer-executor
/// evidence. Absence is never relabeled as "in flight" without job authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmTraceSnapshot {
    pub coordinate: LlmTraceCoordinate,
    pub version: SessionVersion,
    pub epoch: ServerEpoch,
    pub authority: LlmTraceAuthority,
    pub session: TraceEvidence<LlmSession>,
    pub resume: LlmArtifact<LlmResume>,
    pub timeline: Vec<LlmStepEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<LlmArtifact<LlmStepDetail>>,
    pub outer: LlmArtifact<LlmOuterEvidence>,
}

pub(crate) fn load_index(
    repo_root: &Path,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<LlmTraceIndex, PrepareError> {
    let scope = LlmScope::load(repo_root)?;
    let (lanes, issues) = load_lanes(&scope.root, Some(&scope.campaign))?;
    Ok(LlmTraceIndex {
        version,
        epoch,
        campaign: scope.campaign,
        root: scope.root,
        authority: LlmTraceAuthority::ToolLoopCheckpoint,
        lanes,
        issues,
    })
}

pub(crate) fn load_trace(
    repo_root: &Path,
    coordinate: LlmTraceCoordinate,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<LlmTraceSnapshot, PrepareError> {
    validate_component("session id", &coordinate.session_id)?;
    let scope = LlmScope::load(repo_root)?;
    for _ in 0..READ_ATTEMPTS {
        let read = load_trace_once(&scope, coordinate.clone(), version.clone(), epoch.clone())?;
        if read.stable()? {
            return Ok(read.snapshot);
        }
    }
    Err(trace_error(format!(
        "tool-loop session '{}' changed during {} observation attempts; retry the read",
        coordinate.session_id, READ_ATTEMPTS
    )))
}

fn load_lanes(
    root: &Path,
    campaign: Option<&CampaignId>,
) -> Result<(Vec<LlmLane>, Vec<LlmTraceIssue>), PrepareError> {
    if !root.is_dir() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut entries = fs::read_dir(root)
        .map_err(|source| PrepareError::ReadManifest {
            path: root.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| PrepareError::ReadManifest {
            path: root.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut lanes = BTreeMap::<String, Vec<LlmSessionSummary>>::new();
    let mut issues = Vec::new();
    for entry in entries {
        if !entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?
            .is_dir()
        {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path().join("session.json");
        let Some(record) = read_session(root, &id, &path, &mut issues)? else {
            continue;
        };
        if let Some(expected) = campaign
            && record
                .value
                .campaign_id
                .as_deref()
                .is_some_and(|actual| actual != expected.as_str())
        {
            issues.push(LlmTraceIssue::Invalid {
                session_id: id,
                source: record.source,
                detail: format!(
                    "session campaign '{}' does not match admitted campaign '{}'",
                    record.value.campaign_id.as_deref().unwrap_or("-"),
                    expected
                ),
            });
            continue;
        }
        let lane = lane_label(&record.value);
        let resume = load_resume(root, &entry.path(), &record.value.session_id)?.artifact;
        let session = TraceEvidence {
            value: session_view(&record.value, lane.clone()),
            source: record.source,
        };
        lanes
            .entry(lane)
            .or_default()
            .push(LlmSessionSummary { session, resume });
    }

    let lanes = lanes
        .into_iter()
        .map(|(lane_id, mut sessions)| {
            sessions.sort_by(|left, right| {
                left.session
                    .value
                    .session_id
                    .cmp(&right.session.value.session_id)
            });
            LlmLane { lane_id, sessions }
        })
        .collect();
    Ok((lanes, issues))
}

#[derive(Debug)]
struct LlmScope {
    campaign: CampaignId,
    campaign_dir: PathBuf,
    root: PathBuf,
}

impl LlmScope {
    fn load(repo_root: &Path) -> Result<Self, PrepareError> {
        let identity = identity::load_parent_identity_optional(repo_root)?.ok_or_else(|| {
            trace_error(format!(
                "parent identity is required to locate LLM trace evidence: {}",
                identity::parent_identity_path(repo_root).display()
            ))
        })?;
        let campaign = identity.campaign_id().clone();
        let manifest = campaign_manifest_path(&campaign)?;
        let campaign_dir = manifest
            .parent()
            .ok_or_else(|| {
                trace_error(format!(
                    "campaign manifest '{}' has no parent",
                    manifest.display()
                ))
            })?
            .to_path_buf();
        let root = bounded_path(
            &campaign_dir,
            &campaign_dir.join("prototype1/debug/tool-loop"),
        )?;
        Ok(Self {
            campaign,
            campaign_dir,
            root,
        })
    }
}

#[derive(Debug)]
struct Stored<T> {
    value: T,
    source: TraceSource,
}

#[derive(Debug)]
struct ResumeRead {
    artifact: LlmArtifact<LlmResume>,
    record: Option<Stored<ToolLoopResume>>,
}

#[derive(Debug)]
struct TraceRead {
    snapshot: LlmTraceSnapshot,
    root: PathBuf,
    session_path: PathBuf,
    session_hash: String,
    resume_path: PathBuf,
    resume_hash: Option<String>,
}

impl TraceRead {
    fn stable(&self) -> Result<bool, PrepareError> {
        if file_hash(&self.root, &self.session_path)?.as_deref() != Some(self.session_hash.as_str())
        {
            return Ok(false);
        }
        Ok(file_hash(&self.root, &self.resume_path)? == self.resume_hash)
    }
}

fn load_trace_once(
    scope: &LlmScope,
    coordinate: LlmTraceCoordinate,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<TraceRead, PrepareError> {
    let dir = scope.root.join(&coordinate.session_id);
    let session_path = dir.join("session.json");
    let session = load_exact_session(&scope.root, &coordinate.session_id, &session_path)?;
    if session
        .value
        .campaign_id
        .as_deref()
        .is_some_and(|actual| actual != scope.campaign.as_str())
    {
        return Err(trace_error(format!(
            "session '{}' belongs to campaign '{}' instead of admitted campaign '{}'",
            coordinate.session_id,
            session.value.campaign_id.as_deref().unwrap_or("-"),
            scope.campaign
        )));
    }
    let lane = lane_label(&session.value);
    let resume_path = dir.join("resume.json");
    let resume = load_resume(&scope.root, &dir, &coordinate.session_id)?;
    let frontier = resume.record.as_ref().map(|record| record.value.next_step);
    if let Some(frontier) = frontier {
        validate_frontier_bound(frontier)?;
    }
    if let Some(step) = coordinate.step
        && frontier.is_none_or(|next| step >= next)
    {
        return Err(trace_error(format!(
            "step {step} is outside session '{}' published resume frontier {}",
            coordinate.session_id,
            frontier
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(missing)".to_string())
        )));
    }
    let selected_step = coordinate
        .step
        .or_else(|| frontier.and_then(|next| next.checked_sub(1)));
    let mut timeline = Vec::new();
    let mut selected = selected_step.map(|step| LlmArtifact::Missing {
        path: step_path(&dir, step),
    });
    if let Some(resume_record) = resume.record.as_ref() {
        for step in 0..resume_record.value.next_step {
            let path = step_path(&dir, step);
            match load_step(
                &scope.root,
                &path,
                &coordinate.session_id,
                step,
                &resume_record.value,
            )? {
                StepRead::Present(record) => {
                    if selected_step == Some(step) {
                        selected = Some(LlmArtifact::Present {
                            evidence: TraceEvidence {
                                value: step_detail(&record.value),
                                source: record.source.clone(),
                            },
                        });
                    }
                    timeline.push(LlmStepEntry::Present {
                        summary: TraceEvidence {
                            value: step_summary(&record.value),
                            source: record.source,
                        },
                    });
                }
                StepRead::Invalid { source, detail } => {
                    if selected_step == Some(step) {
                        selected = Some(LlmArtifact::Invalid {
                            source: source.clone(),
                            detail: detail.clone(),
                        });
                    }
                    timeline.push(LlmStepEntry::Invalid {
                        step,
                        source,
                        detail,
                    });
                }
                StepRead::Missing => {
                    timeline.push(LlmStepEntry::Missing {
                        step,
                        path: path.clone(),
                    });
                }
                StepRead::Unreadable { detail } => {
                    if selected_step == Some(step) {
                        selected = Some(LlmArtifact::Unreadable {
                            path: path.clone(),
                            detail: detail.clone(),
                        });
                    }
                    timeline.push(LlmStepEntry::Unreadable { step, path, detail });
                }
            }
        }
        validate_frontier(&timeline, &resume_record.value)?;
    }
    let outer = load_outer(
        scope,
        &session.value,
        &session.source,
        resume.record.as_ref(),
    )?;
    let session_hash = session.source.content_sha256.clone();
    let resume_hash = resume
        .record
        .as_ref()
        .map(|record| record.source.content_sha256.clone())
        .or_else(|| {
            if let LlmArtifact::Invalid { source, .. } = &resume.artifact {
                Some(source.content_sha256.clone())
            } else {
                None
            }
        });
    let public_session = TraceEvidence {
        value: session_view(&session.value, lane),
        source: session.source,
    };
    let snapshot = LlmTraceSnapshot {
        coordinate,
        version,
        epoch,
        authority: LlmTraceAuthority::ToolLoopCheckpoint,
        session: public_session,
        resume: resume.artifact,
        timeline,
        selected,
        outer,
    };
    Ok(TraceRead {
        snapshot,
        root: scope.root.clone(),
        session_path,
        session_hash,
        resume_path,
        resume_hash,
    })
}

fn read_session(
    root: &Path,
    id: &str,
    path: &Path,
    issues: &mut Vec<LlmTraceIssue>,
) -> Result<Option<Stored<ToolLoopSession>>, PrepareError> {
    let path = match bounded_path(root, path) {
        Ok(path) => path,
        Err(error) => {
            issues.push(LlmTraceIssue::Unreadable {
                session_id: id.to_string(),
                path: path.to_path_buf(),
                detail: error.to_string(),
            });
            return Ok(None);
        }
    };
    let body = match read_optional(&path) {
        Ok(Some(body)) => body,
        Ok(None) => {
            issues.push(LlmTraceIssue::Missing {
                session_id: id.to_string(),
                path,
            });
            return Ok(None);
        }
        Err(error) => {
            issues.push(LlmTraceIssue::Unreadable {
                session_id: id.to_string(),
                path,
                detail: error.to_string(),
            });
            return Ok(None);
        }
    };
    let source = trace_source(TraceSourceKind::ToolLoopSession, &path, &body);
    match decode_session(&path, &body) {
        Ok(session) if session.session_id == id => Ok(Some(Stored {
            value: session,
            source,
        })),
        Ok(session) => {
            issues.push(LlmTraceIssue::Invalid {
                session_id: id.to_string(),
                source,
                detail: format!(
                    "session payload id '{}' does not match directory id '{id}'",
                    session.session_id
                ),
            });
            Ok(None)
        }
        Err(error) => {
            issues.push(LlmTraceIssue::Invalid {
                session_id: id.to_string(),
                source,
                detail: error.to_string(),
            });
            Ok(None)
        }
    }
}

fn load_exact_session(
    root: &Path,
    id: &str,
    path: &Path,
) -> Result<Stored<ToolLoopSession>, PrepareError> {
    let path = bounded_path(root, path)?;
    let body = read_optional(&path)?.ok_or_else(|| {
        trace_error(format!(
            "tool-loop session '{}' has no manifest at '{}'",
            id,
            path.display()
        ))
    })?;
    let source = trace_source(TraceSourceKind::ToolLoopSession, &path, &body);
    let session = decode_session(&path, &body)?;
    if session.session_id != id {
        return Err(trace_error(format!(
            "session payload id '{}' does not match requested id '{id}'",
            session.session_id
        )));
    }
    Ok(Stored {
        value: session,
        source,
    })
}

fn load_resume(root: &Path, dir: &Path, session_id: &str) -> Result<ResumeRead, PrepareError> {
    let path = dir.join("resume.json");
    let path = match bounded_path(root, &path) {
        Ok(path) => path,
        Err(error) => {
            return Ok(ResumeRead {
                artifact: LlmArtifact::Unreadable {
                    path,
                    detail: error.to_string(),
                },
                record: None,
            });
        }
    };
    let body = match read_optional(&path) {
        Ok(Some(body)) => body,
        Ok(None) => {
            return Ok(ResumeRead {
                artifact: LlmArtifact::Missing { path },
                record: None,
            });
        }
        Err(error) => {
            return Ok(ResumeRead {
                artifact: LlmArtifact::Unreadable {
                    path,
                    detail: error.to_string(),
                },
                record: None,
            });
        }
    };
    let source = trace_source(TraceSourceKind::ToolLoopResume, &path, &body);
    let resume = match decode_resume(&path, &body) {
        Ok(resume) => resume,
        Err(error) => {
            return Ok(ResumeRead {
                artifact: LlmArtifact::Invalid {
                    source,
                    detail: error.to_string(),
                },
                record: None,
            });
        }
    };
    if resume.session_id != session_id {
        return Ok(ResumeRead {
            artifact: LlmArtifact::Invalid {
                source,
                detail: format!(
                    "resume session id '{}' does not match manifest session id '{session_id}'",
                    resume.session_id
                ),
            },
            record: None,
        });
    }
    let value = resume_view(&resume);
    Ok(ResumeRead {
        artifact: LlmArtifact::Present {
            evidence: TraceEvidence {
                value,
                source: source.clone(),
            },
        },
        record: Some(Stored {
            value: resume,
            source,
        }),
    })
}

enum StepRead {
    Present(Stored<ToolLoopStep>),
    Invalid { source: TraceSource, detail: String },
    Missing,
    Unreadable { detail: String },
}

fn load_step(
    root: &Path,
    path: &Path,
    session_id: &str,
    index: usize,
    resume: &ToolLoopResume,
) -> Result<StepRead, PrepareError> {
    let path = match bounded_path(root, path) {
        Ok(path) => path,
        Err(error) => {
            return Ok(StepRead::Unreadable {
                detail: error.to_string(),
            });
        }
    };
    let body = match read_optional(&path) {
        Ok(Some(body)) => body,
        Ok(None) => return Ok(StepRead::Missing),
        Err(error) => {
            return Ok(StepRead::Unreadable {
                detail: error.to_string(),
            });
        }
    };
    let source = trace_source(TraceSourceKind::ToolLoopStep, &path, &body);
    let step = match decode_step(&path, &body) {
        Ok(step) => step,
        Err(error) => {
            return Ok(StepRead::Invalid {
                source,
                detail: error.to_string(),
            });
        }
    };
    let invalid = if step.session_id != session_id {
        Some(format!(
            "step session id '{}' does not match requested session '{session_id}'",
            step.session_id
        ))
    } else if step.step_index != index {
        Some(format!(
            "step payload index {} does not match path index {index}",
            step.step_index
        ))
    } else if step.response.response_index().get() != index {
        Some(format!(
            "provider response index {} does not match checkpoint index {index}",
            step.response.response_index()
        ))
    } else if step.response.assistant_message_id.to_string() != resume.assistant_message_id {
        Some(format!(
            "step assistant '{}' does not match resume assistant '{}'",
            step.response.assistant_message_id, resume.assistant_message_id
        ))
    } else {
        None
    };
    if let Some(detail) = invalid {
        return Ok(StepRead::Invalid { source, detail });
    }
    Ok(StepRead::Present(Stored {
        value: step,
        source,
    }))
}

fn validate_frontier(
    timeline: &[LlmStepEntry],
    resume: &ToolLoopResume,
) -> Result<(), PrepareError> {
    let Some(last) = timeline.last() else {
        if resume.next_step == 0 {
            return Ok(());
        }
        return Err(trace_error(format!(
            "resume publishes {} step(s) but the timeline is empty",
            resume.next_step
        )));
    };
    if let LlmStepEntry::Present { summary } = last
        && summary.value.terminal != resume.terminal
    {
        return Err(trace_error(format!(
            "resume terminal={} disagrees with published head step {} terminal={}",
            resume.terminal, summary.value.step, summary.value.terminal
        )));
    }
    Ok(())
}

fn validate_frontier_bound(frontier: usize) -> Result<(), PrepareError> {
    if frontier <= MAX_TRACE_STEPS {
        return Ok(());
    }
    Err(trace_error(format!(
        "resume frontier {frontier} exceeds observation limit {MAX_TRACE_STEPS}"
    )))
}

fn load_outer(
    scope: &LlmScope,
    session: &ToolLoopSession,
    session_source: &TraceSource,
    resume: Option<&Stored<ToolLoopResume>>,
) -> Result<LlmArtifact<LlmOuterEvidence>, PrepareError> {
    let lane = lane_label(session);
    let request_path = match session.request_path.clone() {
        Some(path) => path,
        None => {
            if let Err(error) = validate_component("lane id", &lane) {
                return Ok(LlmArtifact::Invalid {
                    source: session_source.clone(),
                    detail: error.to_string(),
                });
            }
            scope
                .campaign_dir
                .join("prototype1/messages/edit-harness-request")
                .join(format!("{lane}.json"))
        }
    };
    let request_path = match bounded_path(&scope.campaign_dir, &request_path) {
        Ok(path) => path,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: session_source.clone(),
                detail: error.to_string(),
            });
        }
    };
    let body = match read_optional(&request_path) {
        Ok(Some(body)) => body,
        Ok(None) => return Ok(LlmArtifact::Missing { path: request_path }),
        Err(error) => {
            return Ok(LlmArtifact::Unreadable {
                path: request_path,
                detail: error.to_string(),
            });
        }
    };
    let request_source = trace_source(TraceSourceKind::HarnessRequest, &request_path, &body);
    let published: PublishedBroadHarnessRequest = match serde_json::from_slice(&body) {
        Ok(published) => published,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: request_source,
                detail: format!(
                    "failed to parse published request '{}': {error}",
                    request_path.display()
                ),
            });
        }
    };
    let declared_path = match bounded_path(&scope.campaign_dir, published.request_path()) {
        Ok(path) => path,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: request_source,
                detail: error.to_string(),
            });
        }
    };
    if declared_path != request_path {
        return Ok(LlmArtifact::Invalid {
            source: request_source,
            detail: format!(
                "published request declares '{}' but was read from '{}'",
                declared_path.display(),
                request_path.display()
            ),
        });
    }
    if !same_path(published.workspace_path(), &session.workspace) {
        return Ok(LlmArtifact::Invalid {
            source: request_source,
            detail: format!(
                "published workspace '{}' does not match debugger workspace '{}'",
                published.workspace_path().display(),
                session.workspace.display()
            ),
        });
    }
    let result_path = match bounded_path(&scope.campaign_dir, published.submitted_result_path()) {
        Ok(path) => path,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: request_source,
                detail: error.to_string(),
            });
        }
    };
    let headless_path = match bounded_path(
        &scope.campaign_dir,
        &result_path.with_extension("headless-tui.json"),
    ) {
        Ok(path) => path,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: request_source,
                detail: error.to_string(),
            });
        }
    };
    let turn_path = result_path
        .with_extension("turn-live")
        .join("agent-turn-summary.json");
    let turn_path = match bounded_path(&scope.campaign_dir, &turn_path) {
        Ok(path) => path,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source: request_source,
                detail: error.to_string(),
            });
        }
    };
    let headless = load_headless(&headless_path)?;
    let turn = load_turn(
        &turn_path,
        published.request_id(),
        session,
        resume.map(|record| &record.value),
    )?;
    Ok(LlmArtifact::Present {
        evidence: TraceEvidence {
            value: LlmOuterEvidence { headless, turn },
            source: request_source,
        },
    })
}

fn load_headless(path: &Path) -> Result<LlmArtifact<Option<Terminal>>, PrepareError> {
    let body = match read_optional(path) {
        Ok(Some(body)) => body,
        Ok(None) => {
            return Ok(LlmArtifact::Missing {
                path: path.to_path_buf(),
            });
        }
        Err(error) => {
            return Ok(LlmArtifact::Unreadable {
                path: path.to_path_buf(),
                detail: error.to_string(),
            });
        }
    };
    let source = trace_source(TraceSourceKind::HeadlessSummary, path, &body);
    let summary = match serde_json::from_slice::<
        crate::cli::prototype1_state::edit_surface::tui_adapter::evidence::Summary,
    >(&body)
    {
        Ok(summary) => summary,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source,
                detail: format!(
                    "failed to parse headless summary '{}': {error}",
                    path.display()
                ),
            });
        }
    };
    Ok(LlmArtifact::Present {
        evidence: TraceEvidence {
            value: summary.terminal().cloned(),
            source,
        },
    })
}

fn load_turn(
    path: &Path,
    request_id: &str,
    session: &ToolLoopSession,
    resume: Option<&ToolLoopResume>,
) -> Result<LlmArtifact<AgentTurnSummaryRecord>, PrepareError> {
    let body = match read_optional(path) {
        Ok(Some(body)) => body,
        Ok(None) => {
            return Ok(LlmArtifact::Missing {
                path: path.to_path_buf(),
            });
        }
        Err(error) => {
            return Ok(LlmArtifact::Unreadable {
                path: path.to_path_buf(),
                detail: error.to_string(),
            });
        }
    };
    let source = trace_source(TraceSourceKind::LiveTurnSummary, path, &body);
    let turn: AgentTurnSummaryRecord = match serde_json::from_slice(&body) {
        Ok(turn) => turn,
        Err(error) => {
            return Ok(LlmArtifact::Invalid {
                source,
                detail: format!(
                    "failed to parse agent-turn summary '{}': {error}",
                    path.display()
                ),
            });
        }
    };
    let invalid = if turn.0.task_id != request_id {
        Some(format!(
            "agent-turn task '{}' does not match published request '{request_id}'",
            turn.0.task_id
        ))
    } else if let Some(terminal) = turn.0.terminal_record.as_ref()
        && terminal.session_id != session.session_id
    {
        Some(format!(
            "agent-turn session '{}' does not match debugger session '{}'",
            terminal.session_id, session.session_id
        ))
    } else if let (Some(terminal), Some(resume)) = (turn.0.terminal_record.as_ref(), resume)
        && terminal.assistant_message_id != resume.assistant_message_id
    {
        Some(format!(
            "agent-turn assistant '{}' does not match resume assistant '{}'",
            terminal.assistant_message_id, resume.assistant_message_id
        ))
    } else if let (Some(terminal), Some(resume)) = (turn.0.terminal_record.as_ref(), resume)
        && terminal.parent_id != resume.parent_id
    {
        Some(format!(
            "agent-turn parent '{}' does not match resume parent '{}'",
            terminal.parent_id, resume.parent_id
        ))
    } else {
        None
    };
    if let Some(detail) = invalid {
        return Ok(LlmArtifact::Invalid { source, detail });
    }
    Ok(LlmArtifact::Present {
        evidence: TraceEvidence {
            value: turn,
            source,
        },
    })
}

fn session_view(session: &ToolLoopSession, lane_id: String) -> LlmSession {
    LlmSession {
        session_id: session.session_id.clone(),
        lane_id,
        workspace: session.workspace.clone(),
        model: session.model.clone(),
        status: session.status,
    }
}

fn resume_view(resume: &ToolLoopResume) -> LlmResume {
    LlmResume {
        next_step: resume.next_step,
        assistant_message_id: resume.assistant_message_id.clone(),
        parent_id: resume.parent_id.clone(),
        request_id: resume.request_id.clone(),
        attempts: resume.attempts,
        terminal: resume.terminal,
    }
}

fn step_summary(step: &ToolLoopStep) -> LlmStepSummary {
    LlmStepSummary {
        step: step.step_index,
        outcome: step.outcome.clone(),
        tool_requests: step.tool_requests.len(),
        tool_results: step.tool_results.len(),
        terminal: step.terminal,
    }
}

fn step_detail(step: &ToolLoopStep) -> LlmStepDetail {
    LlmStepDetail {
        step: step.step_index,
        request_messages: step
            .request_messages
            .iter()
            .map(request_message_record)
            .collect(),
        response: step.response.clone(),
        outcome: step.outcome.clone(),
        tool_requests: step.tool_requests.clone(),
        tool_results: step.tool_results.clone(),
        events: step.events.clone(),
        workspace_before: step.workspace_before.clone(),
        workspace_after: step.workspace_after.clone(),
        terminal: step.terminal,
    }
}

fn lane_label(session: &ToolLoopSession) -> String {
    session
        .lane_id
        .clone()
        .or_else(|| {
            session
                .workspace
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| session.session_id.clone())
}

fn step_path(dir: &Path, step: usize) -> PathBuf {
    dir.join("steps").join(format!("{step:04}.json"))
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, PrepareError> {
    match fs::read(path) {
        Ok(body) => Ok(Some(body)),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn file_hash(root: &Path, path: &Path) -> Result<Option<String>, PrepareError> {
    let path = match bounded_path(root, path) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    match read_optional(&path) {
        Ok(body) => Ok(body.map(|body| hex_sha256(&body))),
        Err(_) => Ok(None),
    }
}

fn bounded_path(root: &Path, path: &Path) -> Result<PathBuf, PrepareError> {
    let unsafe_component = path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir));
    if !path.is_absolute() || unsafe_component {
        return Err(path_escape(root, path));
    }
    let root = fs::canonicalize(root).map_err(|source| PrepareError::ReadManifest {
        path: root.to_path_buf(),
        source,
    })?;
    match fs::canonicalize(path) {
        Ok(resolved) => {
            if !resolved.starts_with(&root) {
                return Err(path_escape(&root, path));
            }
            Ok(resolved)
        }
        Err(source) if source.kind() == ErrorKind::NotFound => {
            let mut ancestor = path.parent();
            while let Some(candidate) = ancestor {
                match fs::canonicalize(candidate) {
                    Ok(resolved) => {
                        if !resolved.starts_with(&root) {
                            return Err(path_escape(&root, path));
                        }
                        return Ok(path.to_path_buf());
                    }
                    Err(source) if source.kind() == ErrorKind::NotFound => {
                        ancestor = candidate.parent();
                    }
                    Err(source) => {
                        return Err(PrepareError::ReadManifest {
                            path: candidate.to_path_buf(),
                            source,
                        });
                    }
                }
            }
            Err(path_escape(&root, path))
        }
        Err(source) => Err(PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn path_escape(root: &Path, path: &Path) -> PrepareError {
    trace_error(format!(
        "evidence path '{}' escapes campaign root '{}'",
        path.display(),
        root.display()
    ))
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn trace_source(kind: TraceSourceKind, path: &Path, body: &[u8]) -> TraceSource {
    TraceSource {
        kind,
        path: path.to_path_buf(),
        content_sha256: hex_sha256(body),
    }
}

fn hex_sha256(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn validate_component(label: &str, value: &str) -> Result<(), PrepareError> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(trace_error(format!(
            "{label} must be exactly one normal path component"
        )));
    }
    Ok(())
}

fn trace_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: ERROR_PHASE,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ploke_llm::manager::{RecordedResponse, ResponseIndex};
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::*;
    use crate::{
        cli::prototype1_state::{
            backend::EditSurfaceAdmission,
            edit_surface::{
                harness_request::{HarnessChildBudget, RequestAdmissionBinding},
                surface::SurfacePolicyId,
            },
        },
        loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId},
        replay::tool_loop::{
            FsToolLoopStore, ToolLoopResume, ToolLoopSession, ToolLoopStep, ToolLoopStore,
            WorkspaceState,
        },
    };

    #[test]
    fn index_retains_valid_session_and_reports_invalid_neighbor() {
        let temp = tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(temp.path().to_path_buf());
        let mut session = ToolLoopSession::new("valid-session", "headless-tui", temp.path().into());
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("write valid session");
        let invalid = temp.path().join("invalid-session");
        fs::create_dir_all(&invalid).expect("create invalid session directory");
        fs::write(invalid.join("session.json"), []).expect("write zero-byte manifest");

        let (lanes, issues) = load_lanes(temp.path(), None).expect("load typed lane inventory");

        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].lane_id, "lane-a");
        assert_eq!(lanes[0].sessions.len(), 1);
        assert_eq!(
            lanes[0].sessions[0].session.value.session_id,
            "valid-session"
        );
        assert_eq!(issues.len(), 1);
        let LlmTraceIssue::Invalid {
            session_id,
            source,
            detail,
        } = &issues[0]
        else {
            panic!("zero-byte manifest must be reported as invalid")
        };
        assert_eq!(session_id, "invalid-session");
        assert_eq!(source.content_sha256.len(), 64);
        assert!(detail.contains("parse"));
    }

    #[test]
    fn index_retains_unreadable_resume_and_healthy_sibling() {
        let temp = tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(temp.path().to_path_buf());
        let mut unreadable =
            ToolLoopSession::new("session-unreadable", "headless-tui", temp.path().into());
        unreadable.lane_id = Some("lane-a".to_string());
        store
            .write_session(&unreadable)
            .expect("write unreadable session");
        fs::create_dir_all(store.session_dir("session-unreadable").join("resume.json"))
            .expect("create directory at resume path");
        let mut healthy =
            ToolLoopSession::new("session-healthy", "headless-tui", temp.path().into());
        healthy.lane_id = Some("lane-b".to_string());
        store
            .write_session(&healthy)
            .expect("write healthy session");

        let (lanes, issues) = load_lanes(temp.path(), None).expect("load typed lane inventory");

        assert!(issues.is_empty());
        assert_eq!(lanes.len(), 2);
        let bad = lanes
            .iter()
            .flat_map(|lane| &lane.sessions)
            .find(|summary| summary.session.value.session_id == "session-unreadable")
            .expect("unreadable session remains visible");
        assert!(matches!(
            &bad.resume,
            LlmArtifact::Unreadable { path, detail }
                if path.ends_with("resume.json") && !detail.is_empty()
        ));
        assert!(lanes.iter().any(|lane| {
            lane.sessions
                .iter()
                .any(|summary| summary.session.value.session_id == "session-healthy")
        }));
    }

    #[test]
    fn index_reports_unreadable_manifest_without_hiding_sibling() {
        let temp = tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(temp.path().to_path_buf());
        let mut healthy =
            ToolLoopSession::new("session-healthy", "headless-tui", temp.path().into());
        healthy.lane_id = Some("lane-a".to_string());
        store
            .write_session(&healthy)
            .expect("write healthy session");
        let unreadable = store.session_dir("session-unreadable").join("session.json");
        fs::create_dir_all(&unreadable).expect("create directory at manifest path");

        let (lanes, issues) = load_lanes(temp.path(), None).expect("load typed lane inventory");

        assert_eq!(lanes.len(), 1);
        assert_eq!(
            lanes[0].sessions[0].session.value.session_id,
            "session-healthy"
        );
        assert!(matches!(
            issues.as_slice(),
            [LlmTraceIssue::Unreadable {
                session_id,
                path,
                detail
            }] if session_id == "session-unreadable"
                && path.ends_with("session.json")
                && !detail.is_empty()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn index_rejects_manifest_symlink_escape_without_hiding_sibling() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut healthy =
            ToolLoopSession::new("session-healthy", "headless-tui", temp.path().into());
        healthy.lane_id = Some("lane-a".to_string());
        store
            .write_session(&healthy)
            .expect("write healthy session");
        let escaped = root.join("session-escaped");
        fs::create_dir_all(&escaped).expect("create escaped session directory");
        let outside = temp.path().join("outside-session.json");
        let session = ToolLoopSession::new("session-escaped", "headless-tui", temp.path().into());
        fs::write(
            &outside,
            serde_json::to_vec_pretty(&session).expect("serialize outside session"),
        )
        .expect("write outside session");
        symlink(&outside, escaped.join("session.json")).expect("link outside session");

        let (lanes, issues) = load_lanes(&root, None).expect("load bounded lane inventory");

        assert_eq!(lanes.len(), 1);
        assert_eq!(
            lanes[0].sessions[0].session.value.session_id,
            "session-healthy"
        );
        assert!(matches!(
            issues.as_slice(),
            [LlmTraceIssue::Unreadable {
                session_id,
                path,
                detail
            }] if session_id == "session-escaped"
                && path.ends_with("session.json")
                && detail.contains("escapes campaign root")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn index_rejects_resume_symlink_escape_without_hiding_sibling() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut escaped =
            ToolLoopSession::new("session-escaped", "headless-tui", temp.path().into());
        escaped.lane_id = Some("lane-a".to_string());
        store
            .write_session(&escaped)
            .expect("write escaped session");
        let mut healthy =
            ToolLoopSession::new("session-healthy", "headless-tui", temp.path().into());
        healthy.lane_id = Some("lane-b".to_string());
        store
            .write_session(&healthy)
            .expect("write healthy session");
        let outside = temp.path().join("outside-resume.json");
        let resume = ToolLoopResume::new(
            "session-escaped",
            Uuid::nil().to_string(),
            "parent-a",
            "request-a",
            Vec::new(),
        );
        fs::write(
            &outside,
            serde_json::to_vec_pretty(&resume).expect("serialize outside resume"),
        )
        .expect("write outside resume");
        symlink(
            &outside,
            store.session_dir("session-escaped").join("resume.json"),
        )
        .expect("link outside resume");

        let (lanes, issues) = load_lanes(&root, None).expect("load bounded lane inventory");

        assert!(issues.is_empty());
        assert_eq!(lanes.len(), 2);
        let summary = lanes
            .iter()
            .flat_map(|lane| &lane.sessions)
            .find(|summary| summary.session.value.session_id == "session-escaped")
            .expect("escaped resume session remains visible");
        assert!(matches!(
            &summary.resume,
            LlmArtifact::Unreadable { path, detail }
                if path.ends_with("resume.json")
                    && detail.contains("escapes campaign root")
        ));
        assert!(lanes.iter().any(|lane| {
            lane.sessions
                .iter()
                .any(|summary| summary.session.value.session_id == "session-healthy")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn exact_trace_rejects_step_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let root = campaign_dir.join("prototype1/debug/tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut session = ToolLoopSession::new("session-a", "headless-tui", temp.path().into());
        session.campaign_id = Some("campaign-a".to_string());
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("write session");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let step = ToolLoopStep::new(
            "session-a",
            0,
            Vec::new(),
            content_response(0, assistant),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        let outside = temp.path().join("outside-step.json");
        fs::write(
            &outside,
            serde_json::to_vec_pretty(&step).expect("serialize outside step"),
        )
        .expect("write outside step");
        let step_dir = store.session_dir("session-a").join("steps");
        fs::create_dir_all(&step_dir).expect("create step directory");
        symlink(&outside, step_dir.join("0000.json")).expect("link outside step");
        let mut resume = ToolLoopResume::new(
            "session-a",
            assistant.to_string(),
            "parent-a",
            "request-a",
            Vec::new(),
        );
        resume.next_step = 1;
        store.write_resume(&resume).expect("write resume");
        let scope = LlmScope {
            campaign: CampaignId::from("campaign-a"),
            campaign_dir,
            root,
        };

        let read = load_trace_once(
            &scope,
            LlmTraceCoordinate {
                session_id: "session-a".to_string(),
                step: Some(0),
            },
            SessionVersion::empty(),
            test_epoch(temp.path()),
        )
        .expect("load bounded exact trace");

        assert!(matches!(
            read.snapshot.timeline.as_slice(),
            [LlmStepEntry::Unreadable { step: 0, detail, .. }]
                if detail.contains("escapes campaign root")
        ));
        assert!(matches!(
            read.snapshot.selected,
            Some(LlmArtifact::Unreadable { ref detail, .. })
                if detail.contains("escapes campaign root")
        ));
    }

    #[test]
    fn exact_trace_without_frontier_has_no_selected_checkpoint() {
        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let root = campaign_dir.join("prototype1/debug/tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut session = ToolLoopSession::new("session-a", "headless-tui", temp.path().into());
        session.campaign_id = Some("campaign-a".to_string());
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("write session");
        let scope = LlmScope {
            campaign: CampaignId::from("campaign-a"),
            campaign_dir,
            root,
        };

        let read = load_trace_once(
            &scope,
            LlmTraceCoordinate {
                session_id: "session-a".to_string(),
                step: None,
            },
            SessionVersion::empty(),
            test_epoch(temp.path()),
        )
        .expect("load trace without a frontier");

        assert!(matches!(read.snapshot.resume, LlmArtifact::Missing { .. }));
        assert!(read.snapshot.timeline.is_empty());
        assert!(read.snapshot.selected.is_none());
    }

    #[test]
    fn exact_trace_ignores_invalid_neighbor_and_does_not_infer_running() {
        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let root = campaign_dir.join("prototype1/debug/tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut session = ToolLoopSession::new("session-a", "headless-tui", temp.path().into());
        session.campaign_id = Some("campaign-a".to_string());
        session.lane_id = Some("lane-a".to_string());
        session.status = ToolLoopStatus::Paused;
        store.write_session(&session).expect("write session");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let mut step = ToolLoopStep::new(
            "session-a",
            0,
            Vec::new(),
            content_response(0, assistant),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        step.terminal = false;
        store.write_step(&step).expect("write step");
        let mut resume = ToolLoopResume::new(
            "session-a",
            assistant.to_string(),
            "parent-a",
            "request-a",
            Vec::new(),
        );
        resume.next_step = 1;
        store.write_resume(&resume).expect("write resume");
        let invalid = root.join("invalid-session");
        fs::create_dir_all(&invalid).expect("create invalid neighbor");
        fs::write(invalid.join("session.json"), []).expect("write invalid neighbor");
        let scope = LlmScope {
            campaign: CampaignId::from("campaign-a"),
            campaign_dir,
            root,
        };

        let read = load_trace_once(
            &scope,
            LlmTraceCoordinate {
                session_id: "session-a".to_string(),
                step: None,
            },
            SessionVersion::empty(),
            test_epoch(temp.path()),
        )
        .expect("load exact trace");

        assert_eq!(read.snapshot.session.value.status, ToolLoopStatus::Paused);
        assert_eq!(read.snapshot.timeline.len(), 1);
        assert!(matches!(
            read.snapshot.selected,
            Some(LlmArtifact::Present { .. })
        ));
        let LlmArtifact::Missing { ref path } = read.snapshot.outer else {
            panic!("absent request evidence must remain missing")
        };
        assert!(path.ends_with("lane-a.json"));
        assert!(read.stable().expect("stable observation"));
    }

    #[test]
    fn response_index_mismatch_is_visible_at_the_published_step() {
        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let root = campaign_dir.join("prototype1/debug/tool-loop");
        let store = FsToolLoopStore::new(root.clone());
        let mut session = ToolLoopSession::new("session-a", "headless-tui", temp.path().into());
        session.campaign_id = Some("campaign-a".to_string());
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("write session");
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let step = ToolLoopStep::new(
            "session-a",
            0,
            Vec::new(),
            content_response(1, assistant),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        store.write_step(&step).expect("write step");
        let mut resume = ToolLoopResume::new(
            "session-a",
            assistant.to_string(),
            "parent-a",
            "request-a",
            Vec::new(),
        );
        resume.next_step = 1;
        store.write_resume(&resume).expect("write resume");
        let scope = LlmScope {
            campaign: CampaignId::from("campaign-a"),
            campaign_dir,
            root,
        };

        let read = load_trace_once(
            &scope,
            LlmTraceCoordinate {
                session_id: "session-a".to_string(),
                step: Some(0),
            },
            SessionVersion::empty(),
            test_epoch(temp.path()),
        )
        .expect("load integrity observation");

        let LlmStepEntry::Invalid { detail, .. } = &read.snapshot.timeline[0] else {
            panic!("mismatched provider response index must remain visible")
        };
        assert!(detail.contains("provider response index 1"));
        assert!(matches!(
            read.snapshot.selected,
            Some(LlmArtifact::Invalid { .. })
        ));
    }

    #[test]
    fn huge_frontier_is_rejected() {
        let error = validate_frontier_bound(usize::MAX)
            .expect_err("unbounded resume frontier must fail before filesystem iteration")
            .to_string();

        assert!(error.contains("exceeds observation limit"), "{error}");
    }

    #[test]
    fn bounded_path_rejects_parent_escape() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("campaign");
        fs::create_dir_all(root.join("evidence")).expect("create campaign evidence root");
        let escaped = root
            .join("evidence")
            .join("..")
            .join("..")
            .join("outside")
            .join("request.json");

        let error = bounded_path(&root, &escaped)
            .expect_err("parent traversal must not escape the campaign")
            .to_string();

        assert!(error.contains("escapes campaign root"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn bounded_path_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("campaign");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).expect("create campaign root");
        fs::create_dir_all(&outside).expect("create outside root");
        fs::write(outside.join("request.json"), b"{}").expect("write outside request");
        symlink(&outside, root.join("linked")).expect("link outside root");

        let error = bounded_path(&root, &root.join("linked/request.json"))
            .expect_err("symlink traversal must not escape the campaign")
            .to_string();

        assert!(error.contains("escapes campaign root"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn outer_rejects_headless_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let request_path = campaign_dir.join("prototype1/messages/request/lane-a.json");
        let result_path = campaign_dir.join("prototype1/messages/result/lane-a.json");
        let published = test_request(&campaign_dir, &request_path, &result_path);
        write_request(&request_path, &published);
        let outside = temp.path().join("outside-headless.json");
        fs::write(&outside, b"{}").expect("write outside headless file");
        symlink(&outside, result_path.with_extension("headless-tui.json"))
            .expect("link outside headless file");
        let session = outer_session(&request_path, published.workspace_path());
        let scope = test_scope(&campaign_dir);
        let source = test_source(&campaign_dir);

        let artifact = load_outer(&scope, &session, &source, None).expect("load outer evidence");

        let LlmArtifact::Invalid { detail, .. } = artifact else {
            panic!("escaped headless sidecar must be invalid")
        };
        assert!(detail.contains("escapes campaign root"), "{detail}");
    }

    #[cfg(unix)]
    #[test]
    fn outer_rejects_turn_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let request_path = campaign_dir.join("prototype1/messages/request/lane-a.json");
        let result_path = campaign_dir.join("prototype1/messages/result/lane-a.json");
        let published = test_request(&campaign_dir, &request_path, &result_path);
        write_request(&request_path, &published);
        let outside = temp.path().join("outside-turn");
        fs::create_dir_all(&outside).expect("create outside turn directory");
        fs::write(outside.join("agent-turn-summary.json"), b"{}").expect("write outside turn file");
        symlink(&outside, result_path.with_extension("turn-live"))
            .expect("link outside turn directory");
        let session = outer_session(&request_path, published.workspace_path());
        let scope = test_scope(&campaign_dir);
        let source = test_source(&campaign_dir);

        let artifact = load_outer(&scope, &session, &source, None).expect("load outer evidence");

        let LlmArtifact::Invalid { detail, .. } = artifact else {
            panic!("escaped turn directory must be invalid")
        };
        assert!(detail.contains("escapes campaign root"), "{detail}");
    }

    #[test]
    fn outer_rejects_misaddressed_request() {
        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let declared = campaign_dir.join("prototype1/messages/request/declared.json");
        let actual = campaign_dir.join("prototype1/messages/request/actual.json");
        let result_path = campaign_dir.join("prototype1/messages/result/lane-a.json");
        let published = test_request(&campaign_dir, &declared, &result_path);
        write_request(&actual, &published);
        let session = outer_session(&actual, published.workspace_path());
        let scope = test_scope(&campaign_dir);
        let source = test_source(&campaign_dir);

        let artifact = load_outer(&scope, &session, &source, None).expect("load outer evidence");

        let LlmArtifact::Invalid { detail, .. } = artifact else {
            panic!("misaddressed request must be invalid")
        };
        assert!(detail.contains("declares"), "{detail}");
    }

    #[test]
    fn outer_rejects_workspace_mismatch() {
        let temp = tempdir().expect("tempdir");
        let campaign_dir = temp.path().join("campaign");
        let request_path = campaign_dir.join("prototype1/messages/request/lane-a.json");
        let result_path = campaign_dir.join("prototype1/messages/result/lane-a.json");
        let published = test_request(&campaign_dir, &request_path, &result_path);
        write_request(&request_path, &published);
        let mut session = outer_session(&request_path, published.workspace_path());
        session.workspace = campaign_dir.join("prototype1/workspaces/edit-harness/lane-b");
        let scope = test_scope(&campaign_dir);
        let source = test_source(&campaign_dir);

        let artifact = load_outer(&scope, &session, &source, None).expect("load outer evidence");

        let LlmArtifact::Invalid { detail, .. } = artifact else {
            panic!("workspace mismatch must be invalid")
        };
        assert!(
            detail.contains("does not match debugger workspace"),
            "{detail}"
        );
    }

    fn test_request(
        campaign_dir: &Path,
        request_path: &Path,
        result_path: &Path,
    ) -> PublishedBroadHarnessRequest {
        let artifact = ArtifactId::new("artifact:llm-trace-test");
        let admission = EditSurfaceAdmission::new(
            Coordinate {
                runtime_id: RuntimeId::new(),
                target: OperationTarget::Artifact {
                    artifact_id: artifact,
                },
            },
            SurfacePolicyId::new("policy:llm-trace-test"),
        );
        let binding =
            RequestAdmissionBinding::from_admission(&admission).expect("construct request binding");
        PublishedBroadHarnessRequest::prototype1_workspace(
            "parent-a".to_string(),
            campaign_dir.join("source"),
            HarnessChildBudget {
                min_children: 1,
                max_children: 1,
            },
            &campaign_dir.join("prototype1"),
            request_path.to_path_buf(),
            request_path.with_extension("md"),
            result_path.to_path_buf(),
            binding,
        )
    }

    fn write_request(path: &Path, request: &PublishedBroadHarnessRequest) {
        fs::create_dir_all(path.parent().expect("request parent")).expect("create request parent");
        fs::create_dir_all(
            request
                .submitted_result_path()
                .parent()
                .expect("result parent"),
        )
        .expect("create result parent");
        fs::write(
            path,
            serde_json::to_vec_pretty(request).expect("serialize request"),
        )
        .expect("write request");
    }

    fn outer_session(request_path: &Path, workspace: &Path) -> ToolLoopSession {
        let mut session = ToolLoopSession::new("session-a", "headless-tui", workspace.into());
        session.request_path = Some(request_path.to_path_buf());
        session
    }

    fn test_scope(campaign_dir: &Path) -> LlmScope {
        LlmScope {
            campaign: CampaignId::from("campaign-a"),
            campaign_dir: campaign_dir.to_path_buf(),
            root: campaign_dir.join("prototype1/debug/tool-loop"),
        }
    }

    fn test_source(campaign_dir: &Path) -> TraceSource {
        TraceSource {
            kind: TraceSourceKind::ToolLoopSession,
            path: campaign_dir.join("session.json"),
            content_sha256: "a".repeat(64),
        }
    }

    fn content_response(index: usize, assistant: Uuid) -> RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": format!("chatcmpl-live-trace-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {"role": "assistant", "content": "done"}
            }],
            "created": index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("response json");
        RawFullResponseRecord {
            assistant_message_id: assistant,
            recorded_response: RecordedResponse {
                response_index: ResponseIndex::new(index),
                response,
            },
        }
    }

    fn test_epoch(root: &Path) -> ServerEpoch {
        ServerEpoch {
            protocol_version: 10,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: root.to_path_buf(),
            exe_path: root.join("ploke-eval"),
            exe_modified_unix_ms: None,
            git_head: None,
            active_branch: None,
            source_status_hash: None,
            build_fingerprint: String::new(),
        }
    }
}
