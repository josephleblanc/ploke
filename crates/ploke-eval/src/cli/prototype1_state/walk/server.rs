//! Long-running local typestate walk server.
//!
//! The server binds one Unix socket and owns one `WalkController`. Live
//! `start`/`step` requests are admitted as supervised background jobs so the
//! socket can keep answering `status`/health and reject duplicate live
//! mutations while the controller is busy.
//!
//! The controller remains the only owner of typestate transitions. The server
//! job registry is intentionally just operational state: it reports what is
//! running and prevents accidental second attempts against the same parent
//! checkout.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::{fs::FileTypeExt, net::UnixStream as StdUnixStream};

#[cfg(target_os = "linux")]
use std::os::{
    fd::{AsRawFd, FromRawFd},
    unix::ffi::OsStringExt,
};

use chrono::Utc;

use tokio::{
    net::{UnixListener, UnixStream},
    sync::Mutex,
    task::JoinHandle,
    time,
};
use tracing::{debug, info, warn};

use crate::{
    campaign::campaign_manifest_path,
    cli::{
        Prototype1StateWalkServeCommand,
        prototype1_state::{
            driver::control::{
                PredecessorRelease, RecoveryDirective, ServerAdmission,
                recover_admitted_controller, recover_admitted_version, walk_server_admission,
            },
            event::RuntimeId,
            identity,
            invocation::{ProcessIncarnation, process_incarnation},
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            session::Store,
            successor,
        },
    },
    durable_io,
    layout::campaigns_dir,
    spec::PrepareError,
};

use super::{
    controller::{DeltaRenderStyle, WalkController},
    endpoint::{self, ServerEndpoint},
    epoch::ServerEpoch,
    ipc, paths,
    phase::WalkPhase,
    protocol::{
        MutationGuard, OperationId, SessionVersion, WalkJobSnapshot, WalkJobStatus, WalkRequest,
        WalkRequestBody, WalkResponse, WalkStartConfig,
    },
};

/// Runtime state owned by one server process.
struct WalkServer {
    epoch: ServerEpoch,
    controller: Arc<Mutex<WalkController>>,
    jobs: Arc<Mutex<JobRegistry>>,
    gate: MutationGate,
    operation_root: PathBuf,
}

/// Shared barrier that keeps a successor endpoint inspectable before the
/// predecessor has durably released mutation authority.
#[derive(Debug, Clone)]
pub(crate) struct MutationGate {
    open: Arc<AtomicBool>,
}

impl MutationGate {
    pub(crate) fn open() -> Self {
        Self {
            open: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn closed() -> Self {
        Self {
            open: Arc::new(AtomicBool::new(false)),
        }
    }

    fn allow(&self, _release: PredecessorRelease) {
        self.open.store(true, Ordering::Release);
    }

    fn allows_mutation(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

/// Already-bound server endpoint. Successor startup can expose this listener
/// before publishing Ready, then activate mutation only after transfer.
pub(crate) struct PreparedServer {
    listener: UnixListener,
    endpoint: ServerEndpoint,
    epoch: ServerEpoch,
    idle_ttl: Option<Duration>,
    publish: bool,
}

impl PreparedServer {
    pub(crate) fn endpoint(&self) -> &ServerEndpoint {
        &self.endpoint
    }
}

#[derive(Default)]
struct JobRegistry {
    next_id: u64,
    active: Option<ActiveWalkJob>,
    completed: BTreeMap<OperationId, StoredOperation>,
    stopping: bool,
}

struct ActiveWalkJob {
    snapshot: WalkJobSnapshot,
    fingerprint: Vec<u8>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct StoredOperation {
    snapshot: WalkJobSnapshot,
    fingerprint: Vec<u8>,
}

const OPERATION_SCHEMA_VERSION: &str = "prototype1-walk-operation.v1";

#[derive(serde::Serialize, serde::Deserialize)]
struct DurableOperation {
    schema_version: String,
    epoch: ServerEpoch,
    stored: StoredOperation,
}

enum JobAdmission {
    Accepted(WalkJobSnapshot),
    Duplicate(WalkJobSnapshot),
    Rejected(WalkResponse),
}

struct WalkEventInput {
    command: &'static str,
    phase_before: Option<WalkPhase>,
    phase_after: WalkPhase,
    target_phase: Option<WalkPhase>,
    watch: Option<bool>,
    allow_live_api: Option<bool>,
    allow_git_changes: Option<bool>,
    transitions: Vec<String>,
}

/// Run the server until it receives `Stop`, the listener fails, or idle TTL
/// expires while no admitted job is active.
pub(crate) async fn serve(command: Prototype1StateWalkServeCommand) -> Result<(), PrepareError> {
    let mut prepared = prepare(command, false)?;
    let admission = walk_server_admission(prepared.endpoint.repo_root())?;
    let gate = match admission {
        ServerAdmission::Open => {
            prepared.publish = true;
            MutationGate::open()
        }
        ServerAdmission::Transferred(release) => {
            prepared.publish = true;
            let gate = MutationGate::closed();
            gate.allow(release);
            gate
        }
        ServerAdmission::Pending => MutationGate::closed(),
    };
    serve_prepared(prepared, gate).await
}

/// Bind a distinct successor endpoint without replacing the predecessor's
/// durable client pointer.
pub(crate) fn prepare_successor(
    repo_root: &Path,
    runtime_id: crate::cli::prototype1_state::event::RuntimeId,
) -> Result<PreparedServer, PrepareError> {
    prepare(
        Prototype1StateWalkServeCommand {
            repo_root: Some(repo_root.to_path_buf()),
            socket: Some(paths::successor_socket(repo_root, runtime_id)?),
            ttl_secs: None,
            no_ttl: true,
        },
        false,
    )
}

/// Probe whether the deterministic socket slot named by persisted Ready
/// evidence is currently served. A restarted successor may own a new inode at
/// the same runtime-scoped path without minting a second Ready receipt.
pub(crate) fn endpoint_reachable(endpoint: &ServerEndpoint) -> Result<bool, PrepareError> {
    endpoint.validate_persisted()?;
    socket_reachable(endpoint.socket())
}

/// Publish the successor endpoint and open its mutation gate using the same
/// already-validated predecessor release proof.
pub(crate) fn activate_successor(
    endpoint: &ServerEndpoint,
    predecessor: Option<&ServerEndpoint>,
    gate: &MutationGate,
    release: PredecessorRelease,
) -> Result<(), PrepareError> {
    endpoint.take_over(predecessor)?;
    gate.allow(release);
    Ok(())
}

fn prepare(
    command: Prototype1StateWalkServeCommand,
    publish: bool,
) -> Result<PreparedServer, PrepareError> {
    let idle_ttl = command.idle_ttl()?;
    let repo_root = paths::resolve_repo_root(command.repo_root.as_deref())?;
    let socket_path = paths::socket_path(&repo_root, command.socket.as_deref())?;
    paths::ensure_socket_parent(&socket_path)?;
    clear_stale(&repo_root, &socket_path)?;
    let listener =
        UnixListener::bind(&socket_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_bind",
            detail: format!(
                "failed to bind walk socket '{}': {source}",
                socket_path.display()
            ),
        })?;
    let endpoint = ServerEndpoint::from_bound(repo_root.clone(), socket_path.clone())?;
    let epoch = ServerEpoch::capture(&repo_root)?;
    info!(
        socket = %socket_path.display(),
        repo_root = %repo_root.display(),
        idle_ttl_secs = ?idle_ttl.map(|ttl| ttl.as_secs()),
        "walk server listening"
    );
    Ok(PreparedServer {
        listener,
        endpoint,
        epoch,
        idle_ttl,
        publish,
    })
}

/// Serve an already-bound endpoint with a shared transfer gate.
pub(crate) async fn serve_prepared(
    prepared: PreparedServer,
    gate: MutationGate,
) -> Result<(), PrepareError> {
    let PreparedServer {
        listener,
        endpoint,
        epoch,
        idle_ttl,
        publish,
    } = prepared;
    if publish {
        endpoint.activate()?;
    }
    let repo_root = endpoint.repo_root().to_path_buf();
    let mut controller = WalkController::new(repo_root);
    controller.refresh_from_disk()?;
    let operation_root = paths::operation_dir(endpoint.repo_root())?;
    paths::ensure_operation_dir(&operation_root)?;
    let server = WalkServer {
        epoch,
        controller: Arc::new(Mutex::new(controller)),
        jobs: Arc::new(Mutex::new(JobRegistry::default())),
        gate,
        operation_root,
    };
    let result = accept_loop(server, listener, idle_ttl).await;
    if let Err(error) = endpoint.cleanup() {
        warn!(error = ?error, socket = %endpoint.socket().display(), "failed to clean owned walk endpoint after server exit");
    }
    result
}

fn clear_stale(repo_root: &Path, socket: &Path) -> Result<(), PrepareError> {
    let metadata = match fs::symlink_metadata(socket) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_socket_stat",
                detail: format!("failed to stat socket '{}': {source}", socket.display()),
            });
        }
    };
    #[cfg(unix)]
    if !metadata.file_type().is_socket() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk endpoint path '{}' exists but is not a Unix socket",
                socket.display()
            ),
        });
    }
    #[cfg(not(unix))]
    let _ = metadata;

    if socket_reachable(socket)? {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("walk socket '{}' is still reachable", socket.display()),
        });
    }

    if let Some(active) = endpoint::load(repo_root)?
        && active.socket() == socket
        && active.owns_socket()
    {
        return active.cleanup();
    }
    remove_stale_socket(socket, &metadata)
}

#[cfg(unix)]
fn socket_reachable(socket: &Path) -> Result<bool, PrepareError> {
    match StdUnixStream::connect(socket) {
        Ok(_) => Ok(true),
        Err(source)
            if matches!(
                source.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(false)
        }
        Err(source) => Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_socket_probe",
            detail: format!(
                "failed to probe walk socket '{}': {source}",
                socket.display()
            ),
        }),
    }
}

#[cfg(not(unix))]
fn socket_reachable(_socket: &Path) -> Result<bool, PrepareError> {
    Ok(true)
}

