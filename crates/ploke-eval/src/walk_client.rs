//! Public sibling-client helpers for the Prototype 1 walk server.
//!
//! This is a UI-facing facade over the private `loop walk` protocol. Mutations
//! are admitted by the server from a freshly observed epoch and durable session
//! version; this client does not own transition semantics and it does not make
//! database mirrors authoritative.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

pub use ploke_protocol::{LocalAnalysisAssessment, LocalAnalysisTargetKind, ToolCallNeighborhood};
pub use ploke_records::{
    identity::ParentIdentityRecord,
    ids::RuntimeId,
    protocol::{ArtifactBody, ArtifactFile},
    run_profile::{RunMode, RunProfileCommitmentRecord, RunProfileRecord},
    run_record::{ToolExecutionRecord, ToolResult},
};

use crate::{
    cli::prototype1_state::{
        driver::control::StepAdmission,
        eval_store::prototype1_eval_store_db_path,
        identity, setup_admission,
        walk::{endpoint, ipc, paths},
    },
    layout::campaigns_dir,
    spec::PrepareError,
};

pub use crate::cli::{
    Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
    Prototype1StateWalkLlmStepSource,
};
pub use ploke_records::ids::{CampaignId, GitCommit, InstanceId};

pub use crate::cli::prototype1_state::{
    driver::control::RecoveryDirective,
    edge::ControlEdge,
    event::{ContentHash, TransitionId},
    session::{Cursor, SessionId},
    typestate::{RuntimeAxis, RuntimeAxisDelta},
    walk::{
        audit::{
            AuditSummary, AuditVerdict, CampaignAudit, CampaignSource, DatabaseAudit,
            DatabaseStatus, DocumentAudit, DocumentExpectation, DocumentRole, DocumentStatus,
            ExpectedAt, ExpectedPersistence, ExpectedStatus, PersistenceItemAudit, PersistenceSide,
            PersistenceStatus, RelationCount, TransitionAudit, TransitionChecklist,
            WalkAuditReport,
        },
        config::{
            CampaignConfig, ConfigAdmission, ConfigIdentity, DerivationRule, EffectiveControl,
            ProfileConfig, ProfileField, ProviderSelection, SetupArtifactHashes, SourcedValue,
            ValueSource, WalkConfigSnapshot,
        },
        epoch::ServerEpoch,
        llm_trace::{
            LlmArtifact, LlmHeadlessTerminal, LlmLane, LlmOuterEvidence, LlmResume, LlmSession,
            LlmSessionStatus, LlmSessionSummary, LlmStepDetail, LlmStepEntry, LlmStepOutcome,
            LlmStepSummary, LlmToolResult, LlmTraceAuthority, LlmTraceCoordinate, LlmTraceIndex,
            LlmTraceIssue, LlmTraceSnapshot, LlmWorkspaceState,
        },
        phase::WalkPhase,
        protocol::{
            MutationGuard, OperationId, SessionVersion, WalkAction, WalkActionKind,
            WalkAttemptIntent, WalkAttemptReceipt, WalkAttemptResult, WalkAuthority, WalkBlocker,
            WalkBlockerCode, WalkCursor, WalkCursorEvidence, WalkDeltaSnapshot, WalkDeltaState,
            WalkEdgeDelta, WalkEndpoint, WalkEpochReceipt, WalkErrorCode, WalkEventProjection,
            WalkHandoffAcceptance, WalkJobKind, WalkJobResolutionKind, WalkJobResolutionReceipt,
            WalkJobSnapshot, WalkJobStatus, WalkOkKind, WalkOkPayload, WalkPosition,
            WalkPredecessorAttempt, WalkQuerySnapshot, WalkReadyCommit, WalkReadyReceipt,
            WalkRecoveryResolution, WalkRequest, WalkRequestBody, WalkResponse, WalkRunMode,
            WalkSessionAbandonment, WalkSessionDamage, WalkSessionEvent, WalkSessionEventKind,
            WalkSessionHistory, WalkSessionOrigin, WalkSessionSnapshot, WalkStartConfig,
            WalkTransitionReceipt,
        },
        query::{DbQueryResult, DbQueryRow, ReadRevision},
        trace::{
            CompletedEvaluationTrace, EvaluationRunCoordinate, EvaluationRunEntry,
            EvaluationTraceIndex, EvaluationTraceSnapshot, EvaluationTraceState, ModelExchange,
            TraceAuthority, TraceEvidence, TraceSource, TraceSourceKind,
        },
    },
};

pub use crate::inner::RunRegistration;

/// Canonical setup-derived boundary used when Start omits an explicit target.
pub const DEFAULT_START_PHASE: WalkPhase = WalkPhase::R3;

/// One exact, version-bound Step offer displayed by a sibling client.
///
/// A Step command admits every possible outcome of one transition rather than
/// choosing one runtime branch. This carrier is deliberately not serialized:
/// the server remains authoritative for the action rows and durable attempt
/// intent, while the client retains the exact offer it showed the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisedStep {
    version: SessionVersion,
    outcomes: BTreeSet<ControlEdge>,
    admission: StepAdmission,
}

impl AdvertisedStep {
    /// Validate and retain one Step offer from a typed status snapshot.
    pub fn from_snapshot(
        snapshot: &WalkSessionSnapshot,
        allow_live_api: bool,
        allow_git_changes: bool,
    ) -> Result<Self, PrepareError> {
        if !matches!(snapshot.position, WalkPosition::Session { .. })
            || !snapshot.controller_attached
            || snapshot.authority != WalkAuthority::Active
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "advertised Step requires an attached Active durable Session".to_string(),
            });
        }

        let phase = snapshot.phase();
        let mut seen = BTreeSet::new();
        for action in snapshot
            .actions
            .iter()
            .filter(|action| action.kind == WalkActionKind::Step)
        {
            let Some(edge) = action.edge else {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("Step action at {phase} omitted its typed edge"),
                });
            };
            if !seen.insert(edge) {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("Step action at {phase} duplicated outcome {}", edge.id()),
                });
            }
            if edge.from() != phase {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "Step action outcome {} starts at {}, but status is at {phase}",
                        edge.id(),
                        edge.from()
                    ),
                });
            }
            if action.target != Some(edge.to()) {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "Step action outcome {} targets {:?}, expected {}",
                        edge.id(),
                        action.target,
                        edge.to()
                    ),
                });
            }
            if action.requires_live_api != edge.requires_live()
                || action.requires_git_changes != edge.requires_checkout()
            {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "Step action outcome {} has capability flags inconsistent with the control graph",
                        edge.id()
                    ),
                });
            }
            if !action.enabled || action.blocker.is_some() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "Step action outcome {} is inconsistent with Active authority",
                        edge.id()
                    ),
                });
            }
        }

        let expected = ControlEdge::ALL
            .into_iter()
            .filter(|edge| edge.from() == phase)
            .collect::<BTreeSet<_>>();
        if seen != expected {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "Step actions at {phase} do not match the complete control outcomes: observed {seen:?}, expected {expected:?}"
                ),
            });
        }

        let outcomes = seen
            .iter()
            .copied()
            .filter(|edge| allow_git_changes || !edge.requires_checkout())
            .collect::<BTreeSet<_>>();
        if outcomes.is_empty() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!("walk status at {phase} advertised no admitted Step outcome"),
            });
        }
        if !allow_live_api && outcomes.iter().copied().any(ControlEdge::requires_live) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "Step at {phase} requires the explicit live provider grant for outcomes {outcomes:?}"
                ),
            });
        }

        Ok(Self {
            version: snapshot.version(),
            outcomes,
            admission: StepAdmission::new(allow_live_api, allow_git_changes),
        })
    }

    pub fn version(&self) -> &SessionVersion {
        &self.version
    }

    pub fn outcomes(&self) -> &BTreeSet<ControlEdge> {
        &self.outcomes
    }

    pub const fn allows_live_api(&self) -> bool {
        self.admission.live
    }

    pub const fn allows_git_changes(&self) -> bool {
        self.admission.checkout
    }

    /// Whether any admitted outcome transfers authority to a successor endpoint.
    pub fn requires_endpoint_following(&self) -> bool {
        self.outcomes.contains(&ControlEdge::R12ToR13b)
    }

    /// Validate the single realized edge of a succeeded advertised Step.
    pub fn validate_terminal<'a>(
        &self,
        job: &'a WalkJobSnapshot,
    ) -> Result<&'a WalkTransitionReceipt, PrepareError> {
        self.validate_job(job)?;
        if job.status != WalkJobStatus::Succeeded {
            return Err(protocol_error(format!(
                "advertised Step operation {} ended {:?}, expected Succeeded",
                job.operation_id, job.status
            )));
        }
        let receipt = job.receipt.as_ref().ok_or_else(|| {
            protocol_error(format!(
                "succeeded advertised Step operation {} omitted its typed receipt",
                job.operation_id
            ))
        })?;
        let [edge] = receipt.edges.as_slice() else {
            return Err(protocol_error(format!(
                "advertised Step receipt contained {} edges, expected exactly one",
                receipt.edges.len()
            )));
        };
        if !self.outcomes.contains(edge) {
            return Err(protocol_error(format!(
                "advertised Step realized outcome {}, outside retained outcomes {:?}",
                edge.id(),
                self.outcomes
            )));
        }
        if receipt.phase_before != self.version.phase()
            || edge.from() != receipt.phase_before
            || edge.to() != receipt.phase_after
        {
            return Err(protocol_error(format!(
                "advertised Step receipt edge {} does not match {} -> {} from retained phase {}",
                edge.id(),
                receipt.phase_before,
                receipt.phase_after,
                self.version.phase()
            )));
        }
        if job.phase_before != receipt.phase_before || job.phase_after != Some(receipt.phase_after)
        {
            return Err(protocol_error(
                "advertised Step terminal job phase envelope disagrees with its receipt"
                    .to_string(),
            ));
        }
        if receipt.version.phase() != receipt.phase_after {
            return Err(protocol_error(format!(
                "advertised Step receipt version is at {}, expected realized target {}",
                receipt.version.phase(),
                receipt.phase_after
            )));
        }
        let Some(session) = self.version.session_id() else {
            return Err(protocol_error(
                "advertised Step retained an empty session identity".to_string(),
            ));
        };
        if receipt.version.session_id() != Some(session) {
            return Err(protocol_error(format!(
                "advertised Step receipt session {:?} does not match retained session {session}",
                receipt.version.session_id()
            )));
        }
        if receipt.version.journal_revision() <= self.version.journal_revision() {
            return Err(protocol_error(format!(
                "advertised Step receipt revision {} did not advance beyond retained revision {}",
                receipt.version.journal_revision(),
                self.version.journal_revision()
            )));
        }
        Ok(receipt)
    }

    fn validate_job(&self, job: &WalkJobSnapshot) -> Result<(), PrepareError> {
        if job.command != WalkJobKind::Step
            || job.expected != self.version
            || job.phase_before != self.version.phase()
            || job.target_phase.is_some()
            || job.watch != Some(false)
            || job.allow_live_api != Some(self.admission.live)
            || job.allow_git_changes != Some(self.admission.checkout)
        {
            return Err(protocol_error(format!(
                "advertised Step job envelope did not match the retained offer at {}",
                self.version.phase()
            )));
        }
        Ok(())
    }
}

/// Closed evidence-only projections over one immutable owner-DB snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WalkEvidenceQuery {
    /// Complete relation inventory reported by the owner database.
    Relations,
    /// Curated counts for core operator and typed-trace relations.
    ///
    /// This is intentionally not a complete schema inventory; use `Relations`
    /// to discover every installed relation.
    Counts,
    ConfigEvidence,
    Lineage,
    Progress,
    HandoffEvidence,
}

impl WalkEvidenceQuery {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relations => "relations",
            Self::Counts => "counts",
            Self::ConfigEvidence => "config-evidence",
            Self::Lineage => "lineage",
            Self::Progress => "progress",
            Self::HandoffEvidence => "handoff-evidence",
        }
    }
}

/// Resolved local walk service endpoint for one parent checkout.
#[derive(Debug, Clone)]
pub struct WalkClient {
    repo_root: PathBuf,
    socket: PathBuf,
    follow_endpoint: bool,
}

/// One Prototype 1 run candidate discovered under the local ploke-eval home.
///
/// This is a UI adapter projection, not a persisted record. Authoritative run
/// semantics still live in the campaign files, owner DB, and walk server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkRunEntry {
    pub campaign_id: String,
    pub campaign_dir: PathBuf,
    pub prototype1_root: PathBuf,
    pub worktree_root: Option<PathBuf>,
    pub owner_db_path: PathBuf,
    pub modified_unix_ms: Option<u64>,
    pub has_manifest: bool,
    pub has_closure_state: bool,
    pub has_owner_db: bool,
    pub has_parent_identity: bool,
}

