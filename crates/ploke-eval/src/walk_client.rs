//! Public read-only client helpers for the Prototype 1 walk server.
//!
//! This is a UI-facing facade over the private `loop walk` protocol. It does
//! not own transition semantics and it does not make database mirrors
//! authoritative.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

pub use ploke_records::{
    identity::ParentIdentityRecord,
    ids::RuntimeId,
    run_profile::{RunMode, RunProfileCommitmentRecord, RunProfileRecord},
};

use crate::{
    cli::prototype1_state::{
        eval_store::prototype1_eval_store_db_path,
        identity,
        walk::{endpoint, ipc, paths},
    },
    layout::{campaigns_dir, ploke_eval_home},
    spec::PrepareError,
};

pub use crate::cli::{
    Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
    Prototype1StateWalkLlmStepSource,
};
pub use ploke_records::ids::GitCommit;

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
        phase::WalkPhase,
        protocol::{
            MutationGuard, OperationId, SessionVersion, WalkAction, WalkActionKind,
            WalkAttemptIntent, WalkAttemptReceipt, WalkAttemptResult, WalkAuthority, WalkBlocker,
            WalkBlockerCode, WalkCursor, WalkCursorEvidence, WalkDeltaSnapshot, WalkDeltaState,
            WalkEdgeDelta, WalkEndpoint, WalkEpochReceipt, WalkErrorCode, WalkEventProjection,
            WalkHandoffAcceptance, WalkJobKind, WalkJobResolutionKind, WalkJobResolutionReceipt,
            WalkJobSnapshot, WalkJobStatus, WalkOkKind, WalkOkPayload, WalkPredecessorAttempt,
            WalkQuerySnapshot, WalkReadyCommit, WalkReadyReceipt, WalkRecoveryResolution,
            WalkRequest, WalkRequestBody, WalkResponse, WalkRunMode, WalkSessionAbandonment,
            WalkSessionDamage, WalkSessionEvent, WalkSessionEventKind, WalkSessionHistory,
            WalkSessionOrigin, WalkSessionSnapshot, WalkStartConfig, WalkTransitionReceipt,
        },
        query::{DbQueryResult, DbQueryRow, ReadRevision},
    },
};

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
                    "run '{}' has no worktree under the local ploke-eval home",
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
        self.exchange_read_only(WalkRequestBody::Health).await
    }

    /// Read the current in-memory walk state from the server.
    pub async fn show(&self) -> Result<WalkResponse, PrepareError> {
        self.send_read_only(WalkRequestBody::Show).await
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

    /// Run an immutable query through the walk service against one exact owner snapshot.
    pub async fn query_db(
        &self,
        campaign: Option<&str>,
        script: &str,
    ) -> Result<WalkQuerySnapshot, PrepareError> {
        let campaign = campaign
            .filter(|text| !text.trim().is_empty())
            .map(|text| ploke_records::ids::CampaignId::from(text.trim()));
        match self
            .send_read_only(WalkRequestBody::DbQuery {
                campaign,
                script: script.to_string(),
            })
            .await?
        {
            WalkResponse::Query { query } => Ok(query),
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
                client_epoch: None,
                body: body.clone(),
            };
            let result = match ipc::send(&mut stream, &request).await {
                Ok(()) => ipc::recv(&mut stream).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(response) => return Ok(Some((socket, response))),
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

    async fn wait_for_successor(
        &self,
        prior_socket: &Path,
        prior_endpoint: Option<&endpoint::ServerEndpoint>,
    ) -> Result<bool, PrepareError> {
        for _ in 0..5 {
            let (socket, endpoint) = self.endpoint_observation()?;
            if socket != prior_socket || endpoint.as_ref() != prior_endpoint {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (socket, endpoint) = self.endpoint_observation()?;
        Ok(socket != prior_socket || endpoint.as_ref() != prior_endpoint)
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
                    tokio::time::sleep(Duration::from_millis(20)).await;
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
            | WalkRequestBody::SessionHistory
            | WalkRequestBody::ShowDelta { .. }
    )
}

fn ensure_current_protocol(response: &WalkResponse, repo_root: &Path) -> Result<(), PrepareError> {
    let epoch = response.epoch();
    let server = epoch.protocol_version;
    if server == crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION {
        if epoch.repo_root == repo_root {
            return Ok(());
        }
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_protocol",
            detail: format!(
                "walk response checkout '{}' does not match client checkout '{}'",
                epoch.repo_root.display(),
                repo_root.display()
            ),
        });
    }
    Err(PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_protocol",
        detail: format!(
            "walk protocol mismatch: client={} server={server}; restart the walk server before using this typed read",
            crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
        ),
    })
}

/// Discover Prototype 1 campaign roots under `ploke_eval_home()/campaigns`.
pub fn discover_walk_runs() -> Result<Vec<WalkRunEntry>, PrepareError> {
    let eval_home = ploke_eval_home()?;
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
        let manifest_path = campaign_dir.join("campaign.json");
        let worktree_candidate = eval_home.join("worktrees").join(&campaign_id);
        let worktree_root = worktree_candidate.is_dir().then_some(worktree_candidate);
        let owner_db_path = prototype1_eval_store_db_path(&manifest_path);
        let has_parent_identity = worktree_root
            .as_deref()
            .map(identity::parent_identity_path)
            .is_some_and(|path| path.exists());
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

    use crate::cli::prototype1_state::event::ContentHash;
    use crate::{
        BenchmarkFamily, CampaignManifest, EvalCampaignPolicy, FrameworkConfig,
        ProtocolCampaignPolicy, RegistryDatasetSource, ResolvedCampaignConfig,
    };

    use super::*;

    #[cfg(unix)]
    async fn answer_protocol_probe(listener: &tokio::net::UnixListener, epoch: ServerEpoch) {
        let (mut stream, _) = listener.accept().await.expect("accept protocol probe");
        let request: WalkRequest = ipc::recv(&mut stream).await.expect("read protocol probe");
        assert!(matches!(request.body, WalkRequestBody::Health));
        ipc::send(
            &mut stream,
            &WalkResponse::ok(WalkOkKind::Show, WalkPhase::Empty, "healthy", epoch),
        )
        .await
        .expect("write protocol probe");
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
    }

    #[test]
    fn public_reply_round_trip_preserves_all_protocol_fields() {
        let repo = tempfile::tempdir().expect("audit repo");
        let epoch = ServerEpoch {
            protocol_version: 7,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: repo.path().to_path_buf(),
            exe_path: repo.path().join("ploke-eval"),
            exe_modified_unix_ms: Some(17),
            git_head: Some("abc123".to_string()),
            active_branch: Some("successor/runtime-2".to_string()),
            source_status_hash: Some("def456".to_string()),
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
                    phase: WalkPhase::R6,
                    version: version.clone(),
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
            protocol_version: 7,
            transition_graph_version: "walk-r0-r14a-v2".to_string(),
            repo_root: repo.path().to_path_buf(),
            exe_path: repo.path().join("ploke-eval"),
            exe_modified_unix_ms: Some(19),
            git_head: Some("abc123".to_string()),
            active_branch: Some("parent/runtime-1".to_string()),
            source_status_hash: Some("def456".to_string()),
        };
        let request = WalkRequest {
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
    async fn typed_read_rejects_v6_before_sending_v7_request() {
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
                "client sent a v7-only request after observing a v6 server"
            );
        });
        let client = WalkClient::resolve(Some(&repo_root), Some(&socket)).expect("resolve client");

        let error = client
            .session_history()
            .await
            .expect_err("v6 server must be rejected before session-history request");

        assert!(error.to_string().contains("client=7 server=6"));
        server.await.expect("v6 server task");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn typed_read_rejects_v6_successor_after_v7_probe() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let repo_root = tmp.path().join("parent");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let repo_root = paths::resolve_repo_root(Some(&repo_root)).expect("resolve repo root");
        let old_socket = tmp.path().join("v7-predecessor.sock");
        let old_listener = tokio::net::UnixListener::bind(&old_socket).expect("bind predecessor");
        let old = endpoint::ServerEndpoint::from_bound(repo_root.clone(), old_socket)
            .expect("predecessor endpoint");
        old.activate().expect("activate predecessor");
        let next_socket = tmp.path().join("v6-successor.sock");
        let next_listener = tokio::net::UnixListener::bind(&next_socket).expect("bind successor");
        let next = endpoint::ServerEndpoint::from_bound(repo_root.clone(), next_socket)
            .expect("successor endpoint");
        let client = WalkClient::resolve(Some(&repo_root), None).expect("following client");

        let v7_epoch = ServerEpoch::capture(&repo_root).expect("capture v7 epoch");
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
                &WalkResponse::ok(WalkOkKind::Show, WalkPhase::Empty, "healthy", v7_epoch),
            )
            .await
            .expect("write v7 protocol response");
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
                    "v6 successor received v7 request",
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

        assert!(error.to_string().contains("client=7 server=6"));
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

        assert!(error.to_string().contains("does not match client checkout"));
        server.await.expect("wrong-checkout server task");
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