#[cfg(unix)]
fn remove_stale_socket(socket: &Path, expected: &fs::Metadata) -> Result<(), PrepareError> {
    use std::os::unix::fs::MetadataExt;

    let actual = fs::symlink_metadata(socket).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_socket_recheck",
        detail: format!(
            "failed to recheck stale walk socket '{}': {source}",
            socket.display()
        ),
    })?;
    if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk socket '{}' changed while stale ownership was being verified",
                socket.display()
            ),
        });
    }
    fs::remove_file(socket).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_remove_socket",
        detail: format!(
            "failed to remove stale walk socket '{}': {source}",
            socket.display()
        ),
    })
}

#[cfg(not(unix))]
fn remove_stale_socket(_socket: &Path, _expected: &fs::Metadata) -> Result<(), PrepareError> {
    Err(PrepareError::InvalidBatchSelection {
        detail: "walk socket cleanup is unsupported on this platform".to_string(),
    })
}

/// Accept client connections serially and dispatch each request.
async fn accept_loop(
    server: WalkServer,
    listener: UnixListener,
    idle_ttl: Option<Duration>,
) -> Result<(), PrepareError> {
    loop {
        let accepted = match idle_ttl {
            Some(ttl) => match time::timeout(ttl, listener.accept()).await {
                Ok(accepted) => accepted,
                Err(_) => {
                    if let Some(job) = server.active_job().await {
                        info!(
                            idle_ttl_secs = ttl.as_secs(),
                            job_id = job.job_id,
                            command = %job.command,
                            "walk server idle TTL elapsed while an admitted job remains active"
                        );
                        continue;
                    }
                    info!(
                        idle_ttl_secs = ttl.as_secs(),
                        "walk server idle TTL expired"
                    );
                    return Ok(());
                }
            },
            None => listener.accept().await,
        };
        let (stream, _) = accepted.map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_accept",
            detail: source.to_string(),
        })?;
        let stop = handle_stream(&server, stream).await?;
        if stop {
            return Ok(());
        }
    }
}

/// Read one request from a connected socket and write one response.
async fn handle_stream(server: &WalkServer, mut stream: UnixStream) -> Result<bool, PrepareError> {
    let request: WalkRequest = match ipc::recv(&mut stream).await {
        Ok(request) => request,
        Err(error) => {
            let phase = server.phase_for_response().await;
            let response = WalkResponse::error(
                "bad_request",
                error.to_string(),
                Some(phase),
                server.epoch.clone(),
            );
            let _ = ipc::send(&mut stream, &response).await;
            return Ok(false);
        }
    };
    let (response, stop) = server.handle(request).await;
    ipc::send(&mut stream, &response).await?;
    Ok(stop)
}

impl WalkServer {
    /// Dispatch one decoded request against the in-memory controller.
    async fn handle(&self, request: WalkRequest) -> (WalkResponse, bool) {
        let stop_requested = matches!(request.body, WalkRequestBody::Stop);
        if !self.gate.allows_mutation() && requires_transfer(&request.body) {
            let phase = self.phase_for_response().await;
            return (
                WalkResponse::error(
                    "transfer_pending",
                    "successor endpoint is online but predecessor controller release is not yet durable",
                    Some(phase),
                    self.epoch.clone(),
                ),
                false,
            );
        }
        let result = match request.body {
            WalkRequestBody::Health => self.status_response("walk server online").await,
            WalkRequestBody::Show => self.show_response().await,
            WalkRequestBody::ShowDelta { verbose, color } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                Ok(WalkResponse::ok(
                    phase,
                    controller.delta_report(DeltaRenderStyle { verbose, color }),
                    self.epoch.clone(),
                ))
            }
            WalkRequestBody::Audit {
                campaign,
                scope,
                transition,
                verify,
                verbose,
                with_note,
            } => {
                let mut controller = self.controller.lock().await;
                if verify {
                    controller.refresh_from_disk().map(|_| {
                        let phase = controller.phase();
                        let mut report = controller.audit(scope, campaign, transition);
                        report.verbose = verbose;
                        report.with_note = with_note;
                        WalkResponse::audit(phase, report, self.epoch.clone())
                    })
                } else {
                    let phase = controller.phase();
                    let mut report = controller.audit(scope, campaign, transition);
                    report.verbose = verbose;
                    report.with_note = with_note;
                    Ok(WalkResponse::audit(phase, report, self.epoch.clone()))
                }
            }
            WalkRequestBody::LlmLanes { verbose } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_lanes_report(verbose)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmFocus { lane } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_focus(lane)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmShow {
                session_id,
                lane,
                head,
                step,
            } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_report(session_id.as_deref(), lane.as_deref(), head, step)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmTimeline { session_id, lane } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_timeline(session_id.as_deref(), lane.as_deref())
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmPrompt {
                session_id,
                lane,
                step,
                role,
                message,
                full,
                json,
            } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_prompt_report(
                        session_id.as_deref(),
                        lane.as_deref(),
                        step,
                        role.as_deref(),
                        message,
                        full,
                        json,
                    )
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmProtocol {
                session_id,
                lane,
                json,
            } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_protocol_report(session_id.as_deref(), lane.as_deref(), json)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmTool {
                session_id,
                lane,
                head,
                step,
                call,
                name,
                json,
            } => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_tool_report(
                        session_id.as_deref(),
                        lane.as_deref(),
                        head,
                        step,
                        call,
                        name.as_deref(),
                        json,
                    )
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmStep { .. } => {
                self.effectful_disabled(request.client_epoch.as_ref(), "llm step")
            }
            WalkRequestBody::LlmFinish { .. } => {
                self.effectful_disabled(request.client_epoch.as_ref(), "llm finish")
            }
            WalkRequestBody::LlmBack { lane, steps } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_move(lane.as_deref(), steps, super::controller::LlmMove::Back)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmForward { lane, steps } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_move(lane.as_deref(), steps, super::controller::LlmMove::Forward)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::LlmHead { lane } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .llm_head(lane.as_deref())
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::Replay { index, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .replay_report(index, tail)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::ReplayBack { steps, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .replay_back(steps, tail)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::ReplayForward { steps, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller
                    .replay_forward(steps, tail)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
            }
            WalkRequestBody::BranchLive { .. } => {
                self.effectful_disabled(request.client_epoch.as_ref(), "branch-live")
            }
            WalkRequestBody::Stop => match self.ensure_stop_guard(request.client_epoch.as_ref()) {
                Ok(()) => self.stop_active_job().await,
                Err(error) => Err(error),
            },
            WalkRequestBody::Start {
                guard,
                config,
                until,
                allow_live_api,
            } => {
                self.submit_start(
                    request.client_epoch.as_ref(),
                    guard,
                    config,
                    until,
                    allow_live_api,
                )
                .await
            }
            WalkRequestBody::Step {
                guard,
                until,
                watch,
                allow_live_api,
                allow_git_changes,
            } => {
                self.submit_step(
                    request.client_epoch.as_ref(),
                    guard,
                    until,
                    watch,
                    allow_live_api,
                    allow_git_changes,
                )
                .await
            }
            WalkRequestBody::Reset { guard } => {
                self.submit_reset(request.client_epoch.as_ref(), guard)
                    .await
            }
            WalkRequestBody::Recover { directive, guard } => {
                self.submit_recover(request.client_epoch.as_ref(), directive, guard)
                    .await
            }
            WalkRequestBody::Files => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                Ok(WalkResponse::ok(
                    phase,
                    controller.files_report(),
                    self.epoch.clone(),
                ))
            }
        };
        match result {
            Ok(response) if stop_requested => {
                debug!(phase = ?response.phase(), stop = true, "handled walk request");
                (response, true)
            }
            Ok(response) => {
                debug!(phase = ?response.phase(), stop = false, "handled walk request");
                (response, false)
            }
            Err(error) => {
                let phase = self.phase_for_response().await;
                (
                    WalkResponse::error(
                        request_error_code(&error),
                        error.to_string(),
                        Some(phase),
                        self.epoch.clone(),
                    ),
                    false,
                )
            }
        }
    }