/// Structured inventory for rendering the phase rail without duplicating phase
/// semantics in UI crates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInventory {
    pub phases: Vec<PhaseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInfo {
    pub phase: WalkPhase,
    pub id: String,
    pub label: String,
    pub typestate: String,
    pub next: Vec<NextStepInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStepInfo {
    pub edge: ControlEdge,
    pub phase: WalkPhase,
    pub phase_id: String,
    pub detail: String,
    pub requires_watch: bool,
    pub requires_live_api: bool,
    pub requires_git_changes: bool,
}

impl WalkClient {
    /// Discover Prototype 1 campaign roots under `ploke_eval_home()/campaigns`.
    pub fn discover_runs() -> Result<Vec<WalkRunEntry>, PrepareError> {
        discover_walk_runs()
    }

    /// Resolve a client endpoint for a discovered run.
    ///
    /// The UI starts from a selected campaign instead of the operator's
    /// current directory, but socket selection still needs to honor `walk use`
    /// so the CLI and UI do not disagree about a live server. An explicit UI
    /// socket override wins; otherwise the saved context socket is used only
    /// when it names the same parent checkout as the selected run.
    pub fn resolve_for_run(
        run: &WalkRunEntry,
        socket: Option<&Path>,
    ) -> Result<Self, PrepareError> {
        let Some(repo_root) = run.worktree_root.as_deref() else {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "run '{}' has no exact receipt-backed local checkout",
                    run.campaign_id
                ),
            });
        };
        let repo_root = paths::resolve_repo_root(Some(repo_root))?;
        let context = if socket.is_none() {
            paths::load_context()?
        } else {
            None
        };
        let socket_override = socket.or_else(|| {
            context
                .as_ref()
                .filter(|context| context.repo_root == repo_root)
                .and_then(|context| context.socket.as_deref())
        });
        let socket_path = paths::socket_path(&repo_root, socket_override)?;
        Ok(Self {
            repo_root,
            socket: socket_path,
            follow_endpoint: socket.is_none(),
        })
    }

    /// Resolve a client endpoint using the same context/socket rules as
    /// `ploke-eval loop walk`.
    pub fn resolve(repo_root: Option<&Path>, socket: Option<&Path>) -> Result<Self, PrepareError> {
        let repo_root = paths::resolve_repo_root(repo_root)?;
        let context = if socket.is_none() {
            paths::load_context()?
        } else {
            None
        };
        let fallback = socket.or_else(|| {
            context
                .as_ref()
                .filter(|context| context.repo_root == repo_root)
                .and_then(|context| context.socket.as_deref())
        });
        let socket_path = paths::socket_path(&repo_root, fallback)?;
        Ok(Self {
            repo_root,
            socket: socket_path,
            follow_endpoint: socket.is_none(),
        })
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Whether this client resolves the authoritative successor endpoint.
    ///
    /// Explicit socket overrides are pinned and therefore return `false`.
    pub const fn follows_endpoint(&self) -> bool {
        self.follow_endpoint
    }

    /// Resolve the currently authoritative endpoint for an unpinned client.
    pub fn resolved_socket(&self) -> Result<PathBuf, PrepareError> {
        self.endpoint_observation().map(|(socket, _)| socket)
    }

    fn endpoint_observation(
        &self,
    ) -> Result<(PathBuf, Option<endpoint::ServerEndpoint>), PrepareError> {
        if self.follow_endpoint
            && let Some(active) = endpoint::load(&self.repo_root)?
            && active.repo_root() == self.repo_root
            && active.owns_socket()
        {
            return Ok((active.socket().to_path_buf(), Some(active)));
        }
        Ok((self.socket.clone(), None))
    }

    /// Probe the server without mutating state. `Ok(None)` means no server is
    /// currently listening at the resolved socket.
    pub async fn health(&self) -> Result<Option<WalkResponse>, PrepareError> {
        Ok(self
            .health_observation()
            .await?
            .map(|(_, response)| response))
    }

    pub(crate) async fn health_observation(
        &self,
    ) -> Result<Option<(PathBuf, WalkResponse)>, PrepareError> {
        let observation = self.exchange_read_only(WalkRequestBody::Health).await?;
        if let Some((_, response)) = observation.as_ref() {
            ensure_current_protocol(response, &self.repo_root)?;
        }
        Ok(observation)
    }

    /// Read the current in-memory walk state from the server.
    pub async fn show(&self) -> Result<WalkResponse, PrepareError> {
        self.send_read_only(WalkRequestBody::Show).await
    }

    /// Attach to the setup-derived controller session through one guarded operation.
    ///
    /// The caller owns `operation` and must retain it for polling or an explicit
    /// retry after an ambiguous transport failure.
    pub async fn start(
        &self,
        mut config: WalkStartConfig,
        until: WalkPhase,
        allow_live_api: bool,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        if let Some(repo_root) = config.repo_root.as_deref()
            && paths::resolve_repo_root(Some(repo_root))? != self.repo_root
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk start repository '{}' does not match client repository '{}'",
                    repo_root.display(),
                    self.repo_root.display()
                ),
            });
        }
        config.repo_root = Some(self.repo_root.clone());
        let response = self
            .send_bound(|status, epoch| {
                let guard = mutation_guard(status, epoch, operation)?;
                Ok(WalkRequestBody::Start {
                    guard,
                    config: config.clone(),
                    until,
                    allow_live_api,
                })
            })
            .await?;
        validate_job_response(&response, operation, Some(WalkJobKind::Start), "start")?;
        Ok(response)
    }

    /// Attach only when a fresh status advertises the requested Start target.
    ///
    /// This preserves [`Self::start`] for callers that intentionally own generic
    /// `until` semantics while allowing rendered operator actions to fail closed
    /// when their advertised target becomes stale before submission.
    pub async fn start_advertised(
        &self,
        mut config: WalkStartConfig,
        until: WalkPhase,
        allow_live_api: bool,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        if let Some(repo_root) = config.repo_root.as_deref()
            && paths::resolve_repo_root(Some(repo_root))? != self.repo_root
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk start repository '{}' does not match client repository '{}'",
                    repo_root.display(),
                    self.repo_root.display()
                ),
            });
        }
        config.repo_root = Some(self.repo_root.clone());
        let response = self
            .send_bound(|status, epoch| {
                let WalkResponse::Status { snapshot, .. } = status else {
                    return Err(protocol_error(
                        "advertised Start requires a typed status snapshot".to_string(),
                    ));
                };
                let matching = snapshot
                    .actions
                    .iter()
                    .filter(|action| {
                        action.kind == WalkActionKind::Start
                            && action.enabled
                            && action.target == Some(until)
                    })
                    .count();
                if matching != 1 {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "walk status at {} advertised {matching} enabled copies of Start targeting {until}; expected exactly one",
                            snapshot.phase(),
                        ),
                    });
                }
                let guard = mutation_guard(status, epoch, operation)?;
                Ok(WalkRequestBody::Start {
                    guard,
                    config: config.clone(),
                    until,
                    allow_live_api,
                })
            })
            .await?;
        validate_job_response(
            &response,
            operation,
            Some(WalkJobKind::Start),
            "advertised start",
        )?;
        Ok(response)
    }

    /// Submit one supervised step operation, optionally targeting a later phase.
    ///
    /// The caller owns `operation` and must retain it for polling or an explicit
    /// retry after an ambiguous transport failure.
    pub async fn step(
        &self,
        until: Option<WalkPhase>,
        watch: bool,
        allow_live_api: bool,
        allow_git_changes: bool,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        let response = self
            .send_bound(|status, epoch| {
                let guard = mutation_guard(status, epoch, operation)?;
                Ok(WalkRequestBody::Step {
                    guard,
                    until,
                    watch,
                    allow_live_api,
                    allow_git_changes,
                })
            })
            .await?;
        validate_job_response(&response, operation, Some(WalkJobKind::Step), "step")?;
        Ok(response)
    }

    /// Submit one exact Step command retained from a displayed status.
    ///
    /// The complete branch outcome set and session version are re-derived from
    /// the fresh status that guards the mutation. A changed offer is rejected
    /// before a Step request is framed.
    pub async fn step_advertised(
        &self,
        advertised: &AdvertisedStep,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        let response = self
            .send_bound(|status, epoch| {
                let WalkResponse::Status { snapshot, .. } = status else {
                    return Err(protocol_error(
                        "advertised Step requires a typed status snapshot".to_string(),
                    ));
                };
                let fresh = AdvertisedStep::from_snapshot(
                    snapshot,
                    advertised.allows_live_api(),
                    advertised.allows_git_changes(),
                )?;
                if &fresh != advertised {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "advertised Step changed before submission: displayed version {:?} with outcomes {:?}, fresh version {:?} with outcomes {:?}",
                            advertised.version(),
                            advertised.outcomes(),
                            fresh.version(),
                            fresh.outcomes()
                        ),
                    });
                }
                let guard = mutation_guard(status, epoch, operation)?;
                Ok(WalkRequestBody::Step {
                    guard,
                    until: None,
                    watch: false,
                    allow_live_api: advertised.allows_live_api(),
                    allow_git_changes: advertised.allows_git_changes(),
                })
            })
            .await?;
        validate_job_response(
            &response,
            operation,
            Some(WalkJobKind::Step),
            "advertised step",
        )?;
        validate_advertised_response(&response, advertised)?;
        Ok(response)
    }

    /// Submit exactly one server-advertised transition edge.
    ///
    /// Unlike [`Self::step`], this operation never treats a target phase as an
    /// `advance_until` request. The expected edge is checked against the same
    /// fresh status snapshot whose exact version guards the mutation.
    pub async fn step_edge(
        &self,
        edge: ControlEdge,
        allow_live_api: bool,
        allow_git_changes: bool,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        let response = self
            .send_bound(|status, epoch| {
                let WalkResponse::Status { snapshot, .. } = status else {
                    return Err(protocol_error(
                        "edge-aware Step requires a typed status snapshot".to_string(),
                    ));
                };
                let advertised = AdvertisedStep::from_snapshot(
                    snapshot,
                    allow_live_api,
                    allow_git_changes,
                )?;
                if advertised.outcomes().len() != 1 || !advertised.outcomes().contains(&edge) {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "walk status at {} advertised Step outcomes {:?}; singleton helper requires exactly {}",
                            snapshot.phase(),
                            advertised.outcomes(),
                            edge.id()
                        ),
                    });
                }
                let guard = mutation_guard(status, epoch, operation)?;
                Ok(WalkRequestBody::Step {
                    guard,
                    until: None,
                    watch: false,
                    allow_live_api,
                    allow_git_changes,
                })
            })
            .await?;
        validate_job_response(&response, operation, Some(WalkJobKind::Step), "edge step")?;
        Ok(response)
    }

    /// Inspect one exact supervised operation through the current endpoint.
    pub async fn operation_status(
        &self,
        operation: OperationId,
    ) -> Result<WalkResponse, PrepareError> {
        let response = self
            .send_read_only(WalkRequestBody::OperationStatus { operation })
            .await?;
        validate_job_response(&response, operation, None, "operation status")?;
        Ok(response)
    }

    /// Shut down the authoritative endpoint only when it has no blocking job.
    ///
    /// This is server lifecycle control, not loop cancellation or pause.
    pub async fn stop_idle_server(&self) -> Result<WalkResponse, PrepareError> {
        let response = self.send_stop().await?;
        validate_stop_response(&response)?;
        Ok(response)
    }

    /// Read the admitted campaign, run profile, and effective controller configuration.
    pub async fn config(&self) -> Result<WalkConfigSnapshot, PrepareError> {
        match self.send_read_only(WalkRequestBody::Config).await? {
            WalkResponse::Config { config, .. } => Ok(config),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_config",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_config",
                detail: format!(
                    "walk server returned {:?} instead of a configuration response",
                    response.phase()
                ),
            }),
        }
    }

    /// Read the ordered durable controller-session journal projection.
    pub async fn session_history(&self) -> Result<WalkSessionHistory, PrepareError> {
        match self.send_read_only(WalkRequestBody::SessionHistory).await? {
            WalkResponse::History { history } => Ok(history),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_session_history",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_session_history",
                detail: format!(
                    "walk server returned {:?} instead of a session-history response",
                    response.phase()
                ),
            }),
        }
    }

    /// Read the typed delta for the most recent successful walk advance.
    pub async fn transition_delta(&self) -> Result<WalkDeltaSnapshot, PrepareError> {
        match self
            .send_read_only(WalkRequestBody::ShowDelta {
                verbose: false,
                color: false,
            })
            .await?
        {
            WalkResponse::Delta { snapshot, .. } => Ok(snapshot),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_transition_delta",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_transition_delta",
                detail: format!(
                    "walk server returned {:?} instead of a transition-delta response",
                    response.phase()
                ),
            }),
        }
    }

    /// List completed registered evaluation runs within the admitted campaign root.
    pub async fn evaluation_trace_index(&self) -> Result<EvaluationTraceIndex, PrepareError> {
        match self
            .send_read_only(WalkRequestBody::EvaluationTraceIndex)
            .await?
        {
            WalkResponse::EvaluationTraceIndex { index } => Ok(index),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_evaluation_trace_index",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_evaluation_trace_index",
                detail: format!(
                    "walk server returned {:?} instead of an evaluation-trace index",
                    response.phase()
                ),
            }),
        }
    }

    /// Load one exact registered evaluation run through the canonical trace reader.
    pub async fn evaluation_trace(
        &self,
        coordinate: EvaluationRunCoordinate,
    ) -> Result<EvaluationTraceSnapshot, PrepareError> {
        match self
            .send_read_only(WalkRequestBody::EvaluationTrace { coordinate })
            .await?
        {
            WalkResponse::EvaluationTrace { snapshot } => Ok(snapshot),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_evaluation_trace",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_evaluation_trace",
                detail: format!(
                    "walk server returned {:?} instead of an evaluation trace",
                    response.phase()
                ),
            }),
        }
    }

    /// List every persisted LLM debugger session without collapsing retries.
    pub async fn llm_trace_index(&self) -> Result<LlmTraceIndex, PrepareError> {
        match self.send_read_only(WalkRequestBody::LlmTraceIndex).await? {
            WalkResponse::LlmTraceIndex { index } => Ok(index),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_llm_trace_index",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_llm_trace_index",
                detail: format!(
                    "walk server returned {:?} instead of an LLM trace index",
                    response.phase()
                ),
            }),
        }
    }

    /// Load one exact LLM debugger session and optional published response step.
    pub async fn llm_trace(
        &self,
        coordinate: LlmTraceCoordinate,
    ) -> Result<LlmTraceSnapshot, PrepareError> {
        let requested = coordinate.clone();
        match self
            .send_read_only(WalkRequestBody::LlmTrace { coordinate })
            .await?
        {
            WalkResponse::LlmTrace { snapshot }
                if snapshot.coordinate == requested
                    && snapshot.session.value.session_id == requested.session_id =>
            {
                Ok(snapshot)
            }
            WalkResponse::LlmTrace { snapshot } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_llm_trace",
                detail: format!(
                    "walk LLM trace response for session '{}' step {:?} disagrees with requested session '{}' step {:?}",
                    snapshot.coordinate.session_id,
                    snapshot.coordinate.step,
                    requested.session_id,
                    requested.step
                ),
            }),
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_llm_trace",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_llm_trace",
                detail: format!(
                    "walk server returned {:?} instead of an LLM trace observation",
                    response.phase()
                ),
            }),
        }
    }

    /// Run an immutable query through the walk service against one exact owner snapshot.
    pub async fn query_db(
        &self,
        campaign: Option<&CampaignId>,
        script: &str,
    ) -> Result<WalkQuerySnapshot, PrepareError> {
        let expected = self.query_campaign(campaign)?;
        let requested = campaign.cloned();
        match self
            .send_read_only(WalkRequestBody::DbQuery {
                campaign: requested.clone(),
                script: script.to_string(),
            })
            .await?
        {
            WalkResponse::Query { query } => {
                self.validate_query(query, &expected, None, Some(script))
            }
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_db_query",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_db_query",
                detail: format!(
                    "walk server returned {:?} instead of a query response",
                    response.phase()
                ),
            }),
        }
    }

    /// Run one closed evidence projection against an exact owner-DB snapshot.
    ///
    /// Named projections are durable evidence views. They do not report live
    /// controller authority; use [`Self::health`] for server and job state.
    pub async fn query_evidence(
        &self,
        campaign: Option<&CampaignId>,
        view: WalkEvidenceQuery,
    ) -> Result<WalkQuerySnapshot, PrepareError> {
        let campaign = self.query_campaign(campaign)?;
        match self
            .send_read_only(WalkRequestBody::EvidenceQuery {
                campaign: campaign.clone(),
                view,
            })
            .await?
        {
            WalkResponse::Query { query } => {
                self.validate_query(query, &campaign, Some(view), None)
            }
            WalkResponse::Error { code, detail, .. } => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_db_query",
                detail: format!("{code}: {detail}"),
            }),
            response => Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_db_query",
                detail: format!(
                    "walk server returned {:?} instead of a query response",
                    response.phase()
                ),
            }),
        }
    }

    fn validate_query(
        &self,
        query: WalkQuerySnapshot,
        campaign: &CampaignId,
        view: Option<WalkEvidenceQuery>,
        raw_script: Option<&str>,
    ) -> Result<WalkQuerySnapshot, PrepareError> {
        if query.result.repo_root != self.repo_root {
            return Err(protocol_error(format!(
                "walk db query returned repository '{}', expected client repository '{}'",
                query.result.repo_root.display(),
                self.repo_root.display()
            )));
        }
        if &query.result.campaign_id != campaign {
            return Err(protocol_error(format!(
                "walk db query returned campaign '{}', expected requested campaign '{}'",
                query.result.campaign_id, campaign
            )));
        }
        if query.result.view != view {
            return Err(protocol_error(format!(
                "walk db query returned view {:?}, expected requested view {:?}",
                query.result.view, view
            )));
        }
        if let Some(script) = raw_script
            && query.result.script != script
        {
            return Err(protocol_error(
                "walk db query returned a script other than the exact raw request".to_string(),
            ));
        }
        Ok(query)
    }

    fn query_campaign(&self, campaign: Option<&CampaignId>) -> Result<CampaignId, PrepareError> {
        match campaign {
            Some(campaign) => Ok(campaign.clone()),
            None => identity::load_parent_identity(&self.repo_root)
                .map(|identity| identity.campaign_id().clone()),
        }
    }

    pub(crate) async fn send_read_only(
        &self,
        body: WalkRequestBody,
    ) -> Result<WalkResponse, PrepareError> {
        let requires_current = requires_current_protocol(&body);
        if requires_current {
            let response = self
                .exchange_read_only(WalkRequestBody::Health)
                .await?
                .map(|(_, response)| response)
                .ok_or_else(|| self.offline_error())?;
            ensure_current_protocol(&response, &self.repo_root)?;
        }
        let response = self
            .exchange_read_only(body)
            .await?
            .map(|(_, response)| response)
            .ok_or_else(|| self.offline_error())?;
        if requires_current {
            ensure_current_protocol(&response, &self.repo_root)?;
        }
        Ok(response)
    }

    fn offline_error(&self) -> PrepareError {
        PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_connect",
            detail: format!(
                "walk server is not listening at '{}'",
                self.resolved_socket()
                    .unwrap_or_else(|_| self.socket.clone())
                    .display()
            ),
        }
    }

    async fn exchange_read_only(
        &self,
        body: WalkRequestBody,
    ) -> Result<Option<(PathBuf, WalkResponse)>, PrepareError> {
        for exchange in 0..2 {
            let Some((socket, endpoint, mut stream)) = self.connect_optional().await? else {
                return Ok(None);
            };
            let request = WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: None,
                body: body.clone(),
            };
            let result = match ipc::send(&mut stream, &request).await {
                Ok(()) => ipc::recv(&mut stream).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(response) => {
                    validate_response_shape(&response)?;
                    return Ok(Some((socket, response)));
                }
                Err(_)
                    if exchange == 0
                        && self.follow_endpoint
                        && self.wait_for_successor(&socket, endpoint.as_ref()).await? =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("read-only exchange either returns or retries once")
    }

    async fn send_bound(
        &self,
        body: impl Fn(&WalkResponse, &ServerEpoch) -> Result<WalkRequestBody, PrepareError>,
    ) -> Result<WalkResponse, PrepareError> {
        for attempt in 0..2 {
            let Some((socket, endpoint, status)) = self.probe_bound().await? else {
                return Err(self.offline_error());
            };
            if self.endpoint_moved(&socket, endpoint.as_ref())? {
                if attempt == 0 {
                    continue;
                }
                return Err(endpoint_changed_error(&socket));
            }

            let mut stream = match UnixStream::connect(&socket).await {
                Ok(stream) => stream,
                Err(source)
                    if attempt == 0
                        && self.follow_endpoint
                        && matches!(
                            source.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        )
                        && self.wait_for_successor(&socket, endpoint.as_ref()).await? =>
                {
                    continue;
                }
                Err(source) => return Err(connect_error(&socket, source)),
            };
            if self.endpoint_moved(&socket, endpoint.as_ref())? {
                if attempt == 0 {
                    continue;
                }
                return Err(endpoint_changed_error(&socket));
            }

            let epoch = ServerEpoch::capture(&self.repo_root)?;
            let request = WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: Some(epoch.clone()),
                body: body(&status, &epoch)?,
            };
            ipc::send(&mut stream, &request).await?;
            let response = ipc::recv(&mut stream).await?;
            validate_response_shape(&response)?;
            ensure_current_protocol(&response, &self.repo_root)?;
            return Ok(response);
        }
        unreachable!("bound exchange either returns or retries once")
    }

    async fn probe_bound(
        &self,
    ) -> Result<Option<(PathBuf, Option<endpoint::ServerEndpoint>, WalkResponse)>, PrepareError>
    {
        for exchange in 0..2 {
            let Some((socket, endpoint, mut stream)) = self.connect_optional().await? else {
                return Ok(None);
            };
            let request = WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: None,
                body: WalkRequestBody::Health,
            };
            let result = match ipc::send(&mut stream, &request).await {
                Ok(()) => ipc::recv(&mut stream).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(response) => {
                    validate_response_shape(&response)?;
                    ensure_current_protocol(&response, &self.repo_root)?;
                    if !matches!(&response, WalkResponse::Status { .. }) {
                        return Err(protocol_error(format!(
                            "walk mutation requires a typed health/status version; endpoint returned {response:?}"
                        )));
                    }
                    return Ok(Some((socket, endpoint, response)));
                }
                Err(_)
                    if exchange == 0
                        && self.follow_endpoint
                        && self.wait_for_successor(&socket, endpoint.as_ref()).await? =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("bound probe either returns or retries once")
    }

    async fn send_stop(&self) -> Result<WalkResponse, PrepareError> {
        for attempt in 0..2 {
            let Some((socket, endpoint, status)) = self.probe_stop().await? else {
                return Err(self.offline_error());
            };
            ensure_response_repo(&status, &self.repo_root)?;
            if self.endpoint_moved(&socket, endpoint.as_ref())? {
                if attempt == 0 {
                    continue;
                }
                return Err(endpoint_changed_error(&socket));
            }

            let mut stream = match UnixStream::connect(&socket).await {
                Ok(stream) => stream,
                Err(source)
                    if attempt == 0
                        && self.follow_endpoint
                        && matches!(
                            source.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        )
                        && self.wait_for_successor(&socket, endpoint.as_ref()).await? =>
                {
                    continue;
                }
                Err(source) => return Err(connect_error(&socket, source)),
            };
            if self.endpoint_moved(&socket, endpoint.as_ref())? {
                if attempt == 0 {
                    continue;
                }
                return Err(endpoint_changed_error(&socket));
            }

            let request = WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: Some(ServerEpoch::capture(&self.repo_root)?),
                body: WalkRequestBody::Stop,
            };
            ipc::send(&mut stream, &request).await?;
            let response = ipc::recv(&mut stream).await?;
            validate_response_shape(&response)?;
            ensure_response_repo(&response, &self.repo_root)?;
            return Ok(response);
        }
        unreachable!("stop exchange either returns or retries once")
    }

    async fn probe_stop(
        &self,
    ) -> Result<Option<(PathBuf, Option<endpoint::ServerEndpoint>, WalkResponse)>, PrepareError>
    {
        for exchange in 0..2 {
            let Some((socket, endpoint, mut stream)) = self.connect_optional().await? else {
                return Ok(None);
            };
            let request = WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: None,
                body: WalkRequestBody::Health,
            };
            let result = match ipc::send(&mut stream, &request).await {
                Ok(()) => ipc::recv(&mut stream).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(response) => {
                    validate_response_shape(&response)?;
                    return Ok(Some((socket, endpoint, response)));
                }
                Err(_)
                    if exchange == 0
                        && self.follow_endpoint
                        && self.wait_for_successor(&socket, endpoint.as_ref()).await? =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("stop probe either returns or retries once")
    }

    fn endpoint_moved(
        &self,
        prior_socket: &Path,
        prior_endpoint: Option<&endpoint::ServerEndpoint>,
    ) -> Result<bool, PrepareError> {
        if !self.follow_endpoint {
            return Ok(false);
        }
        let (socket, endpoint) = self.endpoint_observation()?;
        Ok(socket != prior_socket || endpoint.as_ref() != prior_endpoint)
    }

    async fn wait_for_successor(
        &self,
        prior_socket: &Path,
        prior_endpoint: Option<&endpoint::ServerEndpoint>,
    ) -> Result<bool, PrepareError> {
        for _ in 0..5 {
            let (socket, endpoint) = self.endpoint_observation()?;
            if endpoint.is_some() && (socket != prior_socket || endpoint.as_ref() != prior_endpoint)
            {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (socket, endpoint) = self.endpoint_observation()?;
        Ok(endpoint.is_some() && (socket != prior_socket || endpoint.as_ref() != prior_endpoint))
    }

    async fn connect_optional(
        &self,
    ) -> Result<Option<(PathBuf, Option<endpoint::ServerEndpoint>, UnixStream)>, PrepareError> {
        for attempt in 0..2 {
            let (socket, endpoint) = self.endpoint_observation()?;
            match UnixStream::connect(&socket).await {
                Ok(stream) => return Ok(Some((socket, endpoint, stream))),
                Err(source)
                    if matches!(
                        source.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    if !self.follow_endpoint || attempt == 1 {
                        return Ok(None);
                    }
                    if !self.wait_for_successor(&socket, endpoint.as_ref()).await? {
                        return Ok(None);
                    }
                }
                Err(source) => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "prototype1_state_walk_connect",
                        detail: format!(
                            "failed to connect to walk socket '{}': {source}",
                            socket.display()
                        ),
                    });
                }
            }
        }
        Ok(None)
    }
}

fn requires_current_protocol(body: &WalkRequestBody) -> bool {
    matches!(
        body,
        WalkRequestBody::Config
            | WalkRequestBody::DbQuery { .. }
            | WalkRequestBody::EvidenceQuery { .. }
            | WalkRequestBody::SessionHistory
            | WalkRequestBody::OperationStatus { .. }
            | WalkRequestBody::ShowDelta { .. }
            | WalkRequestBody::EvaluationTraceIndex
            | WalkRequestBody::EvaluationTrace { .. }
            | WalkRequestBody::LlmTraceIndex
            | WalkRequestBody::LlmTrace { .. }
    )
}