    async fn submit_start(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        config: WalkStartConfig,
        until: WalkPhase,
        allow_live_api: bool,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let config_root = match config.repo_root.as_deref() {
            Some(root) => paths::resolve_repo_root(Some(root))?,
            None => self.epoch.repo_root.clone(),
        };
        if config_root != self.epoch.repo_root {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk start repository root '{}' does not match the server root '{}'",
                    config_root.display(),
                    self.epoch.repo_root.display()
                ),
            });
        }
        let fingerprint = request_fingerprint(&("start", &config, until, allow_live_api))?;
        let job = match self
            .register_job(
                "start",
                guard,
                fingerprint,
                Some(until),
                None,
                Some(allow_live_api),
                None,
            )
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing start operation; no duplicate job submitted",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };
        let controller = Arc::clone(&self.controller);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let job_id = job.job_id;
        let expected = job.expected.clone();
        let handle = tokio::spawn(run_start_job(
            controller,
            jobs,
            epoch,
            job_id,
            expected,
            config,
            until,
            allow_live_api,
        ));
        self.attach_job_handle(job_id, handle).await;
        Ok(WalkResponse::job(
            job_phase(&job),
            job,
            "accepted start job; use `walk status` to inspect progress",
            self.epoch.clone(),
        ))
    }

    async fn submit_step(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        until: Option<WalkPhase>,
        watch: bool,
        allow_live_api: bool,
        allow_git_changes: bool,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let fingerprint = request_fingerprint(&("step", until, allow_live_api, allow_git_changes))?;
        let job = match self
            .register_job(
                "step",
                guard,
                fingerprint,
                until,
                Some(watch),
                Some(allow_live_api),
                Some(allow_git_changes),
            )
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing step operation; no duplicate job submitted",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };
        let controller = Arc::clone(&self.controller);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let job_id = job.job_id;
        let expected = job.expected.clone();
        let handle = tokio::spawn(run_step_job(
            controller,
            jobs,
            epoch,
            job_id,
            expected,
            until,
            watch,
            allow_live_api,
            allow_git_changes,
        ));
        self.attach_job_handle(job_id, handle).await;
        Ok(WalkResponse::job(
            job_phase(&job),
            job,
            "accepted step job; use `walk status` to inspect progress",
            self.epoch.clone(),
        ))
    }

    async fn submit_reset(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let operation = guard.operation;
        let job = match self
            .register_job("reset", guard, b"reset".to_vec(), None, None, None, None)
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing reset operation; no duplicate reset performed",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };

        let (phase, message) = {
            let mut controller = self.controller.lock().await;
            let phase = controller.reset();
            (phase, format!("reset walk to {phase} - {}", phase.detail()))
        };
        finish_job(
            &self.jobs,
            job.job_id,
            WalkJobStatus::Succeeded,
            Some(phase),
            message,
        )
        .await;
        let completed = self.operation_job(operation).await.unwrap_or(job);
        Ok(WalkResponse::job(
            job_phase(&completed),
            completed,
            "completed reset operation",
            self.epoch.clone(),
        ))
    }

    async fn submit_recover(
        &self,
        client_epoch: Option<&ServerEpoch>,
        directive: RecoveryDirective,
        guard: Option<MutationGuard>,
    ) -> Result<WalkResponse, PrepareError> {
        if directive == RecoveryDirective::Inspect {
            if guard.is_some() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "read-only recovery inspection must not carry a mutation guard"
                        .to_string(),
                });
            }
            let message = recover_admitted_controller(&self.epoch.repo_root, directive)?;
            let version = self.durable_version()?;
            return Ok(WalkResponse::ok(
                version.phase(),
                message,
                self.epoch.clone(),
            ));
        }

        self.ensure_epoch_guard(client_epoch)?;
        let guard = guard.ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail:
                "mutating controller recovery requires an observed session version and operation id"
                    .to_string(),
        })?;
        let operation = guard.operation;
        let expected = guard.expected.clone();
        let fingerprint = request_fingerprint(&("recover", &directive))?;
        let job = match self
            .register_job("recover", guard, fingerprint, None, None, None, None)
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing recovery operation; no duplicate resolution applied",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };

        let result =
            recover_admitted_version(&self.epoch.repo_root, directive, &self.epoch, &expected);
        match result {
            Ok(message) => {
                let phase = {
                    let mut controller = self.controller.lock().await;
                    match controller.refresh_from_disk() {
                        Ok(()) => controller.phase(),
                        Err(source) => {
                            self.mark_stopping().await;
                            let detail = format!(
                                "recovery resolution committed but the server could not refresh its controller; restart the walk server before further mutation: {source}"
                            );
                            finish_job(
                                &self.jobs,
                                job.job_id,
                                WalkJobStatus::Failed,
                                Some(job.phase_before),
                                detail.clone(),
                            )
                            .await;
                            return Err(PrepareError::InvalidBatchSelection { detail });
                        }
                    }
                };
                finish_job(
                    &self.jobs,
                    job.job_id,
                    WalkJobStatus::Succeeded,
                    Some(phase),
                    message,
                )
                .await;
                let completed = self.operation_job(operation).await.unwrap_or(job);
                Ok(WalkResponse::job(
                    job_phase(&completed),
                    completed,
                    "completed controller recovery resolution",
                    self.epoch.clone(),
                ))
            }
            Err(error) => {
                finish_job(
                    &self.jobs,
                    job.job_id,
                    WalkJobStatus::Failed,
                    Some(job.phase_before),
                    error.to_string(),
                )
                .await;
                Err(error)
            }
        }
    }

    fn operation_path(&self, operation: OperationId) -> PathBuf {
        self.operation_root.join(format!("{operation}.json"))
    }

    fn load_operation(
        &self,
        operation: OperationId,
    ) -> Result<Option<DurableOperation>, PrepareError> {
        let path = self.operation_path(operation);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(operation_error("read", &path, source)),
        };
        let record: DurableOperation =
            serde_json::from_slice(&bytes).map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_operation_parse",
                detail: format!(
                    "failed to parse durable operation record '{}': {source}",
                    path.display()
                ),
            })?;
        if record.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "durable operation record '{}' has unsupported schema '{}'; expected '{}'",
                    path.display(),
                    record.schema_version,
                    OPERATION_SCHEMA_VERSION
                ),
            });
        }
        if record.stored.snapshot.operation_id != operation {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "durable operation record '{}' does not match requested operation {operation}",
                    path.display()
                ),
            });
        }
        if record.epoch.repo_root != self.epoch.repo_root {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "durable operation record '{}' belongs to repository '{}' rather than '{}'",
                    path.display(),
                    record.epoch.repo_root.display(),
                    self.epoch.repo_root.display()
                ),
            });
        }
        Ok(Some(record))
    }

    fn persist_operation(
        &self,
        stored: &StoredOperation,
    ) -> Result<Option<DurableOperation>, PrepareError> {
        let operation = stored.snapshot.operation_id;
        let path = self.operation_path(operation);
        let record = DurableOperation {
            schema_version: OPERATION_SCHEMA_VERSION.to_string(),
            epoch: self.epoch.clone(),
            stored: stored.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(PrepareError::Serialize)?;
        let created = durable_io::create_atomic(&path, &bytes)
            .map_err(|source| operation_error("create", &path, source))?;
        if created {
            Ok(None)
        } else {
            self.load_operation(operation)?.ok_or_else(|| {
                PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "durable operation record '{}' won create-no-clobber admission but disappeared before replay",
                        path.display()
                    ),
                }
            }).map(Some)
        }
    }

    async fn register_job(
        &self,
        command: &'static str,
        guard: MutationGuard,
        fingerprint: Vec<u8>,
        target_phase: Option<WalkPhase>,
        watch: Option<bool>,
        allow_live_api: Option<bool>,
        allow_git_changes: Option<bool>,
    ) -> Result<JobAdmission, PrepareError> {
        let mut jobs = self.jobs.lock().await;
        if let Some(existing) = jobs
            .active
            .as_ref()
            .filter(|active| active.snapshot.operation_id == guard.operation)
            .map(|active| StoredOperation {
                snapshot: active.snapshot.clone(),
                fingerprint: active.fingerprint.clone(),
            })
            .or_else(|| jobs.completed.get(&guard.operation).cloned())
        {
            if existing.fingerprint == fingerprint {
                return Ok(JobAdmission::Duplicate(existing.snapshot));
            }
            let actual = self.durable_version()?;
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                "operation_conflict",
                format!(
                    "operation {} was already used for a different {command} payload",
                    guard.operation
                ),
                actual,
                self.epoch.clone(),
            )));
        }

        if let Some(record) = self.load_operation(guard.operation)? {
            let actual = self.durable_version()?;
            let same = record.stored.fingerprint == fingerprint
                && record.stored.snapshot.expected == guard.expected
                && record.stored.snapshot.command == command;
            let code = if same {
                "operation_restart"
            } else {
                "operation_conflict"
            };
            let detail = if same {
                format!(
                    "operation {} was admitted by an earlier walk-server incarnation; inspect durable session state and use a new operation id only for a newly observed version",
                    guard.operation
                )
            } else {
                format!(
                    "operation {} is durably bound to a different request or session version",
                    guard.operation
                )
            };
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                code,
                detail,
                actual,
                self.epoch.clone(),
            )));
        }

        let actual = self.durable_version()?;
        if jobs.stopping {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                "server_stopping",
                format!("walk {command} refused because the server is stopping"),
                actual,
                self.epoch.clone(),
            )));
        }
        if let Some(active) = jobs
            .active
            .as_ref()
            .filter(|active| active.snapshot.status.is_active())
        {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                "job_active",
                format!(
                    "walk {command} refused because job {} ({}) is still {:?}",
                    active.snapshot.job_id, active.snapshot.command, active.snapshot.status
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        if guard.expected != actual {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                "stale_version",
                format!(
                    "walk {command} operation {} expected controller session {:?}, but durable state is {:?}",
                    guard.operation, guard.expected, actual
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        let phase_before = actual.phase();
        if let Some(previous) = jobs.active.take() {
            jobs.completed.insert(
                previous.snapshot.operation_id,
                StoredOperation {
                    snapshot: previous.snapshot,
                    fingerprint: previous.fingerprint,
                },
            );
        }
        jobs.next_id += 1;
        let now = now_rfc3339();
        let snapshot = WalkJobSnapshot {
            job_id: jobs.next_id,
            operation_id: guard.operation,
            expected: guard.expected,
            command: command.to_string(),
            status: WalkJobStatus::Running,
            phase_before,
            phase_after: None,
            target_phase,
            watch,
            allow_live_api,
            allow_git_changes,
            started_at: now.clone(),
            updated_at: now,
            finished_at: None,
            message: None,
        };
        let stored = StoredOperation {
            snapshot: snapshot.clone(),
            fingerprint: fingerprint.clone(),
        };
        if let Some(existing) = self.persist_operation(&stored)? {
            let same = existing.stored.fingerprint == fingerprint
                && existing.stored.snapshot.expected == snapshot.expected
                && existing.stored.snapshot.command == command;
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                if same {
                    "operation_restart"
                } else {
                    "operation_conflict"
                },
                format!(
                    "operation {} was concurrently bound before this {command} job could start",
                    snapshot.operation_id
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        jobs.active = Some(ActiveWalkJob {
            snapshot,
            fingerprint,
            handle: None,
        });
        Ok(JobAdmission::Accepted(
            jobs.active
                .as_ref()
                .expect("accepted operation was just stored")
                .snapshot
                .clone(),
        ))
    }

    async fn attach_job_handle(&self, job_id: u64, handle: JoinHandle<()>) {
        let jobs = Arc::clone(&self.jobs);
        let monitor = tokio::spawn(async move {
            let outcome = handle.await;
            finish_unsettled_job(&jobs, job_id, outcome).await;
        });
        let mut jobs = self.jobs.lock().await;
        if let Some(active) = jobs
            .active
            .as_mut()
            .filter(|active| active.snapshot.job_id == job_id && active.snapshot.status.is_active())
        {
            active.handle = Some(monitor);
        }
    }

    async fn active_job(&self) -> Option<WalkJobSnapshot> {
        self.jobs
            .lock()
            .await
            .active
            .as_ref()
            .filter(|active| active.snapshot.status.is_active())
            .map(|active| active.snapshot.clone())
    }

    async fn operation_job(&self, operation: OperationId) -> Option<WalkJobSnapshot> {
        let jobs = self.jobs.lock().await;
        jobs.active
            .as_ref()
            .filter(|active| active.snapshot.operation_id == operation)
            .map(|active| active.snapshot.clone())
            .or_else(|| {
                jobs.completed
                    .get(&operation)
                    .map(|stored| stored.snapshot.clone())
            })
    }

    async fn mark_stopping(&self) {
        self.jobs.lock().await.stopping = true;
    }

    async fn latest_job(&self) -> Option<WalkJobSnapshot> {
        self.jobs
            .lock()
            .await
            .active
            .as_ref()
            .map(|active| active.snapshot.clone())
    }

    async fn phase_for_response(&self) -> WalkPhase {
        if let Some(job) = self.active_job().await {
            return job_phase(&job);
        }
        self.controller.lock().await.phase()
    }

    async fn status_response(&self, heading: &str) -> Result<WalkResponse, PrepareError> {
        let version = self.durable_version()?;
        let job = self.latest_job().await;
        let controller = self.controller.try_lock().ok();
        let controller_summary = controller.as_ref().map(|controller| controller.describe());
        let phase = match job.as_ref() {
            Some(job) if job.status.is_active() => job_phase(job),
            _ => controller
                .as_ref()
                .map_or_else(|| version.phase(), |controller| controller.phase()),
        };
        let message = self.render_status_message(heading, job.as_ref(), controller_summary);
        Ok(WalkResponse::status(
            phase,
            message,
            job,
            version,
            self.epoch.clone(),
        ))
    }

    async fn show_response(&self) -> Result<WalkResponse, PrepareError> {
        if self
            .latest_job()
            .await
            .is_some_and(|job| job.status.is_active())
        {
            return self.status_response("walk state").await;
        }
        let mut controller = self.controller.lock().await;
        controller.refresh_from_disk()?;
        Ok(WalkResponse::ok(
            controller.phase(),
            self.describe_locked(&controller),
            self.epoch.clone(),
        ))
    }

    async fn stop_active_job(&self) -> Result<WalkResponse, PrepareError> {
        {
            let mut jobs = self.jobs.lock().await;
            if let Some(active) = jobs
                .active
                .as_ref()
                .filter(|active| active.snapshot.status.is_active())
            {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "walk stop refused while job {} ({}) is {:?}; wait for the job to finish, or inspect and explicitly recover/abandon any unresolved durable attempt before stopping the server",
                        active.snapshot.job_id, active.snapshot.command, active.snapshot.status
                    ),
                });
            }
            jobs.stopping = true;
        }

        let cleanup =
            vec!["no local walk job was active; no campaign process was signalled".to_string()];
        let phase = self.controller.lock().await.phase();
        let job = self.latest_job().await;
        let mut message = self.render_status_message("walk server stopping", job.as_ref(), None);
        for line in cleanup {
            message.push('\n');
            message.push_str(&line);
        }
        let version = self
            .durable_version()
            .unwrap_or_else(|_| SessionVersion::empty());
        Ok(WalkResponse::status(
            phase,
            message,
            job,
            version,
            self.epoch.clone(),
        ))
    }

    fn render_status_message(
        &self,
        heading: &str,
        job: Option<&WalkJobSnapshot>,
        controller_summary: Option<String>,
    ) -> String {
        let mut lines = vec![
            format!("server_pid={}", std::process::id()),
            format!(
                "mutation_authority={}",
                if self.gate.allows_mutation() {
                    "active"
                } else {
                    "transfer_pending"
                }
            ),
            heading.to_string(),
        ];
        if let Some(job) = job {
            lines.push(format_job(job));
        } else {
            lines.push("job_status=idle".to_string());
        }
        if let Some(summary) = controller_summary {
            lines.push(summary);
        }
        lines.join("\n")
    }

    fn describe_locked(&self, controller: &WalkController) -> String {
        format!(
            "server_pid={}\n{}",
            std::process::id(),
            controller.describe()
        )
    }

    fn ensure_epoch_guard(&self, client_epoch: Option<&ServerEpoch>) -> Result<(), PrepareError> {
        self.epoch.ensure_compatible_request(client_epoch)?;
        self.epoch.ensure_not_stale_now()
    }

    /// Bind shutdown to the caller-selected repository. Unlike a typestate
    /// mutation, stopping must remain possible after source or binary drift.
    fn ensure_stop_guard(&self, client_epoch: Option<&ServerEpoch>) -> Result<(), PrepareError> {
        let Some(client_epoch) = client_epoch else {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk stop omitted the caller repository epoch".to_string(),
            });
        };
        if client_epoch.repo_root != self.epoch.repo_root {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk repository root mismatch: client='{}' server='{}'",
                    client_epoch.repo_root.display(),
                    self.epoch.repo_root.display()
                ),
            });
        }
        Ok(())
    }

    fn effectful_disabled(
        &self,
        client_epoch: Option<&ServerEpoch>,
        command: &'static str,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk {command} is disabled until its provider/tool/file effects have durable job intent, idempotency, and terminal receipts"
            ),
        })
    }

    /// Inspect the durable session journal directly. This never takes the
    /// controller mutex and therefore remains available while a live job owns
    /// the in-memory typestate value.
    fn durable_version(&self) -> Result<SessionVersion, PrepareError> {
        let Some(parent) = identity::load_parent_identity_optional(&self.epoch.repo_root)? else {
            return Ok(SessionVersion::empty());
        };
        let manifest = campaign_manifest_path(parent.campaign_id())?;
        let snapshot = Store::for_manifest(&manifest)
            .inspect(&parent)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_session_version",
                detail: source.to_string(),
            })?;
        let Some(snapshot) = snapshot else {
            return Ok(SessionVersion::empty());
        };
        Ok(SessionVersion {
            session_id: snapshot
                .created
                .as_ref()
                .map(|created| created.session_id()),
            cursor: snapshot.cursor,
            journal_revision: snapshot.journal_revision,
        })
    }
}

fn requires_transfer(request: &WalkRequestBody) -> bool {
    match request {
        WalkRequestBody::Recover { directive, .. } => *directive != RecoveryDirective::Inspect,
        _ => matches!(
            request,
            WalkRequestBody::Start { .. }
                | WalkRequestBody::Step { .. }
                | WalkRequestBody::Reset { .. }
                | WalkRequestBody::BranchLive { .. }
                | WalkRequestBody::LlmStep { .. }
                | WalkRequestBody::LlmFinish { .. }
        ),
    }
}

fn request_error_code(error: &PrepareError) -> &'static str {
    match error {
        PrepareError::RecoveryInProgress { .. } => "recovery_in_progress",
        _ => "request_failed",
    }
}

fn operation_error(action: &'static str, path: &Path, source: std::io::Error) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_operation_io",
        detail: format!(
            "failed to {action} durable operation record '{}': {source}",
            path.display()
        ),
    }
}

async fn run_start_job(
    controller: Arc<Mutex<WalkController>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    job_id: u64,
    expected: SessionVersion,
    config: WalkStartConfig,
    until: WalkPhase,
    allow_live_api: bool,
) {
    let result = {
        let mut controller = controller.lock().await;
        match controller
            .start_version(config, until, allow_live_api, &expected)
            .await
        {
            Ok(report) => {
                let phase = controller.phase();
                let event = WalkEventInput {
                    command: "start",
                    phase_before: Some(report.from()),
                    phase_after: report.to(),
                    target_phase: Some(until),
                    watch: None,
                    allow_live_api: Some(allow_live_api),
                    allow_git_changes: None,
                    transitions: report.transition_labels(),
                };
                let message = format!("started walk at {} - {}", report.to(), report.to().detail());
                Ok((phase, event, message))
            }
            Err(error) => Err((controller.phase(), error)),
        }
    };
    match result {
        Ok((phase, event, message)) => match record_walk_event(&epoch, event) {
            Ok(()) => {
                finish_job(
                    &jobs,
                    job_id,
                    WalkJobStatus::Succeeded,
                    Some(phase),
                    message,
                )
                .await
            }
            Err(error) => {
                finish_job(
                    &jobs,
                    job_id,
                    WalkJobStatus::Failed,
                    Some(phase),
                    error.to_string(),
                )
                .await
            }
        },
        Err((phase, error)) => {
            finish_job(
                &jobs,
                job_id,
                WalkJobStatus::Failed,
                Some(phase),
                error.to_string(),
            )
            .await
        }
    }
}

async fn run_step_job(
    controller: Arc<Mutex<WalkController>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    job_id: u64,
    expected: SessionVersion,
    until: Option<WalkPhase>,
    client_watch: bool,
    allow_live_api: bool,
    allow_git_changes: bool,
) {
    let result = {
        let mut controller = controller.lock().await;
        match controller
            .step_version(until, allow_live_api, allow_git_changes, &expected)
            .await
        {
            Ok(report) => {
                let phase = controller.phase();
                let message = report.render();
                let event = WalkEventInput {
                    command: "step",
                    phase_before: Some(report.from()),
                    phase_after: report.to(),
                    target_phase: until,
                    watch: Some(client_watch),
                    allow_live_api: Some(allow_live_api),
                    allow_git_changes: Some(allow_git_changes),
                    transitions: report.transition_labels(),
                };
                Ok((phase, event, message))
            }
            Err(error) => Err((controller.phase(), error)),
        }
    };
    match result {
        Ok((phase, event, message)) => match record_walk_event(&epoch, event) {
            Ok(()) => {
                finish_job(
                    &jobs,
                    job_id,
                    WalkJobStatus::Succeeded,
                    Some(phase),
                    message,
                )
                .await
            }
            Err(error) => {
                finish_job(
                    &jobs,
                    job_id,
                    WalkJobStatus::Failed,
                    Some(phase),
                    error.to_string(),
                )
                .await
            }
        },
        Err((phase, error)) => {
            finish_job(
                &jobs,
                job_id,
                WalkJobStatus::Failed,
                Some(phase),
                error.to_string(),
            )
            .await
        }
    }
}

async fn finish_job(
    jobs: &Mutex<JobRegistry>,
    job_id: u64,
    status: WalkJobStatus,
    phase_after: Option<WalkPhase>,
    message: String,
) {
    let mut jobs = jobs.lock().await;
    let Some(active) = jobs
        .active
        .as_mut()
        .filter(|active| active.snapshot.job_id == job_id)
    else {
        return;
    };
    if active.snapshot.status == WalkJobStatus::CancelRequested {
        return;
    }
    active.snapshot.status = status;
    active.snapshot.phase_after = phase_after;
    active.snapshot.updated_at = now_rfc3339();
    active.snapshot.finished_at = Some(now_rfc3339());
    active.snapshot.message = Some(message);
    active.handle = None;
}