fn mutation_guard(
    response: &WalkResponse,
    epoch: &ServerEpoch,
    operation: OperationId,
) -> Result<MutationGuard, PrepareError> {
    response.epoch().ensure_compatible_request(Some(epoch))?;
    let WalkResponse::Status { snapshot, .. } = response else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk mutation requires a typed health/status version; endpoint returned {response:?}"
            ),
        });
    };
    Ok(MutationGuard {
        operation,
        expected: snapshot.version(),
    })
}

fn validate_job_response(
    response: &WalkResponse,
    operation: OperationId,
    command: Option<WalkJobKind>,
    action: &str,
) -> Result<(), PrepareError> {
    match response {
        WalkResponse::Job { job, .. }
            if job.operation_id == operation
                && command.is_none_or(|expected| job.command == expected) =>
        {
            Ok(())
        }
        WalkResponse::Error { .. } => Ok(()),
        WalkResponse::Job { job, .. } => Err(protocol_error(format!(
            "walk {action} response identified operation {} and command {}, expected operation {operation}{}",
            job.operation_id,
            job.command.as_str(),
            command
                .map(|expected| format!(" and command {}", expected.as_str()))
                .unwrap_or_default()
        ))),
        response => Err(protocol_error(format!(
            "walk {action} returned an unexpected response at phase {:?}",
            response.phase()
        ))),
    }
}

fn validate_advertised_response(
    response: &WalkResponse,
    advertised: &AdvertisedStep,
) -> Result<(), PrepareError> {
    match response {
        WalkResponse::Job { job, .. } => {
            advertised.validate_job(job)?;
            if job.status == WalkJobStatus::Succeeded {
                advertised.validate_terminal(job)?;
            }
            Ok(())
        }
        WalkResponse::Error { .. } => Ok(()),
        response => Err(protocol_error(format!(
            "advertised Step returned an unexpected response at phase {:?}",
            response.phase()
        ))),
    }
}

fn validate_stop_response(response: &WalkResponse) -> Result<(), PrepareError> {
    match response {
        WalkResponse::Status { snapshot, .. } if snapshot.authority == WalkAuthority::Stopping => {
            Ok(())
        }
        WalkResponse::Error { .. } => Ok(()),
        response => Err(protocol_error(format!(
            "walk stop returned an unexpected response at phase {:?}",
            response.phase()
        ))),
    }
}

fn validate_response_shape(response: &WalkResponse) -> Result<(), PrepareError> {
    let epoch = response.epoch();
    if epoch.protocol_version >= crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION
        && (epoch.build_fingerprint.len() != 64
            || !epoch
                .build_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(protocol_error(format!(
            "walk protocol {} response omitted its required build fingerprint",
            epoch.protocol_version
        )));
    }
    if let WalkResponse::Status {
        snapshot, epoch, ..
    } = response
        && epoch.protocol_version
            >= crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION
        && matches!(&snapshot.position, WalkPosition::Legacy { .. })
    {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_protocol",
            detail: format!(
                "walk protocol {} status omitted its required position authority",
                epoch.protocol_version
            ),
        });
    }
    Ok(())
}

fn connect_error(socket: &Path, source: std::io::Error) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_connect",
        detail: format!(
            "failed to connect to walk socket '{}': {source}",
            socket.display()
        ),
    }
}

fn endpoint_changed_error(socket: &Path) -> PrepareError {
    protocol_error(format!(
        "authoritative walk endpoint changed after probing '{}'; retry the operation against the current endpoint",
        socket.display()
    ))
}

fn protocol_error(detail: String) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_protocol",
        detail,
    }
}

fn ensure_current_protocol(response: &WalkResponse, repo_root: &Path) -> Result<(), PrepareError> {
    let client_epoch = ServerEpoch::capture(repo_root)?;
    response
        .epoch()
        .ensure_compatible_request(Some(&client_epoch))
}

fn ensure_response_repo(response: &WalkResponse, repo_root: &Path) -> Result<(), PrepareError> {
    let server_root = &response.epoch().repo_root;
    if server_root == repo_root {
        return Ok(());
    }
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "walk repository root mismatch: client='{}' server='{}'",
            repo_root.display(),
            server_root.display()
        ),
    })
}

/// Discover Prototype 1 campaign roots under `ploke_eval_home()/campaigns`.
pub fn discover_walk_runs() -> Result<Vec<WalkRunEntry>, PrepareError> {
    let root = campaigns_dir()?;
    let mut runs = Vec::new();
    if !root.exists() {
        return Ok(runs);
    }

    for entry in fs::read_dir(&root).map_err(|source| PrepareError::ReadCampaignManifest {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadCampaignManifest {
            path: root.clone(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| PrepareError::ReadCampaignManifest {
                path: entry.path(),
                source,
            })?;
        if !file_type.is_dir() {
            continue;
        }

        let campaign_dir = entry.path();
        let prototype1_root = campaign_dir.join("prototype1");
        if !prototype1_root.is_dir() {
            continue;
        }
        let campaign_id = entry.file_name().to_string_lossy().into_owned();
        let campaign = CampaignId::from(campaign_id.clone());
        let manifest_path = campaign_dir.join("campaign.json");
        let worktree_root = admitted_repo_root(&manifest_path, &campaign)?;
        let owner_db_path = prototype1_eval_store_db_path(&manifest_path);
        let has_parent_identity = worktree_root.is_some();
        let modified_unix_ms = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(system_time_unix_ms);

        runs.push(WalkRunEntry {
            campaign_id,
            campaign_dir,
            prototype1_root,
            worktree_root,
            owner_db_path: owner_db_path.clone(),
            modified_unix_ms,
            has_manifest: manifest_path.exists(),
            has_closure_state: entry.path().join("closure-state.json").exists(),
            has_owner_db: owner_db_path.exists(),
            has_parent_identity,
        });
    }

    runs.sort_by(|left, right| {
        right
            .modified_unix_ms
            .cmp(&left.modified_unix_ms)
            .then_with(|| left.campaign_id.cmp(&right.campaign_id))
    });
    Ok(runs)
}

fn admitted_repo_root(
    manifest_path: &Path,
    campaign: &CampaignId,
) -> Result<Option<PathBuf>, PrepareError> {
    let receipt_path = setup_admission::setup_admission_path(manifest_path);
    let Some(receipt) = setup_admission::load_setup_admission(&receipt_path)? else {
        return Ok(None);
    };
    if receipt.completed_head().is_none()
        || &receipt.intent.campaign_id != campaign
        || receipt.intent.manifest_path != manifest_path
    {
        return Ok(None);
    }

    let repo_root = &receipt.intent.repo_root;
    if !repo_root.is_dir() {
        return Ok(None);
    }
    let Ok(canonical) = repo_root.canonicalize() else {
        return Ok(None);
    };
    if canonical != *repo_root {
        return Ok(None);
    }

    let Some(parent) = identity::load_parent_identity_optional(&canonical)? else {
        return Ok(None);
    };
    if parent.campaign_id() != campaign {
        return Ok(None);
    }
    parent.validate_for_command(campaign, None)?;
    Ok(Some(canonical))
}

fn system_time_unix_ms(time: SystemTime) -> Option<u64> {
    let millis = time.duration_since(UNIX_EPOCH).ok()?.as_millis();
    u64::try_from(millis).ok()
}

impl PhaseInventory {
    pub fn current() -> Self {
        let phases = all_phases()
            .into_iter()
            .map(|phase| PhaseInfo {
                phase,
                id: phase.as_str().to_string(),
                label: phase.detail().to_string(),
                typestate: phase.typestate(),
                next: phase
                    .next_steps()
                    .iter()
                    .map(|step| {
                        let edge = ControlEdge::from_phases(phase, step.phase)
                            .expect("walk inventory edge must exist in the control graph");
                        NextStepInfo {
                            edge,
                            phase: step.phase,
                            phase_id: step.phase.as_str().to_string(),
                            detail: step.detail.to_string(),
                            requires_watch: false,
                            requires_live_api: edge.requires_live(),
                            requires_git_changes: edge.requires_checkout(),
                        }
                    })
                    .collect(),
            })
            .collect();
        Self { phases }
    }
}

fn all_phases() -> [WalkPhase; 22] {
    [
        WalkPhase::Empty,
        WalkPhase::R0,
        WalkPhase::R1,
        WalkPhase::R2a,
        WalkPhase::R3,
        WalkPhase::R4a,
        WalkPhase::R4b,
        WalkPhase::R4c,
        WalkPhase::R5,
        WalkPhase::R6,
        WalkPhase::R7,
        WalkPhase::R8,
        WalkPhase::R9,
        WalkPhase::R10,
        WalkPhase::R11a,
        WalkPhase::R11,
        WalkPhase::R12,
        WalkPhase::R13a,
        WalkPhase::R13b,
        WalkPhase::R13c,
        WalkPhase::R14a,
        WalkPhase::R14b,
    ]
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use ploke_llm::request::models::ModelRouteSource;
    use ploke_records::ids::CampaignId;

    use crate::cli::prototype1_state::{
        event::{ContentHash, RecordedAt},
        identity::ParentIdentity,
        setup_admission::{SetupAdmissionIntent, SetupCheckoutBase},
        walk::query::evidence_query_script,
    };
    use crate::{
        BenchmarkFamily, CampaignManifest, EvalCampaignPolicy, FrameworkConfig,
        ProtocolCampaignPolicy, RegistryDatasetSource, ResolvedCampaignConfig,
        intervention::{
            PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1NodeRecord, Prototype1NodeStatus,
            Prototype1RunnerRequest,
        },
    };

    use super::*;

    #[cfg(unix)]
    async fn answer_protocol_probe(listener: &tokio::net::UnixListener, epoch: ServerEpoch) {
        let (mut stream, _) = listener.accept().await.expect("accept protocol probe");
        let request: WalkRequest = ipc::recv(&mut stream).await.expect("read protocol probe");
        assert!(matches!(request.body, WalkRequestBody::Health));
        assert_eq!(
            request
                .client_protocol
                .expect("public read probe must identify its protocol"),
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION
        );
        ipc::send(
            &mut stream,
            &WalkResponse::ok(WalkOkKind::Show, WalkPhase::Empty, "healthy", epoch),
        )
        .await
        .expect("write protocol probe");
    }

    fn sibling_epoch(repo_root: &Path) -> ServerEpoch {
        let mut epoch = ServerEpoch::capture(repo_root).expect("capture sibling epoch");
        let exe_path = repo_root.join("sibling-ploke-eval");
        fs::write(&exe_path, b"sibling executable fixture").expect("write sibling executable");
        let modified = fs::metadata(&exe_path)
            .expect("sibling executable metadata")
            .modified()
            .expect("sibling executable mtime");
        epoch.exe_path = exe_path;
        epoch.exe_modified_unix_ms = system_time_unix_ms(modified);
        epoch
    }

    fn status_response(
        epoch: ServerEpoch,
        position: WalkPosition,
        authority: WalkAuthority,
    ) -> WalkResponse {
        WalkResponse::Status {
            message: "online".to_string(),
            snapshot: WalkSessionSnapshot {
                controller_attached: !matches!(&position, WalkPosition::NoSession),
                position,
                authority,
                job: None,
                blocker: None,
                actions: Vec::new(),
            },
            epoch,
        }
    }

    fn query_response(
        epoch: ServerEpoch,
        repo_root: &Path,
        campaign: &CampaignId,
        script: &str,
        view: Option<WalkEvidenceQuery>,
    ) -> WalkResponse {
        let result: DbQueryResult = serde_json::from_value(serde_json::json!({
            "repo_root": repo_root,
            "campaign_id": campaign,
            "db_path": repo_root.join("owner.cozo"),
            "view": view,
            "script": script,
            "revision": "query-revision",
            "headers": ["name"],
            "row_count": 1,
            "rows": [{"cells": ["eval_campaign"], "object": {"name": "eval_campaign"}}]
        }))
        .expect("query result carrier");
        WalkResponse::Query {
            query: WalkQuerySnapshot {
                phase: WalkPhase::Empty,
                result,
                version: SessionVersion::empty(),
                epoch,
            },
        }
    }

    fn raw_query_response(
        epoch: ServerEpoch,
        repo_root: &Path,
        campaign: &CampaignId,
        script: &str,
    ) -> WalkResponse {
        query_response(epoch, repo_root, campaign, script, None)
    }

    fn evidence_query_response(
        epoch: ServerEpoch,
        repo_root: &Path,
        campaign: &CampaignId,
        view: WalkEvidenceQuery,
    ) -> WalkResponse {
        query_response(
            epoch,
            repo_root,
            campaign,
            evidence_query_script(view),
            Some(view),
        )
    }

    fn setup_intent(
        campaign: CampaignId,
        manifest: PathBuf,
        repo: PathBuf,
    ) -> SetupAdmissionIntent {
        let node_id = "node-root".to_string();
        let branch = format!("prototype1-parent-{}-gen0", campaign.as_str());
        let node_dir = manifest
            .parent()
            .expect("campaign directory")
            .join("prototype1/nodes/node-root");
        let request_path = node_dir.join("runner-request.json");
        let result_path = node_dir.join("runner-result.json");
        let binary_path = node_dir.join("bin/ploke-eval");
        let node = Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: node_id.clone(),
            parent_node_id: None,
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: format!("prototype1-root:{campaign}"),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: branch.clone(),
            candidate_id: "root-parent".to_string(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            node_dir: node_dir.clone(),
            workspace_root: repo.clone(),
            binary_path: binary_path.clone(),
            runner_request_path: request_path,
            runner_result_path: result_path,
            status: Prototype1NodeStatus::Planned,
            created_at: "2026-07-31T00:00:00Z".to_string(),
            updated_at: "2026-07-31T00:00:00Z".to_string(),
        };
        let request = Prototype1RunnerRequest {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            campaign_id: campaign.clone(),
            node_id: node_id.clone(),
            generation: 0,
            instance_id: "instance-1".to_string(),
            source_state_id: format!("prototype1-root:{campaign}"),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            branch_id: branch.clone(),
            target_relpath: PathBuf::from(".ploke/prototype1/parent_identity.json"),
            workspace_root: repo.clone(),
            binary_path,
            stop_on_error: false,
            runner_args: vec!["loop".to_string(), "prototype1-state".to_string()],
        };
        let parent = ParentIdentity::root_bootstrap(
            campaign.clone(),
            node_id,
            "instance-1",
            branch.clone(),
            Some(branch.clone()),
        );
        SetupAdmissionIntent {
            plan_hash: ContentHash("a".repeat(64)),
            campaign_id: campaign,
            manifest_path: manifest,
            repo_root: repo,
            artifact_branch: branch,
            batch_manifest: node_dir.join("batch.json"),
            hashes: SetupArtifactHashes {
                manifest: ContentHash("b".repeat(64)),
                slice: ContentHash("c".repeat(64)),
                profile: ContentHash("d".repeat(64)),
            },
            checkout: SetupCheckoutBase {
                branch: "main".to_string(),
                head: GitCommit("e".repeat(40)),
            },
            node,
            request,
            identity: parent,
            started_at: RecordedAt(1_785_456_000_000),
        }
    }

    fn write_receipt(path: &Path, intent: SetupAdmissionIntent, complete: bool) {
        let current =
            setup_admission::create_setup_admission(path, intent).expect("create setup receipt");
        if complete {
            let next = current
                .clone()
                .complete(GitCommit("f".repeat(40)), RecordedAt(1_785_456_060_000))
                .expect("complete setup receipt");
            setup_admission::replace_setup_admission(path, &current, &next)
                .expect("persist completed setup receipt");
        }
    }

    fn write_parent(repo: &Path, campaign: CampaignId, generation: u32) {
        let node_id = format!("node-{generation}");
        let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: identity::PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: campaign,
            parent_id: node_id.clone(),
            node_id,
            generation,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: (generation > 0).then(|| "node-0".to_string()),
            parent_node_id: (generation > 0).then(|| "node-0".to_string()),
            branch_id: format!("branch-{generation}"),
            artifact_branch: Some(format!("artifact-{generation}")),
            created_at: "2026-07-31T00:00:00Z".to_string(),
        });
        identity::write_parent_identity(repo, &parent).expect("write current parent identity");
    }

    fn write_campaign(eval_home: &Path, campaign: &CampaignId) -> PathBuf {
        let root = eval_home.join("campaigns").join(campaign.as_str());
        fs::create_dir_all(root.join("prototype1")).expect("create Prototype 1 campaign root");
        let manifest = root.join("campaign.json");
        fs::write(&manifest, b"{}").expect("write campaign marker");
        manifest
    }

    fn job_response(
        epoch: ServerEpoch,
        operation: OperationId,
        expected: SessionVersion,
        command: WalkJobKind,
        phase: WalkPhase,
        target: Option<WalkPhase>,
    ) -> WalkResponse {
        WalkResponse::Job {
            phase,
            job: WalkJobSnapshot {
                job_id: 17,
                operation_id: operation,
                expected,
                command,
                status: WalkJobStatus::Running,
                phase_before: phase,
                phase_after: None,
                target_phase: target,
                watch: Some(false),
                allow_live_api: Some(false),
                allow_git_changes: Some(false),
                llm_source: None,
                allow_workspace_mutation: None,
                allow_provenance_record: None,
                started_at: "2026-07-26T00:00:00Z".to_string(),
                updated_at: "2026-07-26T00:00:00Z".to_string(),
                finished_at: None,
                message: Some("accepted".to_string()),
                receipt: None,
                resolution: None,
            },
            message: "accepted".to_string(),
            epoch,
        }
    }

    fn advertised_snapshot(
        phase: WalkPhase,
        revision: usize,
        edges: &[ControlEdge],
    ) -> WalkSessionSnapshot {
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(revision as u128 + 100)),
            cursor: Some(
                Cursor::new(phase, ContentHash::of(&format!("advertised Step {phase}")))
                    .expect("valid advertised Step cursor"),
            ),
            journal_revision: revision,
        };
        WalkSessionSnapshot {
            position: WalkPosition::Session { version },
            controller_attached: true,
            authority: WalkAuthority::Active,
            job: None,
            blocker: None,
            actions: edges
                .iter()
                .copied()
                .map(|edge| WalkAction {
                    kind: WalkActionKind::Step,
                    edge: Some(edge),
                    target: Some(edge.to()),
                    enabled: true,
                    requires_live_api: edge.requires_live(),
                    requires_git_changes: edge.requires_checkout(),
                    blocker: None,
                })
                .collect(),
        }
    }

    fn terminal_step(
        advertised: &AdvertisedStep,
        edge: ControlEdge,
        operation: OperationId,
    ) -> WalkJobSnapshot {
        WalkJobSnapshot {
            job_id: 23,
            operation_id: operation,
            expected: advertised.version().clone(),
            command: WalkJobKind::Step,
            status: WalkJobStatus::Succeeded,
            phase_before: advertised.version().phase(),
            phase_after: Some(edge.to()),
            target_phase: None,
            watch: Some(false),
            allow_live_api: Some(advertised.allows_live_api()),
            allow_git_changes: Some(advertised.allows_git_changes()),
            llm_source: None,
            allow_workspace_mutation: None,
            allow_provenance_record: None,
            started_at: "2026-07-31T00:00:00Z".to_string(),
            updated_at: "2026-07-31T00:00:01Z".to_string(),
            finished_at: Some("2026-07-31T00:00:01Z".to_string()),
            message: Some("succeeded".to_string()),
            receipt: Some(WalkTransitionReceipt {
                phase_before: advertised.version().phase(),
                phase_after: edge.to(),
                edges: vec![edge],
                version: SessionVersion {
                    session_id: advertised.version().session_id(),
                    cursor: Some(
                        Cursor::new(edge.to(), ContentHash::of("realized advertised Step"))
                            .expect("valid realized cursor"),
                    ),
                    journal_revision: advertised.version().journal_revision() + 2,
                },
                event_projection: WalkEventProjection::Recorded,
            }),
            resolution: None,
        }
    }

    #[test]
    fn advertised_step_aggregates_branch_outcomes_and_capability_grants() {
        let r1 = advertised_snapshot(
            WalkPhase::R1,
            1,
            &[ControlEdge::R1ToR2a, ControlEdge::R1ToR3],
        );
        let r1_step = AdvertisedStep::from_snapshot(&r1, false, false).expect("R1 offer");
        assert_eq!(
            r1_step.outcomes(),
            &BTreeSet::from([ControlEdge::R1ToR2a, ControlEdge::R1ToR3])
        );

        let r4a = advertised_snapshot(
            WalkPhase::R4a,
            2,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let r4a_step = AdvertisedStep::from_snapshot(&r4a, false, false).expect("R4a offer");
        assert_eq!(
            r4a_step.outcomes(),
            &BTreeSet::from([ControlEdge::R4aToR4b, ControlEdge::R4aToR4c])
        );

        let r10 = advertised_snapshot(
            WalkPhase::R10,
            3,
            &[ControlEdge::R10ToR11a, ControlEdge::R10ToR11],
        );
        let error = AdvertisedStep::from_snapshot(&r10, false, false)
            .expect_err("R10 requires live authority")
            .to_string();
        assert!(error.contains("live provider grant"), "{error}");
        let r10_step = AdvertisedStep::from_snapshot(&r10, true, false).expect("live R10 offer");
        assert_eq!(
            r10_step.outcomes(),
            &BTreeSet::from([ControlEdge::R10ToR11a, ControlEdge::R10ToR11])
        );

        let r12 = advertised_snapshot(
            WalkPhase::R12,
            4,
            &[
                ControlEdge::R12ToR13a,
                ControlEdge::R12ToR13b,
                ControlEdge::R12ToR13c,
            ],
        );
        let safe = AdvertisedStep::from_snapshot(&r12, false, false).expect("stop-only R12 offer");
        assert_eq!(safe.outcomes(), &BTreeSet::from([ControlEdge::R12ToR13a]));
        let checkout =
            AdvertisedStep::from_snapshot(&r12, false, true).expect("checkout R12 offer");
        assert_eq!(
            checkout.outcomes(),
            &BTreeSet::from([
                ControlEdge::R12ToR13a,
                ControlEdge::R12ToR13b,
                ControlEdge::R12ToR13c,
            ])
        );
    }

    #[test]
    fn advertised_step_rejects_malformed_or_duplicate_action_rows() {
        let mut snapshot = advertised_snapshot(
            WalkPhase::R4a,
            5,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        snapshot.actions[0].target = Some(WalkPhase::R4c);
        let error = AdvertisedStep::from_snapshot(&snapshot, false, false)
            .expect_err("wrong target must fail")
            .to_string();
        assert!(error.contains("targets"), "{error}");

        let mut snapshot = advertised_snapshot(
            WalkPhase::R4a,
            6,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        snapshot.actions.push(snapshot.actions[0].clone());
        let error = AdvertisedStep::from_snapshot(&snapshot, false, false)
            .expect_err("duplicate edge must fail")
            .to_string();
        assert!(error.contains("duplicated outcome"), "{error}");

        let mut snapshot = advertised_snapshot(WalkPhase::R5, 7, &[ControlEdge::R5ToR6]);
        snapshot.actions[0].requires_live_api = false;
        let error = AdvertisedStep::from_snapshot(&snapshot, true, false)
            .expect_err("capability mismatch must fail")
            .to_string();
        assert!(error.contains("capability flags"), "{error}");

        let mut snapshot = advertised_snapshot(WalkPhase::R6, 9, &[ControlEdge::R6ToR7]);
        snapshot.actions[0].enabled = false;
        let error = AdvertisedStep::from_snapshot(&snapshot, false, false)
            .expect_err("disabled Step under Active authority must fail")
            .to_string();
        assert!(
            error.contains("inconsistent with Active authority"),
            "{error}"
        );

        let mut snapshot = advertised_snapshot(
            WalkPhase::R4a,
            10,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        snapshot.actions.pop();
        let error = AdvertisedStep::from_snapshot(&snapshot, false, false)
            .expect_err("missing branch outcome must fail")
            .to_string();
        assert!(error.contains("complete control outcomes"), "{error}");
    }

    #[test]
    fn advertised_step_terminal_accepts_retained_and_rejects_outside_outcomes() {
        let snapshot = advertised_snapshot(
            WalkPhase::R4a,
            8,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let advertised =
            AdvertisedStep::from_snapshot(&snapshot, false, false).expect("branch offer");
        let first = terminal_step(
            &advertised,
            ControlEdge::R4aToR4b,
            OperationId::for_test(120),
        );
        assert_eq!(
            advertised
                .validate_terminal(&first)
                .expect("first branch retained")
                .phase_after,
            WalkPhase::R4b
        );
        let second = terminal_step(
            &advertised,
            ControlEdge::R4aToR4c,
            OperationId::for_test(121),
        );
        assert_eq!(
            advertised
                .validate_terminal(&second)
                .expect("second branch retained")
                .phase_after,
            WalkPhase::R4c
        );

        let outside = terminal_step(&advertised, ControlEdge::R5ToR6, OperationId::for_test(122));
        let error = advertised
            .validate_terminal(&outside)
            .expect_err("outside edge must fail")
            .to_string();
        assert!(error.contains("outside retained outcomes"), "{error}");
    }

    #[test]
    fn advertised_step_terminal_rejects_foreign_session() {
        let snapshot = advertised_snapshot(WalkPhase::R5, 8, &[ControlEdge::R5ToR6]);
        let advertised =
            AdvertisedStep::from_snapshot(&snapshot, true, false).expect("advertised Step");
        let mut terminal =
            terminal_step(&advertised, ControlEdge::R5ToR6, OperationId::for_test(123));
        terminal
            .receipt
            .as_mut()
            .expect("terminal receipt")
            .version
            .session_id = Some(SessionId::for_test(999));

        let error = advertised
            .validate_terminal(&terminal)
            .expect_err("foreign receipt session must fail")
            .to_string();

        assert!(error.contains("does not match retained session"), "{error}");
    }

    #[test]
    fn advertised_step_terminal_rejects_nonadvancing_revision() {
        let snapshot = advertised_snapshot(WalkPhase::R5, 8, &[ControlEdge::R5ToR6]);
        let advertised =
            AdvertisedStep::from_snapshot(&snapshot, true, false).expect("advertised Step");
        let mut terminal =
            terminal_step(&advertised, ControlEdge::R5ToR6, OperationId::for_test(124));
        terminal
            .receipt
            .as_mut()
            .expect("terminal receipt")
            .version
            .journal_revision = advertised.version().journal_revision();

        let error = advertised
            .validate_terminal(&terminal)
            .expect_err("nonadvancing receipt revision must fail")
            .to_string();

        assert!(error.contains("did not advance"), "{error}");
    }

    #[test]
    fn advertised_step_identifies_endpoint_transfer_outcome() {
        let r12 = advertised_snapshot(
            WalkPhase::R12,
            8,
            &[
                ControlEdge::R12ToR13a,
                ControlEdge::R12ToR13b,
                ControlEdge::R12ToR13c,
            ],
        );
        let stop_only = AdvertisedStep::from_snapshot(&r12, false, false).expect("stop-only Step");
        let handoff =
            AdvertisedStep::from_snapshot(&r12, false, true).expect("handoff-capable Step");
        let ordinary = AdvertisedStep::from_snapshot(
            &advertised_snapshot(WalkPhase::R6, 9, &[ControlEdge::R6ToR7]),
            false,
            false,
        )
        .expect("ordinary Step");

        assert!(!stop_only.requires_endpoint_following());
        assert!(handoff.requires_endpoint_following());
        assert!(!ordinary.requires_endpoint_following());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn start_uses_local_epoch_version_and_operation() {
        let repo = tempfile::tempdir().expect("start client repo");
        let repo_root = repo.path().canonicalize().expect("canonical start repo");
        let socket = repo.path().join("start.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind start socket");
        let server_epoch = sibling_epoch(&repo_root);
        let operation = OperationId::for_test(101);
        let server = {
            let server_epoch = server_epoch.clone();
            let repo_root = repo_root.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept start health");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read start health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch.clone(),
                        WalkPosition::NoSession,
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write start health");

                let (mut stream, _) = listener.accept().await.expect("accept start request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read start request");
                let client_epoch = request.client_epoch.expect("start client epoch");
                assert_ne!(client_epoch.exe_path, server_epoch.exe_path);
                assert_eq!(
                    client_epoch.build_fingerprint,
                    server_epoch.build_fingerprint
                );
                match request.body {
                    WalkRequestBody::Start {
                        guard,
                        config,
                        until,
                        allow_live_api,
                    } => {
                        assert_eq!(guard.operation, operation);
                        assert_eq!(guard.expected, SessionVersion::empty());
                        assert_eq!(config.repo_root.as_deref(), Some(repo_root.as_path()));
                        assert_eq!(config.campaign, Some(CampaignId::from("start-campaign")));
                        assert_eq!(until, WalkPhase::R3);
                        assert!(!allow_live_api);
                    }
                    body => panic!("expected start request, got {body:?}"),
                }
                ipc::send(
                    &mut stream,
                    &job_response(
                        server_epoch,
                        operation,
                        SessionVersion::empty(),
                        WalkJobKind::Start,
                        WalkPhase::Empty,
                        Some(WalkPhase::R3),
                    ),
                )
                .await
                .expect("write start response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve start client");
        let response = client
            .start(
                WalkStartConfig {
                    campaign: Some(CampaignId::from("start-campaign")),
                    repo_root: None,
                },
                WalkPhase::R3,
                false,
                operation,
            )
            .await
            .expect("submit guarded start");
        assert!(matches!(
            response,
            WalkResponse::Job {
                job: WalkJobSnapshot {
                    command: WalkJobKind::Start,
                    operation_id,
                    ..
                },
                ..
            } if operation_id == operation
        ));
        server.await.expect("start server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertised_start_uses_the_fresh_observed_version_and_target() {
        let repo = tempfile::tempdir().expect("advertised start repo");
        let repo_root = repo
            .path()
            .canonicalize()
            .expect("canonical advertised start repo");
        let socket = repo.path().join("advertised-start.sock");
        let listener =
            tokio::net::UnixListener::bind(&socket).expect("bind advertised start socket");
        let server_epoch = sibling_epoch(&repo_root);
        let operation = OperationId::for_test(110);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(10)),
            cursor: Some(
                Cursor::new(WalkPhase::R7, ContentHash::of("advertised start position"))
                    .expect("valid advertised start cursor"),
            ),
            journal_revision: 17,
        };
        let server = {
            let epoch = server_epoch.clone();
            let expected = version.clone();
            let repo_root = repo_root.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener
                    .accept()
                    .await
                    .expect("accept advertised start health");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read advertised start health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                let mut status = status_response(
                    epoch.clone(),
                    WalkPosition::Session {
                        version: expected.clone(),
                    },
                    WalkAuthority::Active,
                );
                let WalkResponse::Status { snapshot, .. } = &mut status else {
                    panic!("status helper must return status");
                };
                snapshot.controller_attached = false;
                snapshot.actions.push(WalkAction {
                    kind: WalkActionKind::Start,
                    edge: None,
                    target: Some(WalkPhase::R7),
                    enabled: true,
                    requires_live_api: false,
                    requires_git_changes: false,
                    blocker: None,
                });
                ipc::send(&mut stream, &status)
                    .await
                    .expect("write advertised start health");

                let (mut stream, _) = listener
                    .accept()
                    .await
                    .expect("accept advertised start request");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read advertised start request");
                let WalkRequestBody::Start {
                    guard,
                    config,
                    until,
                    allow_live_api,
                } = request.body
                else {
                    panic!("expected advertised Start request");
                };
                assert_eq!(guard.operation, operation);
                assert_eq!(guard.expected, expected);
                assert_eq!(config.repo_root.as_deref(), Some(repo_root.as_path()));
                assert_eq!(
                    config.campaign,
                    Some(CampaignId::from("advertised-start-campaign"))
                );
                assert_eq!(until, WalkPhase::R7);
                assert!(allow_live_api);
                ipc::send(
                    &mut stream,
                    &job_response(
                        epoch,
                        operation,
                        expected,
                        WalkJobKind::Start,
                        WalkPhase::R7,
                        Some(WalkPhase::R7),
                    ),
                )
                .await
                .expect("write advertised start response");
            })
        };
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket))
            .expect("resolve advertised start client");

        let response = client
            .start_advertised(
                WalkStartConfig {
                    campaign: Some(CampaignId::from("advertised-start-campaign")),
                    repo_root: None,
                },
                WalkPhase::R7,
                true,
                operation,
            )
            .await
            .expect("submit advertised Start");

        assert!(matches!(
            response,
            WalkResponse::Job {
                job: WalkJobSnapshot {
                    command: WalkJobKind::Start,
                    operation_id,
                    expected,
                    target_phase: Some(WalkPhase::R7),
                    ..
                },
                ..
            } if operation_id == operation && expected == version
        ));
        server.await.expect("advertised start server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertised_start_rejects_a_stale_target_before_framing_a_request() {
        let repo = tempfile::tempdir().expect("stale advertised start repo");
        let repo_root = repo
            .path()
            .canonicalize()
            .expect("canonical stale advertised start repo");
        let socket = repo.path().join("stale-advertised-start.sock");
        let listener =
            tokio::net::UnixListener::bind(&socket).expect("bind stale advertised start socket");
        let epoch = sibling_epoch(&repo_root);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(11)),
            cursor: Some(
                Cursor::new(WalkPhase::R7, ContentHash::of("fresh successor position"))
                    .expect("valid successor cursor"),
            ),
            journal_revision: 18,
        };
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("accept stale advertised start health");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read stale advertised start health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            let mut status = status_response(
                epoch,
                WalkPosition::Session { version },
                WalkAuthority::Active,
            );
            let WalkResponse::Status { snapshot, .. } = &mut status else {
                panic!("status helper must return status");
            };
            snapshot.controller_attached = false;
            snapshot.actions.push(WalkAction {
                kind: WalkActionKind::Start,
                edge: None,
                target: Some(WalkPhase::R7),
                enabled: true,
                requires_live_api: false,
                requires_git_changes: false,
                blocker: None,
            });
            ipc::send(&mut stream, &status)
                .await
                .expect("write stale advertised start status");

            let (mut stream, _) =
                tokio::time::timeout(Duration::from_millis(500), listener.accept())
                    .await
                    .expect("advertised Start must open its guarded submission socket")
                    .expect("accept guarded advertised Start socket");
            let error = tokio::time::timeout(
                Duration::from_millis(500),
                ipc::recv::<WalkRequest>(&mut stream),
            )
            .await
            .expect("stale advertised Start must close its guarded submission socket")
            .expect_err("stale advertised Start must not send a framed request");
            assert!(
                matches!(
                    error,
                    PrepareError::DatabaseSetup {
                        phase: "prototype1_state_walk_ipc_read",
                        ..
                    }
                ),
                "unexpected closed-socket error: {error}"
            );
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket))
            .expect("resolve stale advertised start client");

        let error = client
            .start_advertised(
                WalkStartConfig {
                    campaign: Some(CampaignId::from("stale-advertised-start-campaign")),
                    repo_root: None,
                },
                WalkPhase::R3,
                false,
                OperationId::for_test(111),
            )
            .await
            .expect_err("stale advertised Start must fail closed")
            .to_string();

        assert!(
            error.contains("Start targeting r3; expected exactly one"),
            "{error}"
        );
        server.await.expect("stale advertised start server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn step_uses_observed_version_and_capabilities() {
        let repo = tempfile::tempdir().expect("step client repo");
        let repo_root = repo.path().canonicalize().expect("canonical step repo");
        let socket = repo.path().join("step.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind step socket");
        let server_epoch = sibling_epoch(&repo_root);
        let operation = OperationId::for_test(102);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(7)),
            cursor: Some(
                Cursor::new(WalkPhase::R6, ContentHash::of("step position"))
                    .expect("valid step cursor"),
            ),
            journal_revision: 11,
        };
        let server = {
            let server_epoch = server_epoch.clone();
            let version = version.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept step health");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read step health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch.clone(),
                        WalkPosition::Session {
                            version: version.clone(),
                        },
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write step health");

                let (mut stream, _) = listener.accept().await.expect("accept step request");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read step request");
                match request.body {
                    WalkRequestBody::Step {
                        guard,
                        until,
                        watch,
                        allow_live_api,
                        allow_git_changes,
                    } => {
                        assert_eq!(guard.operation, operation);
                        assert_eq!(guard.expected, version);
                        assert_eq!(until, Some(WalkPhase::R7));
                        assert!(watch);
                        assert!(allow_live_api);
                        assert!(!allow_git_changes);
                    }
                    body => panic!("expected step request, got {body:?}"),
                }
                ipc::send(
                    &mut stream,
                    &job_response(
                        server_epoch,
                        operation,
                        version,
                        WalkJobKind::Step,
                        WalkPhase::R6,
                        Some(WalkPhase::R7),
                    ),
                )
                .await
                .expect("write step response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve step client");
        let response = client
            .step(Some(WalkPhase::R7), true, true, false, operation)
            .await
            .expect("submit guarded step");
        assert!(matches!(
            response,
            WalkResponse::Job {
                job: WalkJobSnapshot {
                    command: WalkJobKind::Step,
                    operation_id,
                    ..
                },
                ..
            } if operation_id == operation
        ));
        server.await.expect("step server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertised_step_uses_exact_offer_and_sends_no_until_bound() {
        let repo = tempfile::tempdir().expect("advertised Step repo");
        let repo_root = repo.path().canonicalize().expect("canonical Step repo");
        let socket = repo.path().join("advertised-step.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind Step socket");
        let epoch = sibling_epoch(&repo_root);
        let operation = OperationId::for_test(123);
        let snapshot = advertised_snapshot(
            WalkPhase::R4a,
            21,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let advertised =
            AdvertisedStep::from_snapshot(&snapshot, false, false).expect("displayed Step");
        let expected = snapshot.version();
        let server = {
            let epoch = epoch.clone();
            let snapshot = snapshot.clone();
            let expected = expected.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept Step health");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read Step health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &WalkResponse::status(snapshot, "online", epoch.clone()),
                )
                .await
                .expect("write Step health");

                let (mut stream, _) = listener.accept().await.expect("accept Step request");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read Step request");
                let WalkRequestBody::Step {
                    guard,
                    until,
                    watch,
                    allow_live_api,
                    allow_git_changes,
                } = request.body
                else {
                    panic!("expected advertised Step request");
                };
                assert_eq!(guard.operation, operation);
                assert_eq!(guard.expected, expected);
                assert_eq!(until, None);
                assert!(!watch);
                assert!(!allow_live_api);
                assert!(!allow_git_changes);
                ipc::send(
                    &mut stream,
                    &job_response(
                        epoch,
                        operation,
                        expected,
                        WalkJobKind::Step,
                        WalkPhase::R4a,
                        None,
                    ),
                )
                .await
                .expect("write Step response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve Step client");

        let response = client
            .step_advertised(&advertised, operation)
            .await
            .expect("submit advertised Step");

        assert!(matches!(response, WalkResponse::Job { .. }));
        server.await.expect("advertised Step server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertised_step_rejects_fresh_version_change_without_mutation_frame() {
        let repo = tempfile::tempdir().expect("stale advertised Step repo");
        let repo_root = repo.path().canonicalize().expect("canonical stale repo");
        let socket = repo.path().join("stale-advertised-step.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind stale Step socket");
        let epoch = sibling_epoch(&repo_root);
        let displayed = advertised_snapshot(
            WalkPhase::R4a,
            22,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let advertised =
            AdvertisedStep::from_snapshot(&displayed, false, false).expect("displayed Step");
        let fresh = advertised_snapshot(
            WalkPhase::R4a,
            23,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept stale Step health");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read stale Step health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            ipc::send(&mut stream, &WalkResponse::status(fresh, "online", epoch))
                .await
                .expect("write fresh Step status");

            let (mut stream, _) = listener.accept().await.expect("accept guarded Step socket");
            let error = tokio::time::timeout(
                Duration::from_millis(500),
                ipc::recv::<WalkRequest>(&mut stream),
            )
            .await
            .expect("stale Step must close guarded socket")
            .expect_err("stale Step must not frame a mutation");
            assert!(matches!(
                error,
                PrepareError::DatabaseSetup {
                    phase: "prototype1_state_walk_ipc_read",
                    ..
                }
            ));
        });
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve stale client");

        let error = client
            .step_advertised(&advertised, OperationId::for_test(124))
            .await
            .expect_err("changed version must fail")
            .to_string();
        assert!(error.contains("changed before submission"), "{error}");
        server.await.expect("stale Step server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn advertised_step_rejects_fresh_outcome_change_without_mutation_frame() {
        let repo = tempfile::tempdir().expect("changed advertised Step repo");
        let repo_root = repo.path().canonicalize().expect("canonical changed repo");
        let socket = repo.path().join("changed-advertised-step.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind changed Step socket");
        let epoch = sibling_epoch(&repo_root);
        let displayed = advertised_snapshot(
            WalkPhase::R4a,
            24,
            &[ControlEdge::R4aToR4b, ControlEdge::R4aToR4c],
        );
        let advertised =
            AdvertisedStep::from_snapshot(&displayed, false, false).expect("displayed Step");
        let mut fresh = displayed;
        fresh.actions.pop();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept changed Step health");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read changed Step health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            ipc::send(&mut stream, &WalkResponse::status(fresh, "online", epoch))
                .await
                .expect("write changed Step status");

            let (mut stream, _) = listener.accept().await.expect("accept guarded Step socket");
            let error = tokio::time::timeout(
                Duration::from_millis(500),
                ipc::recv::<WalkRequest>(&mut stream),
            )
            .await
            .expect("changed Step must close guarded socket")
            .expect_err("changed Step must not frame a mutation");
            assert!(matches!(
                error,
                PrepareError::DatabaseSetup {
                    phase: "prototype1_state_walk_ipc_read",
                    ..
                }
            ));
        });
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve changed client");

        let error = client
            .step_advertised(&advertised, OperationId::for_test(125))
            .await
            .expect_err("changed outcomes must fail")
            .to_string();
        assert!(error.contains("complete control outcomes"), "{error}");
        server.await.expect("changed Step server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn edge_step_uses_the_advertised_edge_and_observed_version() {
        let repo = tempfile::tempdir().expect("edge step repo");
        let repo_root = repo.path().canonicalize().expect("canonical edge repo");
        let socket = repo.path().join("edge-step.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind edge socket");
        let server_epoch = sibling_epoch(&repo_root);
        let operation = OperationId::for_test(103);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(8)),
            cursor: Some(
                Cursor::new(WalkPhase::R5, ContentHash::of("edge step position"))
                    .expect("valid edge cursor"),
            ),
            journal_revision: 12,
        };
        let server = {
            let epoch = server_epoch.clone();
            let expected = version.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept edge health");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read edge health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                let mut status = status_response(
                    epoch.clone(),
                    WalkPosition::Session {
                        version: expected.clone(),
                    },
                    WalkAuthority::Active,
                );
                let WalkResponse::Status { snapshot, .. } = &mut status else {
                    panic!("status helper must return status");
                };
                snapshot.actions.push(WalkAction {
                    kind: WalkActionKind::Step,
                    edge: Some(ControlEdge::R5ToR6),
                    target: Some(WalkPhase::R6),
                    enabled: true,
                    requires_live_api: true,
                    requires_git_changes: false,
                    blocker: None,
                });
                ipc::send(&mut stream, &status)
                    .await
                    .expect("write edge health");

                let (mut stream, _) = listener.accept().await.expect("accept edge request");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read edge request");
                let WalkRequestBody::Step {
                    guard,
                    until,
                    watch,
                    allow_live_api,
                    allow_git_changes,
                } = request.body
                else {
                    panic!("expected exact edge Step request");
                };
                assert_eq!(guard.operation, operation);
                assert_eq!(guard.expected, expected);
                assert_eq!(until, None);
                assert!(!watch);
                assert!(allow_live_api);
                assert!(!allow_git_changes);
                ipc::send(
                    &mut stream,
                    &job_response(
                        epoch,
                        operation,
                        expected,
                        WalkJobKind::Step,
                        WalkPhase::R5,
                        None,
                    ),
                )
                .await
                .expect("write edge response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve edge client");

        let response = client
            .step_edge(ControlEdge::R5ToR6, true, false, operation)
            .await
            .expect("submit exact edge");

        assert!(matches!(response, WalkResponse::Job { .. }));
        server.await.expect("edge server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn edge_step_rejects_a_branch_offer_before_submission() {
        let repo = tempfile::tempdir().expect("stale edge repo");
        let repo_root = repo.path().canonicalize().expect("canonical stale repo");
        let socket = repo.path().join("stale-edge.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind stale socket");
        let epoch = sibling_epoch(&repo_root);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(9)),
            cursor: Some(
                Cursor::new(WalkPhase::R4a, ContentHash::of("branch server position"))
                    .expect("valid new cursor"),
            ),
            journal_revision: 13,
        };
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept stale health");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read stale health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            let mut status = status_response(
                epoch,
                WalkPosition::Session { version },
                WalkAuthority::Active,
            );
            let WalkResponse::Status { snapshot, .. } = &mut status else {
                panic!("status helper must return status");
            };
            snapshot.actions.extend([
                WalkAction {
                    kind: WalkActionKind::Step,
                    edge: Some(ControlEdge::R4aToR4b),
                    target: Some(WalkPhase::R4b),
                    enabled: true,
                    requires_live_api: false,
                    requires_git_changes: false,
                    blocker: None,
                },
                WalkAction {
                    kind: WalkActionKind::Step,
                    edge: Some(ControlEdge::R4aToR4c),
                    target: Some(WalkPhase::R4c),
                    enabled: true,
                    requires_live_api: false,
                    requires_git_changes: false,
                    blocker: None,
                },
            ]);
            ipc::send(&mut stream, &status)
                .await
                .expect("write stale status");
            let (mut stream, _) =
                tokio::time::timeout(Duration::from_millis(500), listener.accept())
                    .await
                    .expect("edge client must open its guarded submission socket")
                    .expect("accept guarded submission socket");
            let error = tokio::time::timeout(
                Duration::from_millis(500),
                ipc::recv::<WalkRequest>(&mut stream),
            )
            .await
            .expect("stale edge client must close its guarded submission socket")
            .expect_err("stale edge must not send a framed mutation");
            assert!(
                matches!(
                    error,
                    PrepareError::DatabaseSetup {
                        phase: "prototype1_state_walk_ipc_read",
                        ..
                    }
                ),
                "unexpected closed-socket error: {error}"
            );
        });
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve stale client");

        let error = client
            .step_edge(
                ControlEdge::R4aToR4b,
                false,
                false,
                OperationId::for_test(104),
            )
            .await
            .expect_err("stale edge must fail closed")
            .to_string();

        assert!(error.contains("singleton helper"), "{error}");
        server.await.expect("stale edge server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn start_rejects_old_protocol_before_submission() {
        let repo = tempfile::tempdir().expect("old protocol repo");
        let repo_root = repo.path().canonicalize().expect("canonical protocol repo");
        let socket = repo.path().join("old-protocol.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind protocol socket");
        let mut epoch = sibling_epoch(&repo_root);
        epoch.protocol_version =
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION - 1;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept protocol health");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read protocol health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            ipc::send(
                &mut stream,
                &status_response(epoch, WalkPosition::NoSession, WalkAuthority::Active),
            )
            .await
            .expect("write old protocol health");
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .expect_err("old protocol must not receive a mutation");
        });
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve protocol client");
        let error = client
            .start(
                WalkStartConfig {
                    campaign: None,
                    repo_root: None,
                },
                WalkPhase::R3,
                false,
                OperationId::for_test(104),
            )
            .await
            .expect_err("old protocol must reject start")
            .to_string();
        assert!(error.contains("walk protocol mismatch"), "{error}");
        server.await.expect("old protocol server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn start_preserves_operation_across_handoff() {
        let repo = tempfile::tempdir().expect("handoff start repo");
        let repo_root = repo.path().canonicalize().expect("canonical handoff repo");
        let old_socket = repo.path().join("start-old.sock");
        let old_listener =
            tokio::net::UnixListener::bind(&old_socket).expect("bind start predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("start predecessor endpoint");
        old.activate().expect("activate start predecessor");
        let next_socket = repo.path().join("start-next.sock");
        let next_listener =
            tokio::net::UnixListener::bind(&next_socket).expect("bind start successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket)
            .expect("start successor endpoint");
        let operation = OperationId::for_test(105);
        let predecessor = {
            let old = old.clone();
            let next = next.clone();
            tokio::spawn(async move {
                let (mut stream, _) = old_listener
                    .accept()
                    .await
                    .expect("accept predecessor start health");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read predecessor start health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                next.take_over(Some(&old)).expect("publish start successor");
                drop(stream);
            })
        };
        let successor = {
            let epoch = ServerEpoch::capture(&repo_root).expect("capture start successor epoch");
            tokio::spawn(async move {
                let (mut stream, _) = next_listener
                    .accept()
                    .await
                    .expect("accept successor start health");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read successor start health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        epoch.clone(),
                        WalkPosition::NoSession,
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write successor start health");

                let (mut stream, _) = next_listener
                    .accept()
                    .await
                    .expect("accept successor start request");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read successor start request");
                let WalkRequestBody::Start { guard, .. } = request.body else {
                    panic!("expected start after handoff")
                };
                assert_eq!(guard.operation, operation);
                ipc::send(
                    &mut stream,
                    &job_response(
                        epoch,
                        operation,
                        SessionVersion::empty(),
                        WalkJobKind::Start,
                        WalkPhase::Empty,
                        Some(WalkPhase::R3),
                    ),
                )
                .await
                .expect("write successor start response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), None).expect("resolve following start client");
        let response = client
            .start(
                WalkStartConfig {
                    campaign: None,
                    repo_root: None,
                },
                WalkPhase::R3,
                false,
                operation,
            )
            .await
            .expect("start follows successor");
        assert!(matches!(
            response,
            WalkResponse::Job {
                job: WalkJobSnapshot { operation_id, .. },
                ..
            } if operation_id == operation
        ));
        predecessor.await.expect("start predecessor task");
        successor.await.expect("start successor task");
        next.cleanup().expect("cleanup start successor");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn operation_status_preserves_id_across_handoff() {
        let repo = tempfile::tempdir().expect("operation client repo");
        let repo_root = repo
            .path()
            .canonicalize()
            .expect("canonical operation repo");
        let old_socket = repo.path().join("operation-old.sock");
        let old_listener =
            tokio::net::UnixListener::bind(&old_socket).expect("bind operation predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("operation predecessor endpoint");
        old.activate().expect("activate operation predecessor");
        let next_socket = repo.path().join("operation-next.sock");
        let next_listener =
            tokio::net::UnixListener::bind(&next_socket).expect("bind operation successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket)
            .expect("operation successor endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture operation epoch");
        let operation = OperationId::for_test(103);
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(8)),
            cursor: Some(
                Cursor::new(WalkPhase::R7, ContentHash::of("operation position"))
                    .expect("valid operation cursor"),
            ),
            journal_revision: 12,
        };
        let predecessor = {
            let epoch = epoch.clone();
            let old = old.clone();
            let next = next.clone();
            let version = version.clone();
            tokio::spawn(async move {
                let (mut stream, _) = old_listener
                    .accept()
                    .await
                    .expect("accept operation health");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read operation health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        epoch,
                        WalkPosition::Session { version },
                        WalkAuthority::JobActive,
                    ),
                )
                .await
                .expect("write operation health");

                let (mut stream, _) = old_listener
                    .accept()
                    .await
                    .expect("accept predecessor operation status");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read predecessor operation status");
                assert!(matches!(
                    request.body,
                    WalkRequestBody::OperationStatus {
                        operation: observed
                    } if observed == operation
                ));
                next.take_over(Some(&old))
                    .expect("publish operation successor");
                drop(stream);
            })
        };
        let successor = {
            let epoch = ServerEpoch::capture(&repo_root).expect("capture successor epoch");
            let version = version.clone();
            tokio::spawn(async move {
                let (mut stream, _) = next_listener
                    .accept()
                    .await
                    .expect("accept successor operation status");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read successor operation status");
                assert!(matches!(
                    request.body,
                    WalkRequestBody::OperationStatus {
                        operation: observed
                    } if observed == operation
                ));
                ipc::send(
                    &mut stream,
                    &job_response(
                        epoch,
                        operation,
                        version,
                        WalkJobKind::Step,
                        WalkPhase::R7,
                        Some(WalkPhase::R8),
                    ),
                )
                .await
                .expect("write successor operation status");
            })
        };
        let client = WalkClient::resolve(Some(&repo_root), None)
            .expect("resolve following operation client");
        assert!(client.follows_endpoint());
        let response = client
            .operation_status(operation)
            .await
            .expect("follow operation handoff");
        assert!(matches!(
            response,
            WalkResponse::Job {
                job: WalkJobSnapshot { operation_id, .. },
                ..
            } if operation_id == operation
        ));
        predecessor.await.expect("operation predecessor task");
        successor.await.expect("operation successor task");
        next.cleanup().expect("cleanup operation successor");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn operation_status_survives_retired_predecessor_publication_gap() {
        let repo = tempfile::tempdir().expect("publication gap repo");
        let repo_root = repo
            .path()
            .canonicalize()
            .expect("canonical publication gap repo");
        let old_socket = repo.path().join("gap-old.sock");
        let old_listener =
            tokio::net::UnixListener::bind(&old_socket).expect("bind gap predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("gap predecessor endpoint");
        old.activate().expect("activate gap predecessor");
        let next_socket = repo.path().join("gap-next.sock");
        let next_listener =
            tokio::net::UnixListener::bind(&next_socket).expect("bind gap successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket.clone())
            .expect("gap successor endpoint");
        let client =
            WalkClient::resolve(Some(&repo_root), None).expect("resolve following gap client");
        assert!(client.follows_endpoint());

        drop(old_listener);
        old.cleanup().expect("retire gap predecessor");
        assert!(
            endpoint::load(&repo_root)
                .expect("inspect pointerless gap")
                .is_none(),
            "predecessor retirement must leave no published endpoint"
        );

        let operation = OperationId::for_test(106);
        let expected = SessionVersion {
            session_id: Some(SessionId::for_test(9)),
            cursor: Some(
                Cursor::new(WalkPhase::R12, ContentHash::of("gap predecessor position"))
                    .expect("valid predecessor cursor"),
            ),
            journal_revision: 18,
        };
        let committed = SessionVersion {
            session_id: expected.session_id,
            cursor: Some(
                Cursor::new(WalkPhase::R13b, ContentHash::of("gap handoff position"))
                    .expect("valid handoff cursor"),
            ),
            journal_revision: 19,
        };
        let successor_version = SessionVersion {
            session_id: Some(SessionId::for_test(10)),
            cursor: Some(
                Cursor::new(WalkPhase::R4c, ContentHash::of("gap successor position"))
                    .expect("valid successor cursor"),
            ),
            journal_revision: 4,
        };
        let receipt = WalkTransitionReceipt {
            phase_before: WalkPhase::R12,
            phase_after: WalkPhase::R13b,
            edges: vec![ControlEdge::R12ToR13b],
            version: committed,
            event_projection: WalkEventProjection::Recorded,
        };

        let publication = {
            let old = old.clone();
            let next = next.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                next.take_over(Some(&old))
                    .expect("publish successor after pointerless gap");
            })
        };
        let successor = {
            let epoch = ServerEpoch::capture(&repo_root).expect("capture gap successor epoch");
            let receipt = receipt.clone();
            tokio::spawn(async move {
                let (mut stream, _) = next_listener
                    .accept()
                    .await
                    .expect("accept successor health after gap");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read successor health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        epoch.clone(),
                        WalkPosition::Session {
                            version: successor_version,
                        },
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write successor health");

                let (mut stream, _) = next_listener
                    .accept()
                    .await
                    .expect("accept successor operation after gap");
                let request: WalkRequest = ipc::recv(&mut stream)
                    .await
                    .expect("read successor operation");
                assert!(matches!(
                    request.body,
                    WalkRequestBody::OperationStatus {
                        operation: observed
                    } if observed == operation
                ));
                let mut response = job_response(
                    epoch,
                    operation,
                    expected,
                    WalkJobKind::Step,
                    WalkPhase::R12,
                    Some(WalkPhase::R13b),
                );
                let WalkResponse::Job { job, message, .. } = &mut response else {
                    unreachable!("job response helper returns a job")
                };
                job.status = WalkJobStatus::Succeeded;
                job.phase_after = Some(WalkPhase::R13b);
                job.updated_at = "2026-07-26T00:00:01Z".to_string();
                job.finished_at = Some("2026-07-26T00:00:01Z".to_string());
                job.message = Some("handoff committed".to_string());
                job.receipt = Some(receipt);
                *message = "handoff committed".to_string();
                ipc::send(&mut stream, &response)
                    .await
                    .expect("write terminal operation after gap");
            })
        };

        let response = client
            .operation_status(operation)
            .await
            .expect("operation status follows pointerless handoff gap");
        let WalkResponse::Job { job, .. } = response else {
            panic!("expected terminal operation response")
        };
        assert_eq!(job.operation_id, operation);
        assert_eq!(job.status, WalkJobStatus::Succeeded);
        assert_eq!(job.receipt.as_ref(), Some(&receipt));
        assert_eq!(
            client.resolved_socket().expect("resolved successor socket"),
            next_socket
        );

        publication.await.expect("successor publication task");
        successor.await.expect("gap successor task");
        next.cleanup().expect("cleanup gap successor");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_allows_source_drift() {
        let repo = tempfile::tempdir().expect("stop client repo");
        let repo_root = repo.path().canonicalize().expect("canonical stop repo");
        let socket = repo.path().join("stop.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind stop socket");
        let mut server_epoch = sibling_epoch(&repo_root);
        server_epoch.source_status_hash = Some("server-source-drift".to_string());
        let server = {
            let server_epoch = server_epoch.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept stop health");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read stop health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch.clone(),
                        WalkPosition::NoSession,
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write stop health");

                let (mut stream, _) = listener.accept().await.expect("accept stop request");
                let request: WalkRequest = ipc::recv(&mut stream).await.expect("read stop request");
                assert!(matches!(request.body, WalkRequestBody::Stop));
                let client_epoch = request.client_epoch.expect("stop client epoch");
                assert_ne!(
                    client_epoch.source_status_hash,
                    server_epoch.source_status_hash
                );
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch,
                        WalkPosition::NoSession,
                        WalkAuthority::Stopping,
                    ),
                )
                .await
                .expect("write stop response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve stop client");
        let response = client.stop_idle_server().await.expect("stop idle server");
        assert!(matches!(
            response,
            WalkResponse::Status {
                snapshot: WalkSessionSnapshot {
                    authority: WalkAuthority::Stopping,
                    ..
                },
                ..
            }
        ));
        server.await.expect("stop server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stop_allows_protocol_drift() {
        let repo = tempfile::tempdir().expect("old stop repo");
        let repo_root = repo.path().canonicalize().expect("canonical old stop repo");
        let socket = repo.path().join("old-stop.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind old stop socket");
        let mut server_epoch = sibling_epoch(&repo_root);
        server_epoch.protocol_version =
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION - 1;
        let server = {
            let server_epoch = server_epoch.clone();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept old stop health");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read old stop health");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch.clone(),
                        WalkPosition::NoSession,
                        WalkAuthority::Active,
                    ),
                )
                .await
                .expect("write old stop health");

                let (mut stream, _) = listener.accept().await.expect("accept old stop request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read old stop request");
                assert!(matches!(request.body, WalkRequestBody::Stop));
                let client_epoch = request.client_epoch.expect("old stop client epoch");
                assert_ne!(client_epoch.protocol_version, server_epoch.protocol_version);
                ipc::send(
                    &mut stream,
                    &status_response(
                        server_epoch,
                        WalkPosition::NoSession,
                        WalkAuthority::Stopping,
                    ),
                )
                .await
                .expect("write old stop response");
            })
        };
        let client =
            WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve old stop client");
        let response = client
            .stop_idle_server()
            .await
            .expect("stop stale-protocol server");
        assert!(matches!(
            response,
            WalkResponse::Status {
                snapshot: WalkSessionSnapshot {
                    authority: WalkAuthority::Stopping,
                    ..
                },
                ..
            }
        ));
        server.await.expect("old stop server task");
    }

    fn config_snapshot(repo_root: &Path) -> WalkConfigSnapshot {
        let campaign_id = CampaignId::from("walk-config-fixture");
        let campaign_path = crate::campaign::campaign_manifest_path(&campaign_id)
            .expect("resolve campaign fixture path");
        let profile_path = campaign_path
            .parent()
            .expect("campaign fixture root")
            .join("prototype1/run-profile.toml");
        let admission_path = campaign_path
            .parent()
            .expect("campaign fixture root")
            .join("prototype1/setup-admission.json");
        let instances_root = repo_root.join("instances");
        let batches_root = repo_root.join("batches");
        let sources = vec![RegistryDatasetSource {
            key: Some("fixture".to_string()),
            path: repo_root.join("dataset.jsonl"),
            label: "fixture".to_string(),
            url: None,
        }];
        let mut manifest = CampaignManifest::new(campaign_id.clone());
        manifest.dataset_sources = sources.clone();
        manifest.model_id = Some("google/gemini-3.5-flash".to_string());
        manifest.route_source = Some(ModelRouteSource::DirectGoogle);
        manifest.instances_root = Some(instances_root.clone());
        manifest.batches_root = Some(batches_root.clone());
        let profile: RunProfileRecord = toml::from_str(
            r#"
schema_version = "prototype1-run-profile.v1"
name = "walk-config-fixture"
"#,
        )
        .expect("parse passive profile fixture");
        let runtime_toml = toml::to_string(&profile).expect("serialize passive profile fixture");
        let runtime: crate::cli::prototype1_state::profile::Prototype1RunProfile =
            toml::from_str(&runtime_toml).expect("parse runtime profile fixture");
        let manifest_hash = ContentHash::of(
            &serde_json::to_string_pretty(&manifest).expect("serialize campaign fixture"),
        );
        let profile_hash = ContentHash::of(
            &toml::to_string_pretty(&runtime).expect("normalize runtime profile fixture"),
        );
        let root_identity = ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: campaign_id.clone(),
            parent_id: "node-0".to_string(),
            node_id: "node-0".to_string(),
            generation: 0,
            instance_id: Some("fixture-instance".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-0".to_string(),
            artifact_branch: None,
            created_at: "2026-07-13T00:00:00Z".to_string(),
        };

        WalkConfigSnapshot {
            identity: ConfigIdentity {
                path: repo_root.join(".ploke/prototype1/parent_identity.json"),
                record: root_identity.clone(),
            },
            campaign: CampaignConfig {
                path: campaign_path.clone(),
                content_hash: manifest_hash.clone(),
                admission: ConfigAdmission {
                    path: admission_path,
                    plan_hash: ContentHash::of("setup plan"),
                    manifest_path: campaign_path,
                    setup_root: repo_root.to_path_buf(),
                    root_identity,
                    hashes: SetupArtifactHashes {
                        manifest: manifest_hash,
                        slice: ContentHash::of("batch slice"),
                        profile: profile_hash.clone(),
                    },
                    started_at: "2026-07-13T00:00:01+00:00".to_string(),
                    completed_head: GitCommit("completed-head".to_string()),
                },
                manifest,
                resolved: ResolvedCampaignConfig {
                    campaign_id,
                    benchmark_family: BenchmarkFamily::MultiSweBenchRust,
                    dataset_sources: sources,
                    model_id: "google/gemini-3.5-flash".to_string(),
                    provider_slug: None,
                    route_source: ModelRouteSource::DirectGoogle,
                    required_procedures: vec![
                        "tool-call-intent-segments".to_string(),
                        "tool-call-review".to_string(),
                        "tool-call-segment-review".to_string(),
                    ],
                    instances_root,
                    batches_root,
                    eval: EvalCampaignPolicy::default(),
                    protocol: ProtocolCampaignPolicy::default(),
                    framework: FrameworkConfig::default(),
                },
                provider: ProviderSelection::DirectGoogle,
            },
            profile: ProfileConfig {
                record: profile,
                commitment: RunProfileCommitmentRecord {
                    schema_version: "prototype1-run-profile-commitment.v1".to_string(),
                    profile_path: profile_path.clone(),
                    sha256: profile_hash.0,
                    source_path: None,
                    admitted_at: "2026-07-13T00:00:01+00:00".to_string(),
                },
                reported_source: None,
            },
            control: EffectiveControl {
                path: profile_path,
                mode: RunMode::Continuous,
                parallel_cap: SourcedValue {
                    value: 3,
                    source: ValueSource::Derived(DerivationRule::SearchFanout),
                },
                patch_cap: SourcedValue {
                    value: 3,
                    source: ValueSource::Derived(DerivationRule::DefaultPatchTargets),
                },
            },
        }
    }

    #[test]
    fn public_config_request_and_response_round_trip() {
        let repo = tempfile::tempdir().expect("config repo");
        let epoch = ServerEpoch::capture(repo.path()).expect("capture config epoch");
        let request = WalkRequest {
            client_protocol: None,
            client_epoch: None,
            body: WalkRequestBody::Config,
        };
        let response = WalkResponse::config(WalkPhase::R3, config_snapshot(repo.path()), epoch);

        let request_json = serde_json::to_value(&request).expect("serialize config request");
        let decoded_request: WalkRequest =
            serde_json::from_value(request_json.clone()).expect("decode config request");
        assert_eq!(
            serde_json::to_value(decoded_request).expect("reserialize config request"),
            request_json
        );

        let response_json = serde_json::to_value(&response).expect("serialize config response");
        let decoded_response: WalkResponse =
            serde_json::from_value(response_json.clone()).expect("decode config response");
        assert_eq!(decoded_response.phase(), Some(WalkPhase::R3));
        assert_eq!(
            serde_json::to_value(decoded_response).expect("reserialize config response"),
            response_json
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn config_returns_the_typed_server_projection() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("config.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let expected = config_snapshot(&repo_root);
        let response = expected.clone();
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch.clone()).await;
            let (mut stream, _) = listener.accept().await.expect("accept config request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read config request");
            assert!(matches!(request.body, WalkRequestBody::Config));
            ipc::send(
                &mut stream,
                &WalkResponse::config(WalkPhase::R3, response, epoch),
            )
            .await
            .expect("write config response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client.config().await.expect("read typed config");

        assert_eq!(
            serde_json::to_value(actual).expect("serialize actual config"),
            serde_json::to_value(expected).expect("serialize expected config")
        );
        server.await.expect("config server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn completed_evaluation_trace_round_trips_over_the_public_socket() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("trace.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let coordinate = EvaluationRunCoordinate {
            campaign: CampaignId::from("walk-trace-fixture"),
            instance: InstanceId("org__repo-1".to_string()),
            run_id: "run-trace-socket".to_string(),
        };
        let mut registration = RunRegistration::register_with_run_id(
            crate::inner::RunIntent {
                task_id: coordinate.instance.as_str().to_string(),
                repo_root: repo_root.clone(),
                storage_roots: crate::inner::RunStorageRoots::new(
                    tmp.path().join("registries"),
                    tmp.path().join("instances/org__repo-1/runs"),
                ),
                base_sha: None,
                budget: crate::spec::EvalBudget::default(),
                model_id: None,
                provider_slug: None,
                campaign_id: Some(coordinate.campaign.clone()),
                batch_id: None,
                run_arm_id: "shell-only".to_string(),
                run_role: crate::inner::core::RegisteredRunRole::Control,
            },
            coordinate.run_id.clone(),
        )
        .expect("build trace registration");
        registration.mark_completed();
        let run = serde_json::from_value(serde_json::json!({
            "schema_version": ploke_records::run_record::RUN_RECORD_SCHEMA_VERSION,
            "manifest_id": "manifest-fixture",
            "metadata": {
                "benchmark": {
                    "instance_id": coordinate.instance.as_str(),
                    "repo_root": repo_root,
                    "base_sha": null
                },
                "agent": {},
                "runtime": {},
                "budget": {
                    "max_turns": 1,
                    "max_tool_calls": 1,
                    "wall_clock_secs": 1
                }
            },
            "phases": {},
            "db_time_travel_index": []
        }))
        .expect("build trace run record");
        let snapshot = EvaluationTraceSnapshot {
            coordinate: coordinate.clone(),
            version: SessionVersion::empty(),
            epoch: epoch.clone(),
            authority: TraceAuthority::RunRegistry,
            trace: EvaluationTraceState::Completed {
                trace: CompletedEvaluationTrace {
                    registration: TraceEvidence {
                        value: registration,
                        source: TraceSource {
                            kind: TraceSourceKind::Registration,
                            path: tmp.path().join("registries/runs/run-trace-socket.json"),
                            content_sha256: "registration-hash".to_string(),
                        },
                    },
                    turn: None,
                    run: TraceEvidence {
                        value: run,
                        source: TraceSource {
                            kind: TraceSourceKind::RunRecord,
                            path: tmp.path().join("record.json.gz"),
                            content_sha256: "record-hash".to_string(),
                        },
                    },
                    exchanges: None,
                    protocol: Vec::new(),
                },
            },
        };
        let expected = serde_json::to_value(&snapshot).expect("serialize expected trace");
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch.clone()).await;
            let (mut stream, _) = listener.accept().await.expect("accept trace request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read trace request");
            assert_eq!(
                request.body,
                WalkRequestBody::EvaluationTrace { coordinate }
            );
            ipc::send(&mut stream, &WalkResponse::evaluation_trace(snapshot))
                .await
                .expect("write trace response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client
            .evaluation_trace(EvaluationRunCoordinate {
                campaign: CampaignId::from("walk-trace-fixture"),
                instance: InstanceId("org__repo-1".to_string()),
                run_id: "run-trace-socket".to_string(),
            })
            .await
            .expect("read typed trace");

        assert_eq!(
            serde_json::to_value(actual).expect("serialize actual trace"),
            expected
        );
        server.await.expect("trace server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn config_rejects_an_older_protocol_before_sending_the_typed_request() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("old-config.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let mut epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        epoch.protocol_version -= 1;
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            let next = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
            assert!(
                next.is_err(),
                "client sent a typed config request after a mismatched protocol probe"
            );
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .config()
            .await
            .expect_err("older protocol must be rejected before config decode");

        assert!(
            error.to_string().contains("walk protocol mismatch"),
            "unexpected protocol error: {error}"
        );
        server.await.expect("old protocol server task");
    }

    #[test]
    fn config_requires_the_current_protocol() {
        assert!(requires_current_protocol(&WalkRequestBody::Config));
        assert!(requires_current_protocol(
            &WalkRequestBody::EvaluationTraceIndex
        ));
        assert!(requires_current_protocol(
            &WalkRequestBody::EvaluationTrace {
                coordinate: EvaluationRunCoordinate {
                    campaign: ploke_records::ids::CampaignId::from("campaign"),
                    instance: InstanceId("instance".to_string()),
                    run_id: "run-1".to_string(),
                },
            }
        ));
    }

    #[test]
    fn db_query_requires_the_current_protocol() {
        assert!(requires_current_protocol(&WalkRequestBody::DbQuery {
            campaign: Some(CampaignId::from("query-protocol")),
            script: "::relations".to_string(),
        }));
    }

    #[test]
    fn evidence_query_requires_the_current_protocol() {
        assert!(requires_current_protocol(&WalkRequestBody::EvidenceQuery {
            campaign: CampaignId::from("query-protocol"),
            view: WalkEvidenceQuery::Progress,
        }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_rejects_an_older_protocol_before_sending_the_typed_request() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("old-query.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let mut epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        epoch.protocol_version -= 1;
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            let next = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
            assert!(
                next.is_err(),
                "client sent a typed query after a mismatched protocol probe"
            );
        });
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");
        let campaign = CampaignId::from("query-protocol");

        let error = client
            .query_db(Some(&campaign), "::relations")
            .await
            .expect_err("older protocol must be rejected before query submission")
            .to_string();

        assert!(error.contains("walk protocol mismatch"), "{error}");
        server.await.expect("old query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_accepts_the_exact_repository_and_campaign() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("exact-query.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-exact");
        let server = {
            let repo = repo.clone();
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert_eq!(
                    request.body,
                    WalkRequestBody::DbQuery {
                        campaign: Some(campaign.clone()),
                        script: "::relations".to_string(),
                    }
                );
                ipc::send(
                    &mut stream,
                    &raw_query_response(epoch, &repo, &campaign, "::relations"),
                )
                .await
                .expect("write query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let query = client
            .query_db(Some(&campaign), "::relations")
            .await
            .expect("exact query scope");

        assert_eq!(query.result.repo_root, repo);
        assert_eq!(query.result.campaign_id, campaign);
        server.await.expect("exact query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_without_campaign_validates_parent_identity_scope() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("inferred-raw-query.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let expected = CampaignId::from("query-parent-identity");
        let returned = CampaignId::from("query-wrong-scope");
        write_parent(&repo, expected.clone(), 0);
        let server = {
            let repo = repo.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert_eq!(
                    request.body,
                    WalkRequestBody::DbQuery {
                        campaign: None,
                        script: "::relations".to_string(),
                    }
                );
                ipc::send(
                    &mut stream,
                    &raw_query_response(epoch, &repo, &returned, "::relations"),
                )
                .await
                .expect("write wrong-scope query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let error = client
            .query_db(None, "::relations")
            .await
            .expect_err("server-inferred campaign must match parent identity")
            .to_string();

        assert!(
            error.contains("returned campaign 'query-wrong-scope'")
                && error.contains("expected requested campaign 'query-parent-identity'"),
            "{error}"
        );
        server.await.expect("inferred raw query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_rejects_a_different_returned_script() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("wrong-query-script.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-script");
        let server = {
            let repo = repo.clone();
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert!(matches!(request.body, WalkRequestBody::DbQuery { .. }));
                ipc::send(
                    &mut stream,
                    &raw_query_response(epoch, &repo, &campaign, "::columns"),
                )
                .await
                .expect("write wrong-script query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let error = client
            .query_db(Some(&campaign), "::relations")
            .await
            .expect_err("returned raw script must match exact request")
            .to_string();

        assert!(error.contains("exact raw request"), "{error}");
        server.await.expect("wrong-script query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn evidence_query_round_trip_preserves_typed_view_and_scope() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("named-query.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-named");
        let view = WalkEvidenceQuery::Progress;
        let server = {
            let repo = repo.clone();
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert_eq!(
                    request.body,
                    WalkRequestBody::EvidenceQuery {
                        campaign: campaign.clone(),
                        view,
                    }
                );
                let response = evidence_query_response(epoch, &repo, &campaign, view);
                ipc::send(&mut stream, &response)
                    .await
                    .expect("write named query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let query = client
            .query_evidence(Some(&campaign), view)
            .await
            .expect("exact named query scope");

        assert_eq!(query.result.repo_root, repo);
        assert_eq!(query.result.campaign_id, campaign);
        assert_eq!(query.result.view, Some(view));
        assert_eq!(query.result.script, evidence_query_script(view));
        server.await.expect("named query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn evidence_query_without_campaign_uses_parent_identity() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("inferred-query.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-parent-identity");
        write_parent(&repo, campaign.clone(), 0);
        let view = WalkEvidenceQuery::Progress;
        let server = {
            let repo = repo.clone();
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert_eq!(
                    request.body,
                    WalkRequestBody::EvidenceQuery {
                        campaign: campaign.clone(),
                        view,
                    }
                );
                let response = evidence_query_response(epoch, &repo, &campaign, view);
                ipc::send(&mut stream, &response)
                    .await
                    .expect("write named query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let query = client
            .query_evidence(None, view)
            .await
            .expect("parent-identity-derived query scope");

        assert_eq!(query.result.campaign_id, campaign);
        assert_eq!(query.result.view, Some(view));
        server.await.expect("inferred query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn evidence_query_rejects_a_mismatched_returned_view() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("wrong-query-view.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-view");
        let requested = WalkEvidenceQuery::Progress;
        let server = {
            let repo = repo.clone();
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert!(matches!(
                    request.body,
                    WalkRequestBody::EvidenceQuery {
                        view: WalkEvidenceQuery::Progress,
                        ..
                    }
                ));
                let mut response = evidence_query_response(epoch, &repo, &campaign, requested);
                let WalkResponse::Query { query } = &mut response else {
                    unreachable!("query fixture is a query response");
                };
                query.result.view = Some(WalkEvidenceQuery::Counts);
                ipc::send(&mut stream, &response)
                    .await
                    .expect("write wrong-view query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let error = client
            .query_evidence(Some(&campaign), requested)
            .await
            .expect_err("different returned view must fail")
            .to_string();

        assert!(
            error.contains("expected requested view Some(Progress)"),
            "{error}"
        );
        server.await.expect("wrong-view query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_rejects_a_different_result_repository() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        let other = tmp.path().join("other-parent");
        fs::create_dir_all(&repo).expect("create repo root");
        fs::create_dir_all(&other).expect("create other repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let other = paths::resolve_repo_root(Some(&other)).expect("resolve other repo root");
        let socket = tmp.path().join("wrong-query-root.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let campaign = CampaignId::from("query-root");
        let server = {
            let campaign = campaign.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert!(matches!(request.body, WalkRequestBody::DbQuery { .. }));
                ipc::send(
                    &mut stream,
                    &raw_query_response(epoch, &other, &campaign, "::relations"),
                )
                .await
                .expect("write wrong-root query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let error = client
            .query_db(Some(&campaign), "::relations")
            .await
            .expect_err("different query result repository must fail")
            .to_string();

        assert!(error.contains("returned repository"), "{error}");
        server.await.expect("wrong-root query server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn db_query_rejects_a_different_requested_campaign() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        fs::create_dir_all(&repo).expect("create repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve repo root");
        let socket = tmp.path().join("wrong-query-campaign.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo).expect("capture response epoch");
        let requested = CampaignId::from("query-requested");
        let returned = CampaignId::from("query-returned");
        let server = {
            let repo = repo.clone();
            tokio::spawn(async move {
                answer_protocol_probe(&listener, epoch.clone()).await;
                let (mut stream, _) = listener.accept().await.expect("accept query request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read query request");
                assert!(matches!(request.body, WalkRequestBody::DbQuery { .. }));
                ipc::send(
                    &mut stream,
                    &raw_query_response(epoch, &repo, &returned, "::relations"),
                )
                .await
                .expect("write wrong-campaign query response");
            })
        };
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        let error = client
            .query_db(Some(&requested), "::relations")
            .await
            .expect_err("different returned campaign must fail")
            .to_string();

        assert!(
            error.contains("returned campaign 'query-returned'"),
            "{error}"
        );
        server.await.expect("wrong-campaign query server task");
    }

    #[test]
    fn status_without_position_is_accepted_only_from_an_older_protocol() {
        let legacy_wire = serde_json::json!({
            "type": "status",
            "message": "online",
            "snapshot": {
                "phase": "r4c",
                "version": {
                    "session_id": null,
                    "cursor": null,
                    "journal_revision": 0
                },
                "controller_attached": true,
                "authority": "active",
                "job": null,
                "blocker": null,
                "actions": []
            },
            "epoch": {
                "protocol_version": 8,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "parent/runtime-1",
                "source_status_hash": "def456"
            }
        });
        let legacy: WalkResponse =
            serde_json::from_value(legacy_wire.clone()).expect("decode frozen v8 status");
        let WalkResponse::Status { snapshot, .. } = &legacy else {
            panic!("expected legacy status response");
        };
        assert!(matches!(&snapshot.position, WalkPosition::Legacy { .. }));
        validate_response_shape(&legacy).expect("protocol 8 may omit position authority");
        let rendered = crate::cli::prototype1_state::walk::client::render_response_json(&legacy)
            .expect("CLI must re-emit an accepted v8 status");
        let rendered: serde_json::Value =
            serde_json::from_str(&rendered).expect("rendered v8 status JSON");
        assert_eq!(rendered, legacy_wire);

        let mut invalid_wire = legacy_wire;
        invalid_wire["epoch"]["protocol_version"] =
            serde_json::json!(crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION);
        invalid_wire["epoch"]["build_fingerprint"] = serde_json::json!("a".repeat(64));
        let invalid: WalkResponse =
            serde_json::from_value(invalid_wire).expect("decode malformed current status");
        let error = validate_response_shape(&invalid)
            .expect_err("current protocol must identify position authority")
            .to_string();

        assert!(error.contains("status omitted its required position authority"));
    }

    #[test]
    fn public_reply_round_trip_preserves_all_protocol_fields() {
        let repo = tempfile::tempdir().expect("audit repo");
        let epoch = ServerEpoch {
            protocol_version: crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: repo.path().to_path_buf(),
            exe_path: repo.path().join("ploke-eval"),
            exe_modified_unix_ms: Some(17),
            git_head: Some("abc123".to_string()),
            active_branch: Some("successor/runtime-2".to_string()),
            source_status_hash: Some("def456".to_string()),
            build_fingerprint:
                crate::cli::prototype1_state::walk::epoch::current_build_fingerprint().to_string(),
        };
        let cursor =
            Cursor::new(WalkPhase::R6, ContentHash::of("r6 evidence")).expect("valid cursor");
        let version = SessionVersion {
            session_id: Some(SessionId::for_test(7)),
            cursor: Some(cursor),
            journal_revision: 11,
        };
        let job = WalkJobSnapshot {
            job_id: 23,
            operation_id: OperationId::for_test(29),
            expected: version.clone(),
            command: WalkJobKind::Step,
            status: WalkJobStatus::Running,
            phase_before: WalkPhase::R6,
            phase_after: None,
            target_phase: Some(WalkPhase::R7),
            watch: Some(true),
            allow_live_api: Some(false),
            allow_git_changes: Some(false),
            llm_source: None,
            allow_workspace_mutation: None,
            allow_provenance_record: None,
            started_at: "2026-07-13T00:00:00Z".to_string(),
            updated_at: "2026-07-13T00:00:01Z".to_string(),
            finished_at: None,
            message: Some("running controlled edge".to_string()),
            receipt: None,
            resolution: None,
        };
        let audit = crate::cli::prototype1_state::walk::audit::audit_r0_to_r1(
            repo.path().to_path_buf(),
            WalkPhase::R0,
            None,
            None,
        );
        let query: DbQueryResult = serde_json::from_value(serde_json::json!({
            "repo_root": repo.path(),
            "campaign_id": "walk-query-round-trip",
            "db_path": repo.path().join("owner.cozo"),
            "script": "::relations",
            "revision": "abc123",
            "headers": ["name"],
            "row_count": 1,
            "rows": [{"cells": ["eval_campaign"], "object": {"name": "eval_campaign"}}]
        }))
        .expect("query result carrier");
        let replies = vec![
            WalkResponse::Ok {
                phase: WalkPhase::R6,
                result: WalkOkPayload::Show {
                    report: "show".to_string(),
                },
                epoch: epoch.clone(),
            },
            WalkResponse::Audit {
                phase: WalkPhase::R0,
                report: audit,
                epoch: epoch.clone(),
            },
            WalkResponse::Query {
                query: WalkQuerySnapshot {
                    phase: WalkPhase::R6,
                    result: query,
                    version: version.clone(),
                    epoch: epoch.clone(),
                },
            },
            WalkResponse::Job {
                phase: WalkPhase::R6,
                job: job.clone(),
                message: "accepted".to_string(),
                epoch: epoch.clone(),
            },
            WalkResponse::Status {
                message: "online".to_string(),
                snapshot: WalkSessionSnapshot {
                    position: WalkPosition::Session {
                        version: version.clone(),
                    },
                    controller_attached: true,
                    authority: WalkAuthority::JobActive,
                    job: Some(job),
                    blocker: Some(WalkBlocker {
                        code: WalkBlockerCode::JobActive,
                        detail: "step is running".to_string(),
                    }),
                    actions: vec![WalkAction {
                        kind: WalkActionKind::Inspect,
                        edge: None,
                        target: None,
                        enabled: true,
                        requires_live_api: false,
                        requires_git_changes: false,
                        blocker: None,
                    }],
                },
                epoch: epoch.clone(),
            },
            WalkResponse::History {
                history: WalkSessionHistory::empty(
                    Some(repo.path().join("control-journal.jsonl")),
                    epoch.clone(),
                ),
            },
            WalkResponse::Delta {
                phase: WalkPhase::R6,
                report: "r5 to r6".to_string(),
                snapshot: WalkDeltaSnapshot {
                    version: version.clone(),
                    state: WalkDeltaState::Recorded {
                        from: WalkPhase::R5,
                        edges: vec![WalkEdgeDelta {
                            edge: ControlEdge::R5ToR6,
                            axes: WalkPhase::R6.axis_deltas_from(WalkPhase::R5),
                        }],
                    },
                },
                epoch: epoch.clone(),
            },
            WalkResponse::Error {
                code: WalkErrorCode::StaleVersion,
                detail: "expected revision 10".to_string(),
                phase: Some(WalkPhase::R6),
                version: Some(version),
                epoch,
            },
        ];

        for reply in replies {
            let encoded = serde_json::to_value(&reply).expect("serialize public reply");
            let decoded: WalkResponse =
                serde_json::from_value(encoded.clone()).expect("deserialize public reply");
            assert_eq!(
                serde_json::to_value(decoded).expect("reserialize public reply"),
                encoded
            );
        }
    }

    #[test]
    fn public_request_round_trip_preserves_guard_and_capabilities() {
        let repo = tempfile::tempdir().expect("request repo");
        let epoch = ServerEpoch {
            protocol_version: crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: repo.path().to_path_buf(),
            exe_path: repo.path().join("ploke-eval"),
            exe_modified_unix_ms: Some(19),
            git_head: Some("abc123".to_string()),
            active_branch: Some("parent/runtime-1".to_string()),
            source_status_hash: Some("def456".to_string()),
            build_fingerprint:
                crate::cli::prototype1_state::walk::epoch::current_build_fingerprint().to_string(),
        };
        let request = WalkRequest {
            client_protocol: Some(epoch.protocol_version),
            client_epoch: Some(epoch),
            body: WalkRequestBody::LlmStep {
                guard: MutationGuard {
                    operation: OperationId::for_test(31),
                    expected: SessionVersion::empty(),
                },
                session_id: Some("session-1".to_string()),
                lane: Some("lane-1".to_string()),
                step: Some(4),
                source: Prototype1StateWalkLlmStepSource::Live,
                watch: true,
                allow_workspace_mutation: true,
                model_id: Some("google/gemini-3.5-flash".to_string()),
                provider: Some("google".to_string()),
                max_attempts: 2,
                timeout_secs: 90,
            },
        };
        let bytes = serde_json::to_vec(&request).expect("serialize public request");
        let decoded: WalkRequest =
            serde_json::from_slice(&bytes).expect("deserialize public request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn public_job_resolution_request_round_trip_preserves_target_and_version() {
        let request = WalkRequest {
            client_protocol: None,
            client_epoch: None,
            body: WalkRequestBody::ResolveJob {
                guard: MutationGuard {
                    operation: OperationId::for_test(32),
                    expected: SessionVersion::empty(),
                },
                resolution: WalkJobResolutionKind::Abandon,
            },
        };

        let bytes = serde_json::to_vec(&request).expect("serialize resolution request");
        let decoded: WalkRequest =
            serde_json::from_slice(&bytes).expect("deserialize resolution request");
        assert_eq!(decoded, request);
    }

    #[test]
    fn public_session_history_request_round_trips() {
        let request = WalkRequest {
            client_protocol: None,
            client_epoch: None,
            body: WalkRequestBody::SessionHistory,
        };

        let bytes = serde_json::to_vec(&request).expect("serialize session-history request");
        let decoded: WalkRequest =
            serde_json::from_slice(&bytes).expect("deserialize session-history request");
        assert_eq!(decoded, request);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn session_history_returns_the_typed_server_projection() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("session-history.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let expected = WalkSessionHistory {
            journal_path: Some(repo_root.join("control-journal.jsonl")),
            version: SessionVersion::empty(),
            origin: None,
            profile: None,
            events: Vec::new(),
            damage: None,
            abandonment: None,
            epoch: epoch.clone(),
        };
        let response = expected.clone();
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch.clone()).await;
            let (mut stream, _) = listener.accept().await.expect("accept history request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read history request");
            assert!(matches!(request.body, WalkRequestBody::SessionHistory));
            ipc::send(&mut stream, &WalkResponse::history(response))
                .await
                .expect("write history response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client.session_history().await.expect("read typed history");

        assert_eq!(actual, expected);
        server.await.expect("history server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn session_history_rejects_incoherent_server_projection() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("invalid-session-history.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let mut invalid = WalkSessionHistory::empty(None, epoch.clone());
        invalid.version.journal_revision = 1;
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch.clone()).await;
            let (mut stream, _) = listener.accept().await.expect("accept history request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read history request");
            assert!(matches!(request.body, WalkRequestBody::SessionHistory));
            ipc::send(&mut stream, &WalkResponse::history(invalid))
                .await
                .expect("write invalid history response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .session_history()
            .await
            .expect_err("incoherent history must fail at the client boundary")
            .to_string();

        assert!(error.contains("undamaged session history"));
        server.await.expect("history server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_llm_index() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("llm-index.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let expected = LlmTraceIndex {
            version: SessionVersion::empty(),
            epoch: epoch.clone(),
            campaign: CampaignId::from("campaign-llm-index"),
            root: tmp.path().join("tool-loop"),
            authority: LlmTraceAuthority::ToolLoopCheckpoint,
            lanes: Vec::new(),
            issues: Vec::new(),
        };
        let response = expected.clone();
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            let (mut stream, _) = listener.accept().await.expect("accept LLM index request");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read LLM index request");
            assert!(matches!(request.body, WalkRequestBody::LlmTraceIndex));
            ipc::send(&mut stream, &WalkResponse::llm_trace_index(response))
                .await
                .expect("write LLM index response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client
            .llm_trace_index()
            .await
            .expect("read typed LLM index");

        assert_eq!(
            serde_json::to_value(actual).expect("serialize actual index"),
            serde_json::to_value(expected).expect("serialize expected index")
        );
        server.await.expect("LLM index server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_llm_trace() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("llm-trace.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let coordinate = LlmTraceCoordinate {
            session_id: "session-exact".to_string(),
            step: None,
        };
        let expected = test_llm_snapshot(tmp.path(), epoch.clone(), coordinate.clone());
        let response = expected.clone();
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            let (mut stream, _) = listener.accept().await.expect("accept LLM trace request");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read LLM trace request");
            assert_eq!(
                request.body,
                WalkRequestBody::LlmTrace {
                    coordinate: coordinate.clone()
                }
            );
            ipc::send(&mut stream, &WalkResponse::llm_trace(response))
                .await
                .expect("write LLM trace response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client
            .llm_trace(expected.coordinate.clone())
            .await
            .expect("read typed LLM trace");

        assert_eq!(
            serde_json::to_value(actual).expect("serialize actual trace"),
            serde_json::to_value(expected).expect("serialize expected trace")
        );
        server.await.expect("LLM trace server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_llm_trace_rejects_mismatch() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("llm-trace-mismatch.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let requested = LlmTraceCoordinate {
            session_id: "session-requested".to_string(),
            step: Some(2),
        };
        let returned = LlmTraceCoordinate {
            session_id: "session-returned".to_string(),
            step: Some(2),
        };
        let response = test_llm_snapshot(tmp.path(), epoch.clone(), returned);
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            let (mut stream, _) = listener.accept().await.expect("accept LLM trace request");
            let request: WalkRequest = ipc::recv(&mut stream)
                .await
                .expect("read LLM trace request");
            assert_eq!(
                request.body,
                WalkRequestBody::LlmTrace {
                    coordinate: requested.clone()
                }
            );
            ipc::send(&mut stream, &WalkResponse::llm_trace(response))
                .await
                .expect("write mismatched LLM trace response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .llm_trace(LlmTraceCoordinate {
                session_id: "session-requested".to_string(),
                step: Some(2),
            })
            .await
            .expect_err("mismatched exact-session response must fail")
            .to_string();

        assert!(error.contains("disagrees with requested"), "{error}");
        server.await.expect("LLM trace mismatch server task");
    }

    fn test_llm_snapshot(
        root: &Path,
        epoch: ServerEpoch,
        coordinate: LlmTraceCoordinate,
    ) -> LlmTraceSnapshot {
        let missing_path = root.join("resume.json");
        LlmTraceSnapshot {
            coordinate: coordinate.clone(),
            version: SessionVersion::empty(),
            epoch,
            authority: LlmTraceAuthority::ToolLoopCheckpoint,
            session: TraceEvidence {
                value: LlmSession {
                    session_id: coordinate.session_id,
                    lane_id: "lane-a".to_string(),
                    workspace: root.join("lane-a"),
                    model: Some("google/test".to_string()),
                    status: LlmSessionStatus::Paused,
                },
                source: TraceSource {
                    kind: TraceSourceKind::ToolLoopSession,
                    path: root.join("session.json"),
                    content_sha256: "a".repeat(64),
                },
            },
            resume: LlmArtifact::Missing {
                path: missing_path.clone(),
            },
            timeline: Vec::new(),
            selected: None,
            outer: LlmArtifact::Missing { path: missing_path },
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn transition_delta_returns_the_typed_server_projection() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("transition-delta.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let version = SessionVersion {
            session_id: None,
            cursor: Some(
                Cursor::new(WalkPhase::R4a, ContentHash::of("r4a evidence")).expect("valid cursor"),
            ),
            journal_revision: 1,
        };
        let expected = WalkDeltaSnapshot {
            version,
            state: WalkDeltaState::Recorded {
                from: WalkPhase::R3,
                edges: vec![WalkEdgeDelta {
                    edge: ControlEdge::R3ToR4a,
                    axes: WalkPhase::R4a.axis_deltas_from(WalkPhase::R3),
                }],
            },
        };
        let response = expected.clone();
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch.clone()).await;
            let (mut stream, _) = listener.accept().await.expect("accept delta request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read delta request");
            assert!(matches!(
                request.body,
                WalkRequestBody::ShowDelta {
                    verbose: false,
                    color: false
                }
            ));
            ipc::send(
                &mut stream,
                &WalkResponse::delta(WalkPhase::R4a, "r3 to r4a".to_string(), response, epoch),
            )
            .await
            .expect("write delta response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let actual = client.transition_delta().await.expect("read typed delta");

        assert_eq!(actual, expected);
        server.await.expect("delta server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_read_rejects_v6_before_sending_v9_request() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("v6-server.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let mut epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        epoch.protocol_version = 6;
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, epoch).await;
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
                "client sent a v9-only request after observing a v6 server"
            );
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .session_history()
            .await
            .expect_err("v6 server must be rejected before session-history request");

        assert!(error.to_string().contains(&format!(
            "client={} server=6",
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION
        )));
        server.await.expect("v6 server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_read_rejects_v6_successor_after_v9_probe() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let old_socket = tmp.path().join("v9-predecessor.sock");
        let old_listener = tokio::net::UnixListener::bind(&old_socket).expect("bind predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("predecessor endpoint");
        old.activate().expect("activate predecessor");
        let next_socket = tmp.path().join("v6-successor.sock");
        let next_listener = tokio::net::UnixListener::bind(&next_socket).expect("bind successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket)
            .expect("successor endpoint");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("following client");

        let v9_epoch = ServerEpoch::capture(&repo_root).expect("capture v9 epoch");
        let old_for_task = old.clone();
        let next_for_task = next.clone();
        let predecessor = tokio::spawn(async move {
            let (mut stream, _) = old_listener.accept().await.expect("accept protocol probe");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read protocol probe");
            assert!(matches!(request.body, WalkRequestBody::Health));
            next_for_task
                .take_over(Some(&old_for_task))
                .expect("publish successor before probe response");
            ipc::send(
                &mut stream,
                &WalkResponse::ok(WalkOkKind::Show, WalkPhase::Empty, "healthy", v9_epoch),
            )
            .await
            .expect("write v9 protocol response");
        });

        let mut v6_epoch = ServerEpoch::capture(&repo_root).expect("capture v6 epoch");
        v6_epoch.protocol_version = 6;
        let successor = tokio::spawn(async move {
            let (mut stream, _) = next_listener.accept().await.expect("accept typed request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read typed request");
            assert!(matches!(request.body, WalkRequestBody::SessionHistory));
            ipc::send(
                &mut stream,
                &WalkResponse::error(
                    WalkErrorCode::BadRequest,
                    "v6 successor received v9 request",
                    None,
                    v6_epoch,
                ),
            )
            .await
            .expect("write v6 typed response");
        });

        let error = client
            .session_history()
            .await
            .expect_err("v6 successor response must be rejected");

        assert!(error.to_string().contains(&format!(
            "client={} server=6",
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION
        )));
        predecessor.await.expect("predecessor task");
        successor.await.expect("successor task");
        next.cleanup().expect("cleanup successor endpoint");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_read_rejects_response_for_other_checkout() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        let other_root = tmp.path().join("other-parent");
        fs::create_dir_all(&repo_root).expect("create client repo root");
        fs::create_dir_all(&other_root).expect("create other repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve client root");
        let socket = tmp.path().join("wrong-checkout.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");
        let client_epoch = ServerEpoch::capture(&repo_root).expect("capture client epoch");
        let other_epoch = ServerEpoch::capture(&other_root).expect("capture other epoch");
        let server = tokio::spawn(async move {
            answer_protocol_probe(&listener, client_epoch).await;
            let (mut stream, _) = listener.accept().await.expect("accept typed request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read typed request");
            assert!(matches!(request.body, WalkRequestBody::SessionHistory));
            ipc::send(
                &mut stream,
                &WalkResponse::error(
                    WalkErrorCode::BadRequest,
                    "response from other checkout",
                    None,
                    other_epoch,
                ),
            )
            .await
            .expect("write other-checkout response");
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .session_history()
            .await
            .expect_err("response for another checkout must fail");

        assert!(error.to_string().contains("repository root mismatch"));
        server.await.expect("wrong-checkout server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn health_rejects_incompatible_server_epochs() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo = tmp.path().join("parent");
        let other = tmp.path().join("other-parent");
        fs::create_dir_all(&repo).expect("create client repo root");
        fs::create_dir_all(&other).expect("create other repo root");
        let repo = paths::resolve_repo_root(Some(&repo)).expect("resolve client root");
        let other = paths::resolve_repo_root(Some(&other)).expect("resolve other root");
        let socket = tmp.path().join("incompatible-health.sock");
        let listener = tokio::net::UnixListener::bind(&socket).expect("bind walk endpoint");

        let wrong_root = sibling_epoch(&other);
        let compatible = sibling_epoch(&repo);
        let mut source_drift = compatible.clone();
        source_drift.source_status_hash = Some("server-source-drift".to_string());
        let mut build_drift = compatible;
        let replacement = if build_drift.build_fingerprint.starts_with('0') {
            "1"
        } else {
            "0"
        };
        build_drift
            .build_fingerprint
            .replace_range(0..1, replacement);
        let server = tokio::spawn(async move {
            for epoch in [wrong_root, source_drift, build_drift] {
                let (mut stream, _) = listener.accept().await.expect("accept health request");
                let request: WalkRequest =
                    ipc::recv(&mut stream).await.expect("read health request");
                assert!(matches!(request.body, WalkRequestBody::Health));
                ipc::send(
                    &mut stream,
                    &status_response(epoch, WalkPosition::NoSession, WalkAuthority::Active),
                )
                .await
                .expect("write incompatible health response");
            }
        });
        let client = WalkClient::resolve(Some(&repo), Some(&socket)).expect("resolve client");

        for expected in [
            "repository root mismatch",
            "source epoch differs",
            "build fingerprint differs",
        ] {
            let error = client
                .health()
                .await
                .expect_err("incompatible health must fail closed")
                .to_string();
            assert!(error.contains("stale walk server"), "{error}");
            assert!(error.contains(expected), "{error}");
        }
        server.await.expect("incompatible health server task");
    }

    #[test]
    fn transition_delta_rejects_disconnected_or_forged_edges() {
        let version = SessionVersion {
            session_id: None,
            cursor: Some(
                Cursor::new(WalkPhase::R4a, ContentHash::of("r4a evidence")).expect("valid cursor"),
            ),
            journal_revision: 1,
        };
        let snapshot = WalkDeltaSnapshot {
            version,
            state: WalkDeltaState::Recorded {
                from: WalkPhase::R3,
                edges: vec![WalkEdgeDelta {
                    edge: ControlEdge::R3ToR4a,
                    axes: WalkPhase::R4a.axis_deltas_from(WalkPhase::R3),
                }],
            },
        };
        let value = serde_json::to_value(snapshot).expect("serialize valid delta");

        let mut disconnected = value.clone();
        disconnected["state"]["edges"][0]["edge"] =
            serde_json::Value::String("r4a_to_r4b".to_string());
        let error = serde_json::from_value::<WalkDeltaSnapshot>(disconnected)
            .expect_err("disconnected delta must fail");
        assert!(error.to_string().contains("starts at"));

        let mut forged_axes = value.clone();
        forged_axes["state"]["edges"][0]["axes"] = serde_json::json!([]);
        let error = serde_json::from_value::<WalkDeltaSnapshot>(forged_axes)
            .expect_err("forged typestate axes must fail");
        assert!(error.to_string().contains("typestate axes"));

        let mut unknown = value;
        unknown["unexpected"] = serde_json::Value::Bool(true);
        let error = serde_json::from_value::<WalkDeltaSnapshot>(unknown)
            .expect_err("unknown delta fields must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn transition_delta_rejects_middle_disconnect_and_final_version_mismatch() {
        let version = SessionVersion {
            session_id: None,
            cursor: Some(
                Cursor::new(WalkPhase::R4c, ContentHash::of("r4c evidence")).expect("valid cursor"),
            ),
            journal_revision: 3,
        };
        let snapshot = WalkDeltaSnapshot {
            version,
            state: WalkDeltaState::Recorded {
                from: WalkPhase::R3,
                edges: vec![
                    WalkEdgeDelta {
                        edge: ControlEdge::R3ToR4a,
                        axes: WalkPhase::R4a.axis_deltas_from(WalkPhase::R3),
                    },
                    WalkEdgeDelta {
                        edge: ControlEdge::R4aToR4b,
                        axes: WalkPhase::R4b.axis_deltas_from(WalkPhase::R4a),
                    },
                    WalkEdgeDelta {
                        edge: ControlEdge::R4bToR4c,
                        axes: WalkPhase::R4c.axis_deltas_from(WalkPhase::R4b),
                    },
                ],
            },
        };
        let value = serde_json::to_value(snapshot).expect("serialize valid delta");

        let mut disconnected = value.clone();
        disconnected["state"]["edges"][1]["edge"] =
            serde_json::Value::String("r4b_to_r4c".to_string());
        disconnected["state"]["edges"][1]["axes"] =
            serde_json::to_value(WalkPhase::R4c.axis_deltas_from(WalkPhase::R4b))
                .expect("serialize replacement axes");
        let error = serde_json::from_value::<WalkDeltaSnapshot>(disconnected)
            .expect_err("disconnected middle edge must fail");
        assert!(error.to_string().contains("starts at r4b, expected r4a"));

        let mut wrong_version = value;
        wrong_version["version"]["cursor"]["phase"] = serde_json::Value::String("r4b".to_string());
        let error = serde_json::from_value::<WalkDeltaSnapshot>(wrong_version)
            .expect_err("delta final phase must match the durable version");
        assert!(
            error
                .to_string()
                .contains("walk delta ends at r4c, but durable session version is at r4b")
        );
    }

    #[test]
    fn phase_inventory_separates_follow_from_effect_capabilities() {
        let inventory = PhaseInventory::current();
        let r5 = inventory
            .phases
            .iter()
            .find(|phase| phase.phase == WalkPhase::R5)
            .expect("R5 inventory");
        let baseline = r5.next.first().expect("R5 edge");
        assert!(baseline.requires_live_api);
        assert!(!baseline.requires_watch);

        let r12 = inventory
            .phases
            .iter()
            .find(|phase| phase.phase == WalkPhase::R12)
            .expect("R12 inventory");
        let handoff = r12
            .next
            .iter()
            .find(|step| step.phase == WalkPhase::R13b)
            .expect("handoff edge");
        assert!(handoff.requires_git_changes);
        assert!(!handoff.requires_live_api);
        assert!(!handoff.requires_watch);
    }

    #[test]
    fn phase_inventory_contains_every_walk_phase_once() {
        let phases = PhaseInventory::current()
            .phases
            .into_iter()
            .map(|phase| phase.phase)
            .collect::<Vec<_>>();

        assert_eq!(
            phases,
            vec![
                WalkPhase::Empty,
                WalkPhase::R0,
                WalkPhase::R1,
                WalkPhase::R2a,
                WalkPhase::R3,
                WalkPhase::R4a,
                WalkPhase::R4b,
                WalkPhase::R4c,
                WalkPhase::R5,
                WalkPhase::R6,
                WalkPhase::R7,
                WalkPhase::R8,
                WalkPhase::R9,
                WalkPhase::R10,
                WalkPhase::R11a,
                WalkPhase::R11,
                WalkPhase::R12,
                WalkPhase::R13a,
                WalkPhase::R13b,
                WalkPhase::R13c,
                WalkPhase::R14a,
                WalkPhase::R14b,
            ]
        );
    }

    #[test]
    fn resolve_for_run_uses_saved_context_socket_for_matching_run() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("worktrees").join("campaign-a");
        fs::create_dir_all(&repo_root).expect("create worktree root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("run").join("walk").join("selected.sock");
        paths::save_context(&paths::WalkContext {
            repo_root: repo_root.clone(),
            socket: Some(socket.clone()),
        })
        .expect("save walk context");

        let run = test_run_entry(tmp.path(), "campaign-a", Some(repo_root));
        let client = WalkClient::resolve_for_run(&run, None).expect("resolve client");

        assert_eq!(client.socket(), socket.as_path());
    }

    #[test]
    fn resolve_for_run_prefers_explicit_socket_over_saved_context() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("worktrees").join("campaign-a");
        fs::create_dir_all(&repo_root).expect("create worktree root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let context_socket = tmp.path().join("run").join("walk").join("context.sock");
        paths::save_context(&paths::WalkContext {
            repo_root: repo_root.clone(),
            socket: Some(context_socket),
        })
        .expect("save walk context");

        let explicit_socket = tmp.path().join("run").join("walk").join("explicit.sock");
        let run = test_run_entry(tmp.path(), "campaign-a", Some(repo_root));
        let client = WalkClient::resolve_for_run(&run, Some(&explicit_socket))
            .expect("resolve client with explicit socket");

        assert_eq!(client.socket(), explicit_socket.as_path());
    }

    #[cfg(unix)]
    #[test]
    fn client_follows_endpoint_after_successor_takeover() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let old_socket = tmp.path().join("old.sock");
        let _old_listener =
            std::os::unix::net::UnixListener::bind(&old_socket).expect("bind old endpoint");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket.clone())
            .expect("old endpoint");
        old.activate().expect("activate old endpoint");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("resolve following client");
        assert_eq!(client.resolved_socket().expect("old socket"), old_socket);

        let next_socket = tmp.path().join("next.sock");
        let _next_listener =
            std::os::unix::net::UnixListener::bind(&next_socket).expect("bind next endpoint");
        let next = endpoint::ServerEndpoint::from_bound(repo_root, next_socket.clone())
            .expect("next endpoint");
        next.take_over(Some(&old)).expect("successor takeover");

        assert_eq!(
            client.resolved_socket().expect("successor socket"),
            next_socket
        );
        next.cleanup().expect("cleanup next endpoint");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_only_request_retries_successor_after_midflight_handoff() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let old_socket = tmp.path().join("old-midflight.sock");
        let old_listener = tokio::net::UnixListener::bind(&old_socket).expect("bind predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("predecessor endpoint");
        old.activate().expect("activate predecessor");
        let next_socket = tmp.path().join("next-midflight.sock");
        let next_listener = tokio::net::UnixListener::bind(&next_socket).expect("bind successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket.clone())
            .expect("successor endpoint");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("following client");
        let old_for_task = old.clone();
        let next_for_task = next.clone();
        let predecessor = tokio::spawn(async move {
            let (mut stream, _) = old_listener
                .accept()
                .await
                .expect("accept predecessor request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read health request");
            assert!(matches!(request.body, WalkRequestBody::Health));
            next_for_task
                .take_over(Some(&old_for_task))
                .expect("publish successor during request");
            drop(stream);
        });
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let successor = tokio::spawn(async move {
            let (mut stream, _) = next_listener
                .accept()
                .await
                .expect("accept retried request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read retried health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            ipc::send(
                &mut stream,
                &WalkResponse::Ok {
                    phase: WalkPhase::R6,
                    result: WalkOkPayload::Show {
                        report: "successor response".to_string(),
                    },
                    epoch,
                },
            )
            .await
            .expect("write successor response");
        });

        let (observed_socket, response) = client
            .health_observation()
            .await
            .expect("health follows handoff")
            .expect("successor health response");
        assert_eq!(observed_socket, next_socket);
        assert!(matches!(
            response,
            WalkResponse::Ok {
                phase: WalkPhase::R6,
                result: WalkOkPayload::Show { .. },
                ..
            }
        ));
        predecessor.await.expect("predecessor task");
        successor.await.expect("successor task");
        next.cleanup().expect("cleanup successor endpoint");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_only_request_retries_same_path_endpoint_replacement() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let socket = tmp.path().join("shared-midflight.sock");
        let old_listener = tokio::net::UnixListener::bind(&socket).expect("bind predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), socket.clone())
            .expect("predecessor endpoint");
        old.activate().expect("activate predecessor");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("following client");
        let epoch = ServerEpoch::capture(&repo_root).expect("capture response epoch");
        let repo_for_task = repo_root.clone();
        let socket_for_task = socket.clone();
        let old_for_task = old.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = old_listener
                .accept()
                .await
                .expect("accept predecessor request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read health request");
            assert!(matches!(request.body, WalkRequestBody::Health));

            fs::remove_file(&socket_for_task).expect("unlink predecessor socket");
            let next_listener =
                tokio::net::UnixListener::bind(&socket_for_task).expect("bind same-path successor");
            let next = endpoint::ServerEndpoint::from_bound(repo_for_task, socket_for_task)
                .expect("successor endpoint");
            next.take_over(Some(&old_for_task))
                .expect("publish same-path successor");
            drop(stream);

            let (mut stream, _) = next_listener
                .accept()
                .await
                .expect("accept retried request");
            let request: WalkRequest = ipc::recv(&mut stream).await.expect("read retried health");
            assert!(matches!(request.body, WalkRequestBody::Health));
            ipc::send(
                &mut stream,
                &WalkResponse::Ok {
                    phase: WalkPhase::R6,
                    result: WalkOkPayload::Show {
                        report: "same-path successor response".to_string(),
                    },
                    epoch,
                },
            )
            .await
            .expect("write successor response");
            next
        });

        let response = client
            .health()
            .await
            .expect("health follows same-path handoff")
            .expect("successor health response");
        assert!(matches!(
            response,
            WalkResponse::Ok {
                phase: WalkPhase::R6,
                result: WalkOkPayload::Show { .. },
                ..
            }
        ));
        server
            .await
            .expect("same-path successor task")
            .cleanup()
            .expect("cleanup successor endpoint");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn explicit_socket_does_not_retry_midflight_handoff() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let old_socket = tmp.path().join("pinned-midflight.sock");
        let old_listener = tokio::net::UnixListener::bind(&old_socket).expect("bind predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket.clone())
            .expect("predecessor endpoint");
        old.activate().expect("activate predecessor");
        let next_socket = tmp.path().join("unused-successor.sock");
        let _next_listener = tokio::net::UnixListener::bind(&next_socket).expect("bind successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket)
            .expect("successor endpoint");
        let client = WalkClient::resolve(Some(&repo_root), Some(&old_socket))
            .expect("explicitly pinned client");
        let old_for_task = old.clone();
        let next_for_task = next.clone();
        let predecessor = tokio::spawn(async move {
            let (mut stream, _) = old_listener
                .accept()
                .await
                .expect("accept predecessor request");
            let _: WalkRequest = ipc::recv(&mut stream).await.expect("read health request");
            next_for_task
                .take_over(Some(&old_for_task))
                .expect("publish successor during request");
            drop(stream);
        });

        let error = client
            .health()
            .await
            .expect_err("explicit socket must remain pinned after midflight failure");
        assert!(
            error.to_string().contains("prototype1_state_walk_ipc_read")
                || error.to_string().contains("early eof"),
            "unexpected pinned-client error: {error}"
        );
        predecessor.await.expect("predecessor task");
        next.cleanup().expect("cleanup successor endpoint");
    }

    #[test]
    fn discovery_uses_the_completed_receipt_root_after_a_successor_handoff() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign = CampaignId::from("receipt-root");
        let manifest = write_campaign(tmp.path(), &campaign);
        let repo = tmp.path().join("setup-seeds/arbitrary-parent");
        fs::create_dir_all(&repo).expect("create arbitrary setup checkout");
        let repo = repo.canonicalize().expect("canonical setup checkout");
        let receipt = setup_admission::setup_admission_path(&manifest);
        write_receipt(
            &receipt,
            setup_intent(campaign.clone(), manifest, repo.clone()),
            true,
        );
        write_parent(&repo, campaign.clone(), 2);

        let runs = discover_walk_runs().expect("discover receipted run");
        let run = runs
            .iter()
            .find(|run| run.campaign_id == campaign.as_str())
            .expect("receipted campaign row");

        assert_eq!(run.worktree_root.as_deref(), Some(repo.as_path()));
        assert!(run.has_parent_identity);
    }

    #[test]
    fn discovery_ignores_a_conventional_worktree_without_a_receipt() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign = CampaignId::from("stale-fallback");
        write_campaign(tmp.path(), &campaign);
        let guessed = tmp.path().join("worktrees").join(campaign.as_str());
        fs::create_dir_all(&guessed).expect("create stale conventional checkout");
        write_parent(&guessed, campaign.clone(), 0);

        let runs = discover_walk_runs().expect("discover historical run");
        let run = runs
            .iter()
            .find(|run| run.campaign_id == campaign.as_str())
            .expect("historical campaign row");

        assert!(run.worktree_root.is_none());
        assert!(!run.has_parent_identity);
    }

    #[test]
    fn discovery_leaves_incomplete_or_mismatched_receipts_unbound() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);

        let incomplete = CampaignId::from("receipt-incomplete");
        let manifest = write_campaign(tmp.path(), &incomplete);
        let repo = tmp.path().join("setup-seeds/incomplete");
        fs::create_dir_all(&repo).expect("create incomplete checkout");
        let repo = repo.canonicalize().expect("canonical incomplete checkout");
        let receipt = setup_admission::setup_admission_path(&manifest);
        write_receipt(
            &receipt,
            setup_intent(incomplete.clone(), manifest, repo.clone()),
            false,
        );
        write_parent(&repo, incomplete.clone(), 0);

        let wrong_campaign = CampaignId::from("receipt-wrong-campaign");
        let manifest = write_campaign(tmp.path(), &wrong_campaign);
        let repo = tmp.path().join("setup-seeds/wrong-campaign");
        fs::create_dir_all(&repo).expect("create wrong-campaign checkout");
        let repo = repo
            .canonicalize()
            .expect("canonical wrong-campaign checkout");
        let receipt = setup_admission::setup_admission_path(&manifest);
        write_receipt(
            &receipt,
            setup_intent(
                CampaignId::from("receipt-other-campaign"),
                manifest,
                repo.clone(),
            ),
            true,
        );
        write_parent(&repo, wrong_campaign.clone(), 0);

        let wrong_manifest = CampaignId::from("receipt-wrong-manifest");
        let manifest = write_campaign(tmp.path(), &wrong_manifest);
        let repo = tmp.path().join("setup-seeds/wrong-manifest");
        fs::create_dir_all(&repo).expect("create wrong-manifest checkout");
        let repo = repo
            .canonicalize()
            .expect("canonical wrong-manifest checkout");
        let receipt = setup_admission::setup_admission_path(&manifest);
        let other_manifest = tmp.path().join("other/campaign.json");
        write_receipt(
            &receipt,
            setup_intent(wrong_manifest.clone(), other_manifest, repo.clone()),
            true,
        );
        write_parent(&repo, wrong_manifest.clone(), 0);

        let wrong_identity = CampaignId::from("receipt-wrong-identity");
        let manifest = write_campaign(tmp.path(), &wrong_identity);
        let repo = tmp.path().join("setup-seeds/wrong-identity");
        fs::create_dir_all(&repo).expect("create wrong-identity checkout");
        let repo = repo
            .canonicalize()
            .expect("canonical wrong-identity checkout");
        let receipt = setup_admission::setup_admission_path(&manifest);
        write_receipt(
            &receipt,
            setup_intent(wrong_identity.clone(), manifest, repo.clone()),
            true,
        );
        write_parent(&repo, CampaignId::from("identity-other-campaign"), 1);

        let runs = discover_walk_runs().expect("discover unbound receipt rows");
        for campaign in [incomplete, wrong_campaign, wrong_manifest, wrong_identity] {
            let run = runs
                .iter()
                .find(|run| run.campaign_id == campaign.as_str())
                .expect("mismatched campaign row");
            assert!(run.worktree_root.is_none(), "{campaign}");
            assert!(!run.has_parent_identity, "{campaign}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn explicit_socket_override_remains_pinned() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let pinned_socket = tmp.path().join("pinned.sock");
        let pinned = WalkClient::resolve(Some(&repo_root), Some(&pinned_socket))
            .expect("resolve pinned client");
        let active_socket = tmp.path().join("active.sock");
        let _active_listener =
            std::os::unix::net::UnixListener::bind(&active_socket).expect("bind active endpoint");
        let active = endpoint::ServerEndpoint::from_bound(repo_root, active_socket)
            .expect("active endpoint");
        active.activate().expect("activate endpoint");

        assert_eq!(
            pinned.resolved_socket().expect("pinned socket"),
            pinned_socket
        );
        active.cleanup().expect("cleanup active endpoint");
    }

    fn test_run_entry(
        eval_home: &Path,
        campaign_id: &str,
        worktree_root: Option<PathBuf>,
    ) -> WalkRunEntry {
        let campaign_dir = eval_home.join("campaigns").join(campaign_id);
        let prototype1_root = campaign_dir.join("prototype1");
        WalkRunEntry {
            campaign_id: campaign_id.to_string(),
            campaign_dir,
            prototype1_root,
            worktree_root,
            owner_db_path: eval_home
                .join("campaigns")
                .join(campaign_id)
                .join("prototype1")
                .join("owner.cozo.sqlite"),
            modified_unix_ms: None,
            has_manifest: true,
            has_closure_state: true,
            has_owner_db: false,
            has_parent_identity: true,
        }
    }
}