async fn finish_unsettled_job(
    jobs: &Mutex<JobRegistry>,
    job_id: u64,
    outcome: Result<(), tokio::task::JoinError>,
) {
    let message = match outcome {
        Ok(()) => {
            "walk job task exited without recording a terminal status; inspect durable session state and explicitly recover or abandon any unresolved attempt"
                .to_string()
        }
        Err(error) if error.is_panic() => format!(
            "walk job task panicked after admission ({error}); inspect durable session state and explicitly recover or abandon any unresolved attempt"
        ),
        Err(error) if error.is_cancelled() => format!(
            "walk job task was cancelled after admission ({error}); inspect durable session state and explicitly recover or abandon any unresolved attempt"
        ),
        Err(error) => format!(
            "walk job task failed after admission ({error}); inspect durable session state and explicitly recover or abandon any unresolved attempt"
        ),
    };
    let mut jobs = jobs.lock().await;
    let Some(active) = jobs
        .active
        .as_mut()
        .filter(|active| active.snapshot.job_id == job_id && active.snapshot.status.is_active())
    else {
        return;
    };
    active.snapshot.status = WalkJobStatus::Failed;
    active.snapshot.updated_at = now_rfc3339();
    active.snapshot.finished_at = Some(now_rfc3339());
    active.snapshot.message = Some(message);
    active.handle = None;
}

fn record_walk_event(epoch: &ServerEpoch, input: WalkEventInput) -> Result<(), PrepareError> {
    let Some(identity) = identity::load_parent_identity_optional(&epoch.repo_root)? else {
        return Ok(());
    };
    let db_path = owner_db_path(identity.campaign_id().as_str())?;
    if !db_path.is_file() {
        return Ok(());
    }
    crate::cli::prototype1_state::eval_store::write_walk_event_to_owner_db(
        &db_path,
        crate::cli::prototype1_state::eval_store::WalkEventEvidence {
            campaign_id: identity.campaign_id().to_string(),
            node_id: identity.node_id().to_string(),
            parent_id: identity.parent_id().to_string(),
            generation: identity.generation(),
            branch_id: identity.branch_id().to_string(),
            command: input.command.to_string(),
            status: "ok".to_string(),
            phase_before: input.phase_before.map(|phase| phase.to_string()),
            phase_after: input.phase_after.to_string(),
            target_phase: input.target_phase.map(|phase| phase.to_string()),
            watch: input.watch,
            allow_live_api: input.allow_live_api,
            allow_git_changes: input.allow_git_changes,
            transitions: input.transitions,
            protocol_version: epoch.protocol_version,
            transition_graph_version: epoch.transition_graph_version.clone(),
            repo_root: epoch.repo_root.display().to_string(),
            exe_path: epoch.exe_path.display().to_string(),
            exe_modified_unix_ms: epoch.exe_modified_unix_ms,
            git_head: epoch.git_head.clone(),
            source_status_hash: epoch.source_status_hash.clone(),
            recorded_at: Utc::now().to_rfc3339(),
        },
    )
    .map_err(|source| PrepareError::DatabaseSetup {
        phase: "eval_walk_event_put",
        detail: format!(
            "failed to persist walk event '{}' for campaign '{}': {source}",
            input.command,
            identity.campaign_id()
        ),
    })
}

#[derive(Debug)]
struct RecordedProcess {
    kind: &'static str,
    runtime_id: RuntimeId,
    pid: u32,
    incarnation: Option<ProcessIncarnation>,
    argv: Vec<OsString>,
}

fn terminate_recorded_processes(epoch: &ServerEpoch) -> Result<Vec<String>, PrepareError> {
    let Some(journal_path) = transition_journal_path_for_epoch(epoch)? else {
        return Ok(vec![
            "process_cleanup: no parent identity; no transition journal inspected".to_string(),
        ]);
    };
    let journal = PrototypeJournal::new(journal_path.clone());
    let entries = journal
        .load_entries()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_transition_journal_read",
            detail: format!(
                "failed to read transition journal '{}': {source}",
                journal_path.display()
            ),
        })?;
    let processes = recorded_processes(entries);
    if processes.is_empty() {
        return Ok(vec![format!(
            "process_cleanup: no recorded child/successor pids in {}",
            journal_path.display()
        )]);
    }

    let mut messages = Vec::new();
    for process in processes {
        let handle = match recorded_process_still_matches(&process) {
            Ok(Some(handle)) => handle,
            Ok(None) => {
                messages.push(format!(
                    "process_cleanup: skipped pid {} ({} runtime {}) because its exact incarnation, argv, or process-group identity did not match",
                    process.pid, process.kind, process.runtime_id
                ));
                continue;
            }
            Err(error) => {
                messages.push(format!(
                    "process_cleanup: could not verify pid {} ({} runtime {}); no signal sent: {}",
                    process.pid, process.kind, process.runtime_id, error
                ));
                continue;
            }
        };
        match terminate_process_group(&handle) {
            Ok(()) => messages.push(format!(
                "process_cleanup: sent SIGTERM to process group {} ({} runtime {})",
                process.pid, process.kind, process.runtime_id
            )),
            Err(error) => messages.push(format!(
                "process_cleanup: failed to signal process group {} ({} runtime {}): {}",
                process.pid, process.kind, process.runtime_id, error
            )),
        }
    }
    Ok(messages)
}

fn recorded_processes(entries: Vec<JournalEntry>) -> Vec<RecordedProcess> {
    let mut seen = BTreeSet::new();
    let mut processes = Vec::new();
    for entry in entries {
        match entry {
            JournalEntry::SpawnChild(entry) => {
                if let Some((pid, incarnation)) = entry
                    .cleanup_authority()
                    .map(|(pid, incarnation)| (pid, incarnation.clone()))
                {
                    let key = ("child", entry.runtime_id);
                    if seen.insert(key) {
                        let mut argv = Vec::with_capacity(entry.argv.len() + 1);
                        argv.push(entry.paths.binary_path.into_os_string());
                        argv.extend(entry.argv.into_iter().map(OsString::from));
                        processes.push(RecordedProcess {
                            kind: "child",
                            runtime_id: entry.runtime_id,
                            pid,
                            incarnation: Some(incarnation),
                            argv,
                        });
                    }
                }
            }
            JournalEntry::Successor(entry) => {
                if let (
                    Some(runtime_id),
                    successor::State::Spawned {
                        pid,
                        incarnation,
                        active_parent_root,
                        binary_path,
                        invocation_path,
                        ..
                    },
                ) = (entry.runtime_id, entry.state)
                {
                    let key = ("successor", runtime_id);
                    if seen.insert(key) {
                        processes.push(RecordedProcess {
                            kind: "successor",
                            runtime_id,
                            pid,
                            incarnation,
                            argv: successor_argv(
                                binary_path,
                                entry.campaign_id.as_str(),
                                active_parent_root,
                                invocation_path,
                            ),
                        });
                    }
                }
            }
            JournalEntry::SuccessorHandoff(entry) => {
                let key = ("successor", entry.runtime_id);
                if seen.insert(key) {
                    processes.push(RecordedProcess {
                        kind: "successor",
                        runtime_id: entry.runtime_id,
                        pid: entry.pid,
                        incarnation: None,
                        argv: successor_argv(
                            entry.binary_path,
                            entry.campaign_id.as_str(),
                            entry.active_parent_root,
                            entry.invocation_path,
                        ),
                    });
                }
            }
            JournalEntry::ParentStarted(_)
            | JournalEntry::Resource(_)
            | JournalEntry::ChildArtifactCommitted(_)
            | JournalEntry::ActiveCheckoutAdvanced(_)
            | JournalEntry::MaterializeBranch(_)
            | JournalEntry::BuildChild(_)
            | JournalEntry::Child(_)
            | JournalEntry::ChildReady(_)
            | JournalEntry::ObserveChild(_) => {}
        }
    }
    processes
}

fn successor_argv(
    binary: PathBuf,
    campaign: &str,
    repo_root: PathBuf,
    invocation: PathBuf,
) -> Vec<OsString> {
    vec![
        binary.into_os_string(),
        OsString::from("loop"),
        OsString::from("prototype1-state"),
        OsString::from("--campaign"),
        OsString::from(campaign),
        OsString::from("--repo-root"),
        repo_root.into_os_string(),
        OsString::from("--handoff-invocation"),
        invocation.into_os_string(),
    ]
}

#[cfg(target_os = "linux")]
fn recorded_process_still_matches(process: &RecordedProcess) -> std::io::Result<Option<fs::File>> {
    let Some(handle) = open_pidfd(process.pid)? else {
        return Ok(None);
    };
    if process.incarnation.is_none() {
        return Ok(None);
    }
    if let Some(expected) = process.incarnation.as_ref()
        && process_incarnation(process.pid)?.as_ref() != Some(expected)
    {
        return Ok(None);
    }
    let Some(argv) = process_cmdline(process.pid)? else {
        return Ok(None);
    };
    if argv != process.argv || process_group(process.pid)? != Some(process.pid) {
        return Ok(None);
    }
    Ok(Some(handle))
}

#[cfg(target_os = "linux")]
fn open_pidfd(pid: u32) -> std::io::Result<Option<fs::File>> {
    // SAFETY: `pidfd_open` receives a numeric process id and the only currently
    // supported flags value, zero. A successful return owns a new descriptor.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(None);
        }
        return Err(error);
    }
    let fd = i32::try_from(fd).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("pidfd_open returned out-of-range descriptor {fd}"),
        )
    })?;
    // SAFETY: the successful syscall returned a fresh owned descriptor and no
    // other owner is constructed for it.
    Ok(Some(unsafe { fs::File::from_raw_fd(fd) }))
}

#[cfg(not(target_os = "linux"))]
fn recorded_process_still_matches(_process: &RecordedProcess) -> std::io::Result<Option<fs::File>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "exact process cleanup requires Linux pidfd support",
    ))
}

#[cfg(target_os = "linux")]
fn process_cmdline(pid: u32) -> std::io::Result<Option<Vec<OsString>>> {
    let bytes = match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(source),
    };
    if bytes.is_empty() {
        return Ok(None);
    }
    let mut parts = bytes
        .split(|byte| *byte == 0)
        .map(|part| OsString::from_vec(part.to_vec()))
        .collect::<Vec<_>>();
    if parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    Ok((!parts.is_empty()).then_some(parts))
}

#[cfg(target_os = "linux")]
fn process_group(pid: u32) -> std::io::Result<Option<u32>> {
    let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(source),
    };
    let (_, fields) = stat.rsplit_once(')').ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("malformed /proc/{pid}/stat"),
        )
    })?;
    let group = fields
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("missing process group in /proc/{pid}/stat"),
            )
        })?
        .parse::<u32>()
        .map_err(|source| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid process group in /proc/{pid}/stat: {source}"),
            )
        })?;
    Ok(Some(group))
}

#[cfg(target_os = "linux")]
fn terminate_process_group(handle: &fs::File) -> std::io::Result<()> {
    const PIDFD_SIGNAL_PROCESS_GROUP: libc::c_uint = 1 << 2;

    // SAFETY: `handle` is a pidfd returned by `pidfd_open`, the signal carries
    // no siginfo payload, and the kernel validates the group-leader requirement
    // atomically against that stable process reference.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            handle.as_raw_fd(),
            libc::SIGTERM,
            std::ptr::null::<libc::siginfo_t>(),
            PIDFD_SIGNAL_PROCESS_GROUP,
        )
    };
    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn terminate_process_group(_handle: &fs::File) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "exact process-group signaling requires Linux 6.9 or newer",
    ))
}

fn transition_journal_path_for_epoch(epoch: &ServerEpoch) -> Result<Option<PathBuf>, PrepareError> {
    let Some(identity) = identity::load_parent_identity_optional(&epoch.repo_root)? else {
        return Ok(None);
    };
    let manifest_path = campaigns_dir()?
        .join(identity.campaign_id().as_str())
        .join("campaign.json");
    Ok(Some(prototype1_transition_journal_path(&manifest_path)))
}

fn format_job(job: &WalkJobSnapshot) -> String {
    let mut lines = Vec::new();
    lines.push(format!("job_id={}", job.job_id));
    lines.push(format!("operation_id={}", job.operation_id));
    lines.push(format!("job_command={}", job.command));
    lines.push(format!("job_status={}", job_status_label(job.status)));
    lines.push(format!(
        "job_phase: {} -> {}",
        job.phase_before,
        job.phase_after
            .map(|phase| phase.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    if let Some(target) = job.target_phase {
        lines.push(format!("job_target={target}"));
    }
    if let Some(watch) = job.watch {
        lines.push(format!("job_watch={watch}"));
    }
    if let Some(allow) = job.allow_live_api {
        lines.push(format!("job_allow_live_api={allow}"));
    }
    if let Some(allow) = job.allow_git_changes {
        lines.push(format!("job_allow_git_changes={allow}"));
    }
    if let Some(message) = &job.message {
        lines.push(format!("job_message={message}"));
    }
    lines.join("\n")
}

fn request_fingerprint(value: &impl serde::Serialize) -> Result<Vec<u8>, PrepareError> {
    serde_json::to_vec(value).map_err(PrepareError::Serialize)
}

fn job_phase(job: &WalkJobSnapshot) -> WalkPhase {
    job.phase_after.unwrap_or(job.phase_before)
}

fn job_status_label(status: WalkJobStatus) -> &'static str {
    match status {
        WalkJobStatus::Running => "running",
        WalkJobStatus::Succeeded => "succeeded",
        WalkJobStatus::Failed => "failed",
        WalkJobStatus::CancelRequested => "cancel_requested",
        WalkJobStatus::Cancelled => "cancelled",
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn owner_db_path(campaign_id: &str) -> Result<PathBuf, PrepareError> {
    Ok(campaigns_dir()?
        .join(campaign_id)
        .join("prototype1/eval-store.cozo.sqlite"))
}

#[cfg(all(test, unix))]
mod tests {
    use std::{ffi::OsString, str::FromStr};

    #[cfg(target_os = "linux")]
    use std::{os::unix::process::CommandExt, process::Command};

    use ploke_records::ids::CampaignId;
    use tempfile::tempdir;

    use crate::{
        cli::{
            Prototype1StateWalkLlmStepSource,
            prototype1_state::{
                driver::control::PredecessorRelease,
                event::{RecordedAt, RuntimeId},
                journal::{Streams, SuccessorHandoffEntry},
                walk::{endpoint, epoch::ServerEpoch},
            },
        },
        test_support::env_guard_os,
    };

    use super::*;

    fn test_server(repo_root: &Path, gate: MutationGate) -> WalkServer {
        WalkServer {
            epoch: ServerEpoch::capture(repo_root).expect("capture server epoch"),
            controller: Arc::new(Mutex::new(WalkController::new(repo_root.to_path_buf()))),
            jobs: Arc::new(Mutex::new(JobRegistry::default())),
            gate,
            operation_root: repo_root.join("walk-operations"),
        }
    }

    fn test_guard(value: u128) -> MutationGuard {
        MutationGuard {
            operation: OperationId::for_test(value),
            expected: SessionVersion::empty(),
        }
    }

    fn accepted(admission: JobAdmission) -> WalkJobSnapshot {
        match admission {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(_) | JobAdmission::Rejected(_) => {
                panic!("expected a newly accepted job")
            }
        }
    }

    fn successor_record(runtime: RuntimeId, pid: u32, tag: &str) -> JournalEntry {
        JournalEntry::Successor(successor::Record {
            runtime_id: Some(runtime),
            recorded_at: RecordedAt::now(),
            campaign_id: CampaignId::from("campaign"),
            node_id: format!("node-{tag}"),
            state: successor::State::Spawned {
                pid,
                incarnation: Some(ProcessIncarnation {
                    boot_id: uuid::Uuid::from_u128(1),
                    start_ticks: 1,
                }),
                active_parent_root: PathBuf::from(format!("/repo/{tag}")),
                binary_path: PathBuf::from(format!("/repo/{tag}/ploke-eval")),
                invocation_path: PathBuf::from(format!("/run/{tag}.json")),
                ready_path: PathBuf::from(format!("/run/{tag}.ready")),
                streams: Streams {
                    stdout: PathBuf::from(format!("/run/{tag}.stdout")),
                    stderr: PathBuf::from(format!("/run/{tag}.stderr")),
                },
            },
        })
    }

    fn handoff_record(runtime: RuntimeId, pid: u32, tag: &str) -> JournalEntry {
        JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
            recorded_at: RecordedAt::now(),
            campaign_id: CampaignId::from("campaign"),
            node_id: format!("node-{tag}"),
            runtime_id: runtime,
            active_parent_root: PathBuf::from(format!("/repo/{tag}")),
            binary_path: PathBuf::from(format!("/repo/{tag}/ploke-eval")),
            invocation_path: PathBuf::from(format!("/run/{tag}.json")),
            ready_path: PathBuf::from(format!("/run/{tag}.ready")),
            streams: None,
            pid,
            acceptance: None,
        })
    }

    fn transfer_cases(repo_root: &Path) -> Vec<(&'static str, WalkRequestBody)> {
        vec![
            (
                "start",
                WalkRequestBody::Start {
                    guard: test_guard(1),
                    config: WalkStartConfig {
                        campaign: None,
                        repo_root: Some(repo_root.to_path_buf()),
                    },
                    until: WalkPhase::R0,
                    allow_live_api: false,
                },
            ),
            (
                "step",
                WalkRequestBody::Step {
                    guard: test_guard(2),
                    until: None,
                    watch: false,
                    allow_live_api: false,
                    allow_git_changes: false,
                },
            ),
            (
                "reset",
                WalkRequestBody::Reset {
                    guard: test_guard(3),
                },
            ),
            (
                "recover",
                WalkRequestBody::Recover {
                    directive: RecoveryDirective::AdmitEpoch,
                    guard: Some(test_guard(4)),
                },
            ),
            (
                "branch_live",
                WalkRequestBody::BranchLive {
                    reason: "test transfer gate".to_string(),
                    allow_provenance_record: true,
                },
            ),
            (
                "llm_step",
                WalkRequestBody::LlmStep {
                    session_id: None,
                    lane: None,
                    step: None,
                    source: Prototype1StateWalkLlmStepSource::Historical,
                    watch: false,
                    allow_workspace_mutation: false,
                    model_id: None,
                    provider: None,
                    max_attempts: 1,
                    timeout_secs: 1,
                },
            ),
            (
                "llm_finish",
                WalkRequestBody::LlmFinish {
                    session_id: None,
                    lane: None,
                    step: None,
                    watch: false,
                    allow_workspace_mutation: false,
                    model_id: None,
                    provider: None,
                    max_steps: 1,
                    max_attempts: 1,
                    timeout_secs: 1,
                },
            ),
        ]
    }

    fn walk_request(body: WalkRequestBody) -> WalkRequest {
        WalkRequest {
            client_epoch: None,
            body,
        }
    }

    #[tokio::test]
    async fn closed_gate_admits_reads_and_blocks_transfer_requests() {
        let repo = tempdir().expect("repo tempdir");
        let gate = MutationGate::closed();
        let server = test_server(repo.path(), gate.clone());

        for body in [WalkRequestBody::Health, WalkRequestBody::Show] {
            let (response, stop) = server.handle(walk_request(body)).await;
            assert!(!stop, "read-only request must not stop the server");
            match response {
                WalkResponse::Ok { phase, .. } | WalkResponse::Status { phase, .. } => {
                    assert_eq!(phase, WalkPhase::Empty);
                }
                other => panic!("closed gate rejected a read-only request: {other:?}"),
            }
        }
        assert!(
            !requires_transfer(&WalkRequestBody::Recover {
                directive: RecoveryDirective::Inspect,
                guard: None,
            }),
            "recovery inspection must remain available before transfer"
        );

        for (kind, body) in transfer_cases(repo.path()) {
            assert!(
                requires_transfer(&body),
                "{kind} must remain classified as transfer-required"
            );
            let (response, stop) = server.handle(walk_request(body)).await;
            assert!(!stop, "{kind} rejection must not stop the server");
            match response {
                WalkResponse::Error {
                    code,
                    detail,
                    phase,
                    ..
                } => {
                    assert_eq!(code, "transfer_pending", "wrong rejection for {kind}");
                    assert!(
                        detail.contains("predecessor controller release"),
                        "missing transfer detail for {kind}: {detail}"
                    );
                    assert_eq!(phase, Some(WalkPhase::Empty));
                }
                other => panic!("{kind} bypassed the closed transfer gate: {other:?}"),
            }
        }

        gate.allow(PredecessorRelease::for_test());
        let request = WalkRequest {
            client_epoch: Some(server.epoch.clone()),
            body: WalkRequestBody::Reset {
                guard: test_guard(5),
            },
        };
        let (response, stop) = server.handle(request).await;
        assert!(!stop, "admitted reset must not stop the server");
        match response {
            WalkResponse::Job { phase, job, .. } => {
                assert_eq!(phase, WalkPhase::Empty);
                assert_eq!(job.status, WalkJobStatus::Succeeded);
            }
            other => panic!("release proof did not open the mutation gate: {other:?}"),
        }

        let pending = test_server(repo.path(), MutationGate::closed());
        let (response, stop) = pending
            .handle(WalkRequest {
                client_epoch: Some(pending.epoch.clone()),
                body: WalkRequestBody::Stop,
            })
            .await;
        assert!(stop, "closed pending endpoint must permit local shutdown");
        match response {
            WalkResponse::Status { message, job, .. } => {
                assert!(
                    job.is_none(),
                    "local shutdown must not invent a cancelled job"
                );
                assert!(
                    message.contains("no campaign process was signalled"),
                    "local shutdown must not perform campaign cancellation: {message}"
                );
            }
            other => panic!("closed endpoint did not return local shutdown status: {other:?}"),
        }
    }

    #[tokio::test]
    async fn start_rejects_epoch_config_root_mismatch_before_job_admission() {
        let repo = tempdir().expect("server repo");
        let other = tempdir().expect("other repo");
        let server = test_server(repo.path(), MutationGate::open());

        let (response, stop) = server
            .handle(WalkRequest {
                client_epoch: Some(server.epoch.clone()),
                body: WalkRequestBody::Start {
                    guard: test_guard(40),
                    config: WalkStartConfig {
                        campaign: None,
                        repo_root: Some(other.path().to_path_buf()),
                    },
                    until: WalkPhase::R3,
                    allow_live_api: false,
                },
            })
            .await;

        assert!(!stop);
        match response {
            WalkResponse::Error { detail, .. } => {
                assert!(
                    detail.contains("does not match the server root"),
                    "{detail}"
                )
            }
            other => panic!("cross-root start did not fail closed: {other:?}"),
        }
        assert!(server.latest_job().await.is_none());
        assert_eq!(server.controller.lock().await.phase(), WalkPhase::Empty);
    }

    #[tokio::test]
    async fn effectful_debug_requests_fail_closed() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());

        for (kind, body) in transfer_cases(repo.path())
            .into_iter()
            .filter(|(kind, _)| matches!(*kind, "branch_live" | "llm_step" | "llm_finish"))
        {
            let request = WalkRequest {
                client_epoch: Some(server.epoch.clone()),
                body,
            };
            let (response, stop) = server.handle(request).await;
            assert!(!stop, "{kind} rejection must not stop the server");
            match response {
                WalkResponse::Error { detail, .. } => {
                    assert!(detail.contains("is disabled"), "{kind}: {detail}");
                    assert!(detail.contains("durable job intent"), "{kind}: {detail}");
                }
                other => panic!("{kind} did not fail closed: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn show_reports_active_job_without_waiting_for_controller() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                "step",
                test_guard(10),
                b"step".to_vec(),
                Some(WalkPhase::R6),
                Some(false),
                None,
                None,
            )
            .await
            .map(accepted)
            .expect("register test job");
        let _controller = server.controller.lock().await;

        let handled = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::Show)),
        )
        .await
        .expect("show must not wait for the live job's controller lock");

        match handled {
            (
                WalkResponse::Status {
                    phase,
                    job: Some(active),
                    ..
                },
                false,
            ) => {
                assert_eq!(phase, WalkPhase::Empty);
                assert_eq!(active.job_id, job.job_id);
                assert_eq!(active.status, WalkJobStatus::Running);
            }
            other => panic!("show did not report the active job: {other:?}"),
        }
    }

    #[tokio::test]
    async fn idle_ttl_waits_for_active_job() {
        let repo = tempdir().expect("repo tempdir");
        let listener = UnixListener::bind(repo.path().join("walk.sock")).expect("bind walk socket");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                "step",
                test_guard(10_001),
                b"step".to_vec(),
                Some(WalkPhase::R6),
                Some(false),
                None,
                None,
            )
            .await
            .map(accepted)
            .expect("register active job");
        let jobs = Arc::clone(&server.jobs);
        let serving = tokio::spawn(accept_loop(
            server,
            listener,
            Some(Duration::from_millis(20)),
        ));

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            !serving.is_finished(),
            "idle TTL must not terminate a server with an admitted job"
        );

        finish_job(
            &jobs,
            job.job_id,
            WalkJobStatus::Failed,
            None,
            "test job settled".to_string(),
        )
        .await;
        tokio::time::timeout(Duration::from_millis(200), serving)
            .await
            .expect("server should apply TTL after the job settles")
            .expect("accept loop task")
            .expect("accept loop result");
    }

    #[tokio::test]
    async fn task_panic_releases_recovery_admission() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                "step",
                test_guard(10_002),
                b"step".to_vec(),
                Some(WalkPhase::R6),
                Some(false),
                None,
                None,
            )
            .await
            .map(accepted)
            .expect("register test job");
        let handle = tokio::spawn(async move {
            panic!("simulated walk job panic");
        });
        server.attach_job_handle(job.job_id, handle).await;

        let failed = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let snapshot = server.latest_job().await.expect("job snapshot");
                if snapshot.status == WalkJobStatus::Failed {
                    break snapshot;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("panic supervisor should settle the job");
        assert!(
            failed.message.as_deref().is_some_and(|message| {
                message.contains("panicked after admission")
                    && message.contains("recover or abandon")
            }),
            "unexpected supervisor message: {:?}",
            failed.message
        );

        let recovery = server
            .register_job(
                "recover",
                test_guard(10_003),
                b"recover".to_vec(),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("recovery admission response");
        assert!(
            matches!(recovery, JobAdmission::Accepted(_)),
            "failed task must not leave the job registry permanently Running"
        );
    }

    #[tokio::test]
    async fn stop_requires_observed_epoch_and_awaits_task_abort() {
        struct AbortObserved(Arc<AtomicBool>);

        impl Drop for AbortObserved {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                "step",
                test_guard(11),
                b"step".to_vec(),
                Some(WalkPhase::R6),
                Some(false),
                None,
                None,
            )
            .await
            .map(accepted)
            .expect("register test job");
        let aborted = Arc::new(AtomicBool::new(false));
        let started = Arc::new(tokio::sync::Notify::new());
        let task_aborted = Arc::clone(&aborted);
        let task_started = Arc::clone(&started);
        let handle = tokio::spawn(async move {
            let _observed = AbortObserved(task_aborted);
            task_started.notify_one();
            std::future::pending::<()>().await;
        });
        started.notified().await;
        server.attach_job_handle(job.job_id, handle).await;
        let (health, _) = server.handle(walk_request(WalkRequestBody::Health)).await;
        let observed = match health {
            WalkResponse::Status { epoch, .. } => epoch,
            other => panic!("health did not return endpoint authority: {other:?}"),
        };

        let (rejected, stop) = server.handle(walk_request(WalkRequestBody::Stop)).await;
        assert!(!stop, "unfenced stop must not stop the server");
        match rejected {
            WalkResponse::Error { detail, .. } => {
                assert!(
                    detail.contains("omitted the caller repository epoch"),
                    "{detail}"
                )
            }
            other => panic!("unfenced stop was not rejected: {other:?}"),
        }
        assert!(
            !aborted.load(Ordering::Acquire),
            "rejected stop must leave the job running"
        );

        let (response, stop) = server
            .handle(WalkRequest {
                client_epoch: Some(observed),
                body: WalkRequestBody::Stop,
            })
            .await;
        assert!(!stop, "an active job must keep the server online");
        assert!(
            !aborted.load(Ordering::Acquire),
            "stop must not abort a task after durable attempt admission"
        );
        match response {
            WalkResponse::Error { detail, .. } => {
                assert!(detail.contains("stop refused while job"), "{detail}")
            }
            other => panic!("active-job stop did not fail closed: {other:?}"),
        }
        let admission = server
            .register_job(
                "step",
                test_guard(12),
                b"next-step".to_vec(),
                Some(WalkPhase::R6),
                Some(false),
                None,
                None,
            )
            .await
            .expect("stopping rejection is a typed response");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error { code, detail, .. }) => {
                assert_eq!(code, "job_active");
                assert!(detail.contains("is still Running"), "{detail}");
            }
            JobAdmission::Accepted(_) | JobAdmission::Duplicate(_) => {
                panic!("stopping server admitted another mutation")
            }
            JobAdmission::Rejected(other) => panic!("wrong stopping response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn stop_is_repo_bound_but_source_drift_tolerant() {
        let repo = tempdir().expect("server repo");
        let other = tempdir().expect("other repo");
        let server = test_server(repo.path(), MutationGate::open());

        let (rejected, stop) = server
            .handle(WalkRequest {
                client_epoch: Some(ServerEpoch::capture(other.path()).expect("other epoch")),
                body: WalkRequestBody::Stop,
            })
            .await;
        assert!(!stop, "cross-repository stop must be rejected");
        assert!(matches!(
            rejected,
            WalkResponse::Error { detail, .. }
                if detail.contains("repository root mismatch")
        ));

        let mut drifted = server.epoch.clone();
        drifted.git_head = Some("new-source-head".to_string());
        let (accepted, stop) = server
            .handle(WalkRequest {
                client_epoch: Some(drifted),
                body: WalkRequestBody::Stop,
            })
            .await;
        assert!(stop, "same-repository stale server must remain stoppable");
        assert!(matches!(accepted, WalkResponse::Status { .. }));
    }

    #[tokio::test]
    async fn health_exposes_durable_version_without_controller_lock() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let _controller = server.controller.lock().await;

        let handled = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::Health)),
        )
        .await
        .expect("health must not wait for the controller mutex");

        match handled {
            (WalkResponse::Status { version, phase, .. }, false) => {
                assert_eq!(version, SessionVersion::empty());
                assert_eq!(phase, WalkPhase::Empty);
            }
            other => panic!("health did not expose a durable version: {other:?}"),
        }
    }

    #[tokio::test]
    async fn operation_retry_is_idempotent_and_payload_reuse_conflicts() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let guard = test_guard(20);
        let first = accepted(
            server
                .register_job(
                    "step",
                    guard.clone(),
                    b"step-a".to_vec(),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .expect("accept first operation"),
        );

        let duplicate = server
            .register_job(
                "step",
                guard.clone(),
                b"step-a".to_vec(),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("look up duplicate operation");
        match duplicate {
            JobAdmission::Duplicate(job) => assert_eq!(job.job_id, first.job_id),
            JobAdmission::Accepted(_) | JobAdmission::Rejected(_) => {
                panic!("exact operation retry did not attach to the original job")
            }
        }

        let conflict = server
            .register_job("step", guard, b"step-b".to_vec(), None, None, None, None)
            .await
            .expect("classify operation conflict");
        match conflict {
            JobAdmission::Rejected(WalkResponse::Error { code, .. }) => {
                assert_eq!(code, "operation_conflict")
            }
            JobAdmission::Accepted(_) | JobAdmission::Duplicate(_) => {
                panic!("changed payload reused an operation id")
            }
            JobAdmission::Rejected(other) => panic!("wrong operation conflict: {other:?}"),
        }
    }

    #[tokio::test]
    async fn operation_reuse_after_restart_is_a_typed_conflict() {
        let repo = tempdir().expect("repo tempdir");
        let guard = test_guard(21);
        let first = test_server(repo.path(), MutationGate::open());
        accepted(
            first
                .register_job(
                    "step",
                    guard.clone(),
                    b"step-a".to_vec(),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .expect("accept first-server operation"),
        );
        drop(first);

        let restarted = test_server(repo.path(), MutationGate::open());
        let admission = restarted
            .register_job("step", guard, b"step-a".to_vec(), None, None, None, None)
            .await
            .expect("classify restarted operation");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error { code, detail, .. }) => {
                assert_eq!(code, "operation_restart");
                assert!(
                    detail.contains("earlier walk-server incarnation"),
                    "{detail}"
                );
            }
            JobAdmission::Accepted(_) | JobAdmission::Duplicate(_) => {
                panic!("server restart reused an already admitted operation")
            }
            JobAdmission::Rejected(other) => panic!("wrong restart conflict: {other:?}"),
        }
    }

    #[tokio::test]
    async fn new_operation_with_stale_version_is_rejected_before_job_admission() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let guard = MutationGuard {
            operation: OperationId::for_test(30),
            expected: SessionVersion {
                session_id: None,
                cursor: None,
                journal_revision: 1,
            },
        };

        let admission = server
            .register_job("start", guard, b"start".to_vec(), None, None, None, None)
            .await
            .expect("classify stale version");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error {
                code,
                version: Some(actual),
                ..
            }) => {
                assert_eq!(code, "stale_version");
                assert_eq!(actual, SessionVersion::empty());
                assert!(server.latest_job().await.is_none());
            }
            JobAdmission::Accepted(_) | JobAdmission::Duplicate(_) => {
                panic!("stale session version admitted a job")
            }
            JobAdmission::Rejected(other) => panic!("wrong stale response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn successor_bind_is_unique_without_replacing_active_pointer() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let socket_dir = home.path().join("sockets");
        let _env = env_guard_os(vec![
            ("PLOKE_EVAL_HOME", OsString::from(home.path())),
            ("PLOKE_EVAL_WALK_SOCKET_DIR", OsString::from(&socket_dir)),
        ]);
        fs::create_dir_all(&socket_dir).expect("create socket dir");

        let active_path = socket_dir.join("active.sock");
        let active_listener =
            std::os::unix::net::UnixListener::bind(&active_path).expect("bind active endpoint");
        let active = ServerEndpoint::from_bound(repo.path().to_path_buf(), active_path)
            .expect("capture active endpoint");
        active.activate().expect("publish active endpoint");

        let first_id =
            RuntimeId::from_str("11111111-1111-4111-8111-111111111111").expect("first runtime id");
        let next_id =
            RuntimeId::from_str("22222222-2222-4222-8222-222222222222").expect("next runtime id");
        let first = prepare_successor(repo.path(), first_id).expect("prepare first successor");
        let next = prepare_successor(repo.path(), next_id).expect("prepare next successor");

        assert_ne!(first.endpoint().socket(), next.endpoint().socket());
        assert_ne!(first.endpoint().socket(), active.socket());
        assert!(first.endpoint().owns_socket());
        assert!(next.endpoint().owns_socket());
        assert_eq!(
            endpoint::load(repo.path()).expect("load active endpoint"),
            Some(active.clone()),
            "preparing successors must not publish either endpoint"
        );

        first.endpoint().cleanup().expect("cleanup first successor");
        next.endpoint().cleanup().expect("cleanup next successor");
        active.cleanup().expect("cleanup active endpoint");
        drop(active_listener);
    }

    #[tokio::test]
    async fn successor_rebind_rejects_live_and_replaces_stale_socket() {
        let repo = tempdir().expect("repo tempdir");
        let home = tempdir().expect("eval home tempdir");
        let socket_dir = home.path().join("sockets");
        let _env = env_guard_os(vec![
            ("PLOKE_EVAL_HOME", OsString::from(home.path())),
            ("PLOKE_EVAL_WALK_SOCKET_DIR", OsString::from(&socket_dir)),
        ]);
        let runtime_id =
            RuntimeId::from_str("33333333-3333-4333-8333-333333333333").expect("runtime id");
        let live = prepare_successor(repo.path(), runtime_id).expect("prepare live successor");
        let stale = live.endpoint().clone();

        let error = match prepare_successor(repo.path(), runtime_id) {
            Ok(_) => panic!("reachable successor socket was stolen"),
            Err(error) => error,
        };
        match error {
            PrepareError::InvalidBatchSelection { detail } => assert!(
                detail.contains("still reachable"),
                "wrong live-socket rejection: {detail}"
            ),
            other => panic!("wrong live-socket error: {other:?}"),
        }
        assert!(live.endpoint().owns_socket());

        drop(live);
        assert!(stale.owns_socket(), "dropped listener must leave its inode");
        let rebound =
            prepare_successor(repo.path(), runtime_id).expect("rebind stale successor socket");
        assert!(rebound.endpoint().owns_socket());
        assert_ne!(rebound.endpoint(), &stale);
        assert!(
            !stale.owns_socket(),
            "stale owner must not claim the rebound socket inode"
        );

        rebound
            .endpoint()
            .cleanup()
            .expect("cleanup rebound successor");
    }

    #[test]
    fn process_projection_keeps_runtime_identity() {
        let first =
            RuntimeId::from_str("44444444-4444-4444-8444-444444444444").expect("first runtime");
        let second =
            RuntimeId::from_str("55555555-5555-4555-8555-555555555555").expect("second runtime");
        let pid = 4242;
        let handoff = handoff_record(first, pid, "first");

        let projected = recorded_processes(vec![
            successor_record(first, pid, "first"),
            handoff.clone(),
            successor_record(second, pid, "second"),
        ]);
        assert_eq!(projected.len(), 2, "PID reuse must not dedupe runtimes");
        assert_eq!(projected[0].runtime_id, first);
        assert_eq!(projected[1].runtime_id, second);
        assert!(
            projected
                .iter()
                .all(|process| process.incarnation.is_some())
        );
        assert_eq!(
            projected[0].argv,
            successor_argv(
                PathBuf::from("/repo/first/ploke-eval"),
                "campaign",
                PathBuf::from("/repo/first"),
                PathBuf::from("/run/first.json"),
            ),
            "pre-Ready Spawned evidence must preserve exact launch argv"
        );

        let historical = recorded_processes(vec![handoff]);
        assert_eq!(
            historical.len(),
            1,
            "historical handoff remains inspectable"
        );
        assert_eq!(historical[0].runtime_id, first);
        assert!(
            historical[0].incarnation.is_none(),
            "handoff-only history must fail closed instead of inventing process identity"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_match_requires_exact_argv() {
        let runtime = RuntimeId::from_str("66666666-6666-4666-8666-666666666666").expect("runtime");
        let mut command = Command::new("/bin/sleep");
        command.arg("30").process_group(0);
        let mut child = command.spawn().expect("spawn isolated sleep");
        let incarnation = process_incarnation(child.id())
            .expect("read process incarnation")
            .expect("running process incarnation");
        let wrong = RecordedProcess {
            kind: "child",
            runtime_id: runtime,
            pid: child.id(),
            incarnation: Some(incarnation.clone()),
            argv: vec![OsString::from("/bin/sleep"), OsString::from("300")],
        };
        assert!(
            recorded_process_still_matches(&wrong)
                .expect("mismatch probe")
                .is_none(),
            "same binary with different argv must not match"
        );
        assert!(
            child.try_wait().expect("poll mismatched child").is_none(),
            "identity mismatch must not signal the process"
        );

        let reused = RecordedProcess {
            kind: "child",
            runtime_id: runtime,
            pid: child.id(),
            incarnation: Some(ProcessIncarnation {
                boot_id: incarnation.boot_id,
                start_ticks: incarnation.start_ticks.saturating_add(1),
            }),
            argv: vec![OsString::from("/bin/sleep"), OsString::from("30")],
        };
        assert!(
            recorded_process_still_matches(&reused)
                .expect("incarnation probe")
                .is_none(),
            "a reused PID must not match a different process incarnation"
        );

        let exact = RecordedProcess {
            argv: vec![OsString::from("/bin/sleep"), OsString::from("30")],
            ..wrong
        };
        let mut observed = None;
        for _ in 0..100 {
            observed = process_cmdline(child.id()).expect("read exact cmdline");
            if observed.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            observed.expect("running process cmdline"),
            exact.argv,
            "test process must expose the exact argv being validated"
        );
        let handle = recorded_process_still_matches(&exact)
            .expect("exact probe")
            .expect("exact process identity");
        if let Err(error) = terminate_process_group(&handle) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("pidfd process-group signal failed: {error}");
        }
        child.wait().expect("reap signalled child");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_match_requires_group_leader() {
        let runtime = RuntimeId::from_str("77777777-7777-4777-8777-777777777777").expect("runtime");
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn inherited-group sleep");
        let incarnation = process_incarnation(child.id())
            .expect("read process incarnation")
            .expect("running process incarnation");
        let process = RecordedProcess {
            kind: "child",
            runtime_id: runtime,
            pid: child.id(),
            incarnation: Some(incarnation),
            argv: vec![OsString::from("/bin/sleep"), OsString::from("30")],
        };
        assert!(
            recorded_process_still_matches(&process)
                .expect("group probe")
                .is_none(),
            "a recorded PID must still lead its own process group"
        );
        child.kill().expect("terminate test child");
        child.wait().expect("reap test child");
    }
}
