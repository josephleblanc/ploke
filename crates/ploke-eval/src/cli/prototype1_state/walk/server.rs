//! Long-running local typestate walk server.
//!
//! The server binds one Unix socket and owns one `WalkController` plus a
//! separately locked `LlmInspector`. Live `start`/`step` requests are admitted
//! as supervised background jobs so the socket can keep answering
//! `status`/health and persisted LLM inspection requests while rejecting
//! duplicate live mutations.
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
        Arc, RwLock as SyncRwLock,
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
use ploke_records::ids::CampaignId;

use tokio::{
    net::{UnixListener, UnixStream},
    sync::{Mutex, RwLock, mpsc},
    task::{JoinHandle, JoinSet},
    time,
};
use tracing::{debug, info, warn};

use crate::{
    campaign::campaign_manifest_path,
    cli::{
        Prototype1StateWalkLlmStepSource, Prototype1StateWalkServeCommand,
        prototype1_state::{
            driver::control::{
                PredecessorRelease, RecoveryDirective, ServerAdmission,
                recover_admitted_controller, recover_admitted_version, validate_fresh_session,
                walk_server_admission,
            },
            edge::ControlEdge,
            event::RuntimeId,
            identity,
            invocation::{ProcessIncarnation, process_incarnation},
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
            session::{Attempt, AttemptResult, Damage, SessionId, Store},
            successor,
        },
    },
    durable_io,
    layout::campaigns_dir,
    spec::PrepareError,
};

use super::{
    config,
    controller::{DeltaRenderStyle, LlmInspector, WalkController},
    endpoint::{self, ServerEndpoint},
    epoch::{ServerEpoch, WALK_PROTOCOL_VERSION},
    ipc, llm_trace, paths,
    phase::WalkPhase,
    protocol::{
        MutationGuard, OperationId, SessionVersion, WalkAction, WalkActionKind, WalkAuthority,
        WalkBlocker, WalkBlockerCode, WalkDeltaSnapshot, WalkDeltaState, WalkErrorCode,
        WalkEventProjection, WalkJobKind, WalkJobResolutionKind, WalkJobResolutionReceipt,
        WalkJobSnapshot, WalkJobStatus, WalkOkKind, WalkOkPayload, WalkPosition, WalkRequest,
        WalkRequestBody, WalkResponse, WalkSessionHistory, WalkSessionSnapshot, WalkStartConfig,
        WalkTransitionReceipt,
    },
    query::run_snapshot_query,
    trace,
};

/// Runtime state owned by one server process.
#[derive(Clone)]
struct WalkServer {
    epoch: ServerEpoch,
    controller: Arc<Mutex<WalkController>>,
    llm: Arc<Mutex<LlmInspector>>,
    delta: Arc<RwLock<PublishedDelta>>,
    jobs: Arc<Mutex<JobRegistry>>,
    gate: MutationGate,
    operation_root: PathBuf,
    observed: Arc<ControllerCache>,
}

/// Complete controller observation captured while holding the controller lock.
#[derive(Debug, Clone)]
struct ControllerObservation {
    phase: WalkPhase,
    attached: bool,
    blocker: Option<String>,
    fresh: FreshAdmission,
}

impl ControllerObservation {
    fn capture(controller: &WalkController, repo_root: &Path, session_exists: bool) -> Self {
        let phase = controller.phase();
        let fresh = if session_exists {
            FreshAdmission::Unchecked
        } else {
            match validate_fresh_session(repo_root) {
                Ok(()) => FreshAdmission::Ready,
                Err(error) => FreshAdmission::Blocked(error.to_string()),
            }
        };
        Self {
            phase,
            attached: phase != WalkPhase::Empty,
            blocker: controller.blocker_detail(),
            fresh,
        }
    }

    fn controller(controller: &WalkController) -> Self {
        let phase = controller.phase();
        Self {
            phase,
            attached: phase != WalkPhase::Empty,
            blocker: controller.blocker_detail(),
            fresh: FreshAdmission::Unchecked,
        }
    }

    fn unavailable() -> Self {
        Self {
            phase: WalkPhase::Empty,
            attached: false,
            blocker: Some(
                "controller observation cache is contended; retry status before mutating"
                    .to_string(),
            ),
            fresh: FreshAdmission::Blocked(
                "fresh-session admission is unavailable while the observation cache is contended"
                    .to_string(),
            ),
        }
    }
}

#[derive(Debug, Clone)]
enum FreshAdmission {
    Unchecked,
    Ready,
    Blocked(String),
}

/// Last complete observation, accessed only through bounded `try_*` locks so
/// status never parks an async executor thread behind another observer.
#[derive(Debug)]
struct ControllerCache {
    current: SyncRwLock<ControllerObservation>,
}

impl ControllerCache {
    fn new(observation: ControllerObservation) -> Self {
        Self {
            current: SyncRwLock::new(observation),
        }
    }

    fn read(&self) -> ControllerObservation {
        for _ in 0..4 {
            if let Ok(observation) = self.current.try_read() {
                return observation.clone();
            }
            std::hint::spin_loop();
        }
        ControllerObservation::unavailable()
    }

    fn update(&self, mut observation: ControllerObservation) {
        for _ in 0..4 {
            if let Ok(mut current) = self.current.try_write() {
                if matches!(observation.fresh, FreshAdmission::Unchecked) {
                    observation.fresh = current.fresh.clone();
                }
                *current = observation;
                return;
            }
            std::hint::spin_loop();
        }
    }
}

/// Immutable response material captured after a controller advance completes.
///
/// Keeping both the typed snapshot and its style variants here lets delta
/// inspection remain read-only while a later live job owns the controller.
#[derive(Clone)]
struct PublishedDelta {
    source_job: Option<u64>,
    phase: WalkPhase,
    snapshot: WalkDeltaSnapshot,
    plain: String,
    verbose: String,
    color: String,
    verbose_color: String,
}

impl PublishedDelta {
    fn capture(
        controller: &WalkController,
        observed: SessionVersion,
        source_job: Option<u64>,
    ) -> Result<Self, PrepareError> {
        Ok(Self {
            source_job,
            phase: controller.phase(),
            snapshot: controller.delta_snapshot(observed)?,
            plain: controller.delta_report(DeltaRenderStyle {
                verbose: false,
                color: false,
            }),
            verbose: controller.delta_report(DeltaRenderStyle {
                verbose: true,
                color: false,
            }),
            color: controller.delta_report(DeltaRenderStyle {
                verbose: false,
                color: true,
            }),
            verbose_color: controller.delta_report(DeltaRenderStyle {
                verbose: true,
                color: true,
            }),
        })
    }

    fn not_recorded(phase: WalkPhase, version: SessionVersion, source_job: Option<u64>) -> Self {
        let message = "no previous step delta; run `walk step` first".to_string();
        Self {
            source_job,
            phase,
            snapshot: WalkDeltaSnapshot {
                version,
                state: WalkDeltaState::NotRecorded,
            },
            plain: message.clone(),
            verbose: message.clone(),
            color: message.clone(),
            verbose_color: message,
        }
    }

    fn report(&self, style: DeltaRenderStyle) -> &str {
        match (style.verbose, style.color) {
            (false, false) => &self.plain,
            (true, false) => &self.verbose,
            (false, true) => &self.color,
            (true, true) => &self.verbose_color,
        }
    }
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
    restore: JobRestoreScope,
}

impl PreparedServer {
    pub(crate) fn endpoint(&self) -> &ServerEndpoint {
        &self.endpoint
    }
}

#[derive(Clone, Copy)]
enum JobRestoreScope {
    Repository,
    Successor { predecessor: SessionId },
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
    restored: bool,
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

fn restore_job_registry(
    operation_root: &Path,
    epoch: &ServerEpoch,
    scope: JobRestoreScope,
    session: Option<SessionId>,
) -> Result<JobRegistry, PrepareError> {
    let (current, predecessor) = match scope {
        JobRestoreScope::Repository => (None, None),
        JobRestoreScope::Successor { predecessor } => {
            let current = session.ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "successor walk service has no durable controller session".to_string(),
            })?;
            (Some(current), Some(predecessor))
        }
    };
    let mut jobs = JobRegistry::default();
    for entry in fs::read_dir(operation_root)
        .map_err(|source| operation_error("scan", operation_root, source))?
    {
        let entry = entry.map_err(|source| operation_error("scan", operation_root, source))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|source| operation_error("inspect", &path, source))?
            .is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let operation = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "durable operation record '{}' has no UTF-8 operation-id filename",
                    path.display()
                ),
            })?
            .parse::<OperationId>()
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "durable operation record '{}' has an invalid operation-id filename: {source}",
                    path.display()
                ),
            })?;
        let record = read_operation_record(&path, operation, epoch)?;
        jobs.next_id = jobs.next_id.max(record.stored.snapshot.job_id);
        let from_predecessor = predecessor
            .is_some_and(|session| record.stored.snapshot.expected.session_id() == Some(session));
        if from_predecessor {
            // A closed-gate successor may inspect its durable session while
            // the exact predecessor still publishes its terminal job receipt.
            // Retain only that exact predecessor operation for lookup without
            // converting it into the successor controller's recovery blocker.
            // Any unrelated unresolved session still follows the repository
            // fail-closed path below.
            jobs.completed.insert(operation, record.stored);
            continue;
        }
        let unrelated = current
            .is_some_and(|session| record.stored.snapshot.expected.session_id() != Some(session));
        if unrelated && record.stored.snapshot.status.blocks_mutation() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor walk service found unresolved operation {operation} from an unrelated controller session"
                ),
            });
        }
        if record.stored.snapshot.status.blocks_mutation() {
            if let Some(active) = jobs.active.as_ref() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "durable operation records contain multiple unresolved jobs: {} and {}",
                        active.snapshot.operation_id, operation
                    ),
                });
            }
            let prior_status = record.stored.snapshot.status;
            let mut snapshot = record.stored.snapshot;
            if snapshot.status.is_active() {
                snapshot.status = WalkJobStatus::Indeterminate;
                snapshot.updated_at = now_rfc3339();
                snapshot.message = Some(format!(
                    "walk server restarted while operation {operation} was {:?}; effects may have occurred, so inspect evidence and explicitly abandon this job before admitting more mutation",
                    prior_status
                ));
            }
            jobs.active = Some(ActiveWalkJob {
                snapshot,
                fingerprint: record.stored.fingerprint,
                handle: None,
                restored: true,
            });
        } else {
            jobs.completed.insert(operation, record.stored);
        }
    }
    Ok(jobs)
}

fn read_operation_record(
    path: &Path,
    operation: OperationId,
    epoch: &ServerEpoch,
) -> Result<DurableOperation, PrepareError> {
    let bytes = fs::read(path).map_err(|source| operation_error("read", path, source))?;
    decode_operation_record(path, operation, epoch, &bytes)
}

fn decode_operation_record(
    path: &Path,
    operation: OperationId,
    epoch: &ServerEpoch,
    bytes: &[u8],
) -> Result<DurableOperation, PrepareError> {
    let record: DurableOperation =
        serde_json::from_slice(bytes).map_err(|source| PrepareError::DatabaseSetup {
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
    if record.epoch.repo_root != epoch.repo_root {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "durable operation record '{}' belongs to repository '{}' rather than '{}'",
                path.display(),
                record.epoch.repo_root.display(),
                epoch.repo_root.display()
            ),
        });
    }
    Ok(record)
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

enum TransitionFailure {
    Attempt(PrepareError),
    Receipt(PrepareError),
}

struct JobIntent {
    command: WalkJobKind,
    target_phase: Option<WalkPhase>,
    watch: Option<bool>,
    allow_live_api: Option<bool>,
    allow_git_changes: Option<bool>,
    llm_source: Option<Prototype1StateWalkLlmStepSource>,
    allow_workspace_mutation: Option<bool>,
    allow_provenance_record: Option<bool>,
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
    predecessor: SessionId,
) -> Result<PreparedServer, PrepareError> {
    let mut prepared = prepare(
        Prototype1StateWalkServeCommand {
            repo_root: Some(repo_root.to_path_buf()),
            socket: Some(paths::successor_socket(repo_root, runtime_id)?),
            ttl_secs: None,
            no_ttl: true,
        },
        false,
    )?;
    prepared.restore = JobRestoreScope::Successor { predecessor };
    Ok(prepared)
}

/// Probe whether the deterministic socket slot named by persisted Ready
/// evidence is currently served. A restarted successor may own a new inode at
/// the same runtime-scoped path without minting a second Ready receipt.
pub(crate) fn endpoint_reachable(endpoint: &ServerEndpoint) -> Result<bool, PrepareError> {
    endpoint.validate_persisted()?;
    socket_reachable(endpoint.socket())
}

/// Wait until an already-bound successor endpoint answers a protocol Health
/// request, rather than treating a successful Unix connect as service Ready.
pub(crate) async fn await_responsive(
    endpoint: &ServerEndpoint,
    timeout: Duration,
) -> Result<(), PrepareError> {
    const POLL: Duration = Duration::from_millis(10);
    const REQUEST: Duration = Duration::from_millis(250);

    endpoint.validate_persisted()?;
    let deadline = time::Instant::now() + timeout;
    let mut detail = "successor Health has not responded".to_string();
    loop {
        if !endpoint.owns_socket() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor endpoint '{}' stopped before service Ready: {detail}",
                    endpoint.socket().display()
                ),
            });
        }
        let now = time::Instant::now();
        if now >= deadline {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "successor endpoint '{}' did not answer Health before service Ready: {detail}",
                    endpoint.socket().display()
                ),
            });
        }
        let request = REQUEST.min(deadline - now);
        let response = time::timeout(request, async {
            let mut stream = ipc::connect(endpoint.socket()).await?;
            ipc::send(
                &mut stream,
                &WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: None,
                    body: WalkRequestBody::Health,
                },
            )
            .await?;
            ipc::recv(&mut stream).await
        })
        .await;
        match response {
            Ok(Ok(WalkResponse::Status { .. })) => return Ok(()),
            Ok(Ok(WalkResponse::Error {
                detail: response, ..
            })) => detail = response,
            Ok(Ok(response)) => detail = format!("unexpected Health response: {response:?}"),
            Ok(Err(error)) => detail = error.to_string(),
            Err(_) => detail = "successor Health request timed out".to_string(),
        }
        time::sleep(POLL).await;
    }
}

/// Refresh the closed-gate successor after its bootstrap lease atomically
/// releases Ready, and require the service to expose the exact R4c boundary.
pub(crate) async fn refresh_ready(
    endpoint: &ServerEndpoint,
    timeout: Duration,
) -> Result<(), PrepareError> {
    endpoint.validate_persisted()?;
    let response = time::timeout(timeout, async {
        let mut stream = ipc::connect(endpoint.socket()).await?;
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_protocol: Some(WALK_PROTOCOL_VERSION),
                client_epoch: None,
                body: WalkRequestBody::Show,
            },
        )
        .await?;
        ipc::recv(&mut stream).await
    })
    .await
    .map_err(|_| PrepareError::InvalidBatchSelection {
        detail: format!(
            "successor endpoint '{}' did not refresh released Ready before publication",
            endpoint.socket().display()
        ),
    })??;
    match response {
        WalkResponse::Ok {
            phase: WalkPhase::R4c,
            result: WalkOkPayload::Show { .. },
            ..
        } => Ok(()),
        WalkResponse::Error { detail, .. } => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor endpoint '{}' rejected its released Ready refresh: {detail}",
                endpoint.socket().display()
            ),
        }),
        response => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "successor endpoint '{}' did not expose released R4c Ready: {response:?}",
                endpoint.socket().display()
            ),
        }),
    }
}

/// Stop the exact predecessor listener after its fenced handoff release.
///
/// The predecessor may still be publishing the terminal walk-operation receipt
/// for the R12->R13b job when controller authority is released, so a bounded
/// retry treats `job active` as transfer drain rather than as permission to
/// leave two live mutation endpoints behind.
pub(crate) async fn retire_predecessor(predecessor: &ServerEndpoint) -> Result<(), PrepareError> {
    // Match the existing successor transfer scale while preserving bounded
    // failure when the predecessor service cannot settle cleanly.
    const RETIRE_TIMEOUT: Duration = Duration::from_secs(30);
    const RETIRE_POLL: Duration = Duration::from_millis(25);

    predecessor.validate_persisted()?;
    let repo_root = predecessor.repo_root().to_path_buf();
    let client_epoch = match time::timeout(
        RETIRE_TIMEOUT,
        tokio::task::spawn_blocking(move || ServerEpoch::capture(&repo_root)),
    )
    .await
    {
        Ok(Ok(epoch)) => epoch?,
        Ok(Err(source)) => {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_successor_stop_epoch_join",
                detail: source.to_string(),
            });
        }
        Err(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor endpoint '{}' did not retire after handoff release: repository epoch capture timed out",
                    predecessor.socket().display()
                ),
            });
        }
    };
    let deadline = time::Instant::now() + RETIRE_TIMEOUT;
    let mut detail = "no stop response".to_string();
    loop {
        if !predecessor.owns_socket() {
            if socket_reachable(predecessor.socket())? {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "predecessor endpoint '{}' changed ownership before retirement",
                        predecessor.socket().display()
                    ),
                });
            }
            return Ok(());
        }
        if !socket_reachable(predecessor.socket())? {
            predecessor.cleanup()?;
            return Ok(());
        }
        let now = time::Instant::now();
        if now >= deadline {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor endpoint '{}' did not retire after handoff release: {detail}",
                    predecessor.socket().display()
                ),
            });
        }

        let response = match time::timeout(deadline - now, async {
            let mut stream = ipc::connect(predecessor.socket()).await?;
            ipc::send(
                &mut stream,
                &WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(client_epoch.clone()),
                    body: WalkRequestBody::Stop,
                },
            )
            .await?;
            ipc::recv(&mut stream).await
        })
        .await
        {
            Ok(response) => response,
            Err(_) => {
                detail = format!(
                    "predecessor endpoint '{}' did not answer Stop before the handoff drain deadline",
                    predecessor.socket().display()
                );
                continue;
            }
        };

        match response {
            Ok(WalkResponse::Error {
                code: WalkErrorCode::JobActive,
                detail: response,
                ..
            }) => detail = response,
            Ok(WalkResponse::Error {
                code,
                detail: response,
                ..
            }) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "predecessor endpoint '{}' rejected retirement with {code}: {response}",
                        predecessor.socket().display()
                    ),
                });
            }
            Ok(WalkResponse::Status { snapshot, .. })
                if snapshot.authority == WalkAuthority::Stopping =>
            {
                detail = "predecessor accepted Stop but kept its socket".to_string();
            }
            Ok(response) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "predecessor endpoint '{}' returned an unexpected Stop response: {response:?}",
                        predecessor.socket().display()
                    ),
                });
            }
            Err(error @ PrepareError::DatabaseSetup { .. }) => {
                detail = format!("predecessor Stop transport remained unsettled: {error}");
            }
            Err(error) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "predecessor endpoint '{}' returned an invalid Stop response: {error}",
                        predecessor.socket().display()
                    ),
                });
            }
        }

        if time::Instant::now() >= deadline {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "predecessor endpoint '{}' did not retire after handoff release: {detail}",
                    predecessor.socket().display(),
                ),
            });
        }
        time::sleep(RETIRE_POLL).await;
    }
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
        restore: JobRestoreScope::Repository,
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
        restore,
    } = prepared;
    let result = async {
        if publish {
            endpoint.activate()?;
        }
        let repo_root = endpoint.repo_root().to_path_buf();
        let mut controller = WalkController::new(repo_root.clone());
        controller.refresh_from_disk()?;
        let version = durable_version_for(endpoint.repo_root())?;
        let delta = PublishedDelta::capture(&controller, version.clone(), None)?;
        let operation_root = paths::operation_dir(endpoint.repo_root())?;
        paths::ensure_operation_dir(&operation_root)?;
        let observation =
            ControllerObservation::capture(&controller, &repo_root, version.session_id().is_some());
        let observed = Arc::new(ControllerCache::new(observation));
        let jobs = restore_job_registry(&operation_root, &epoch, restore, version.session_id())?;
        let server = WalkServer {
            epoch,
            controller: Arc::new(Mutex::new(controller)),
            llm: Arc::new(Mutex::new(LlmInspector::new(repo_root))),
            delta: Arc::new(RwLock::new(delta)),
            jobs: Arc::new(Mutex::new(jobs)),
            gate,
            operation_root,
            observed,
        };
        accept_loop(server, listener, idle_ttl).await
    }
    .await;
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

const REQUEST_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Accept client connections concurrently and isolate connection failures.
async fn accept_loop(
    server: WalkServer,
    listener: UnixListener,
    idle_ttl: Option<Duration>,
) -> Result<(), PrepareError> {
    let (stop_tx, mut stop_rx) = mpsc::unbounded_channel();
    let mut connections = JoinSet::new();
    let idle_wait = idle_ttl.unwrap_or(Duration::from_secs(86_400));
    let idle_sleep = time::sleep(idle_wait);
    tokio::pin!(idle_sleep);

    loop {
        tokio::select! {
            Some(()) = stop_rx.recv() => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|source| PrepareError::DatabaseSetup {
                    phase: "prototype1_state_walk_accept",
                    detail: source.to_string(),
                })?;
                if let Some(ttl) = idle_ttl {
                    idle_sleep.as_mut().reset(time::Instant::now() + ttl);
                }
                let connection_server = server.clone();
                let connection_stop = stop_tx.clone();
                connections.spawn(async move {
                    match handle_stream(&connection_server, stream).await {
                        Ok(true) => {
                            let _ = connection_stop.send(());
                        }
                        Ok(false) => {}
                        Err(error) => {
                            warn!(error = ?error, "walk client connection failed");
                        }
                    }
                });
            }
            _ = &mut idle_sleep, if idle_ttl.is_some() => {
                let ttl = idle_ttl.expect("idle TTL branch requires a duration");
                if let Some(job) = server.active_job().await {
                    info!(
                        idle_ttl_secs = ttl.as_secs(),
                        job_id = job.job_id,
                        command = %job.command,
                        "walk server idle TTL elapsed while an admitted job remains active"
                    );
                    idle_sleep.as_mut().reset(time::Instant::now() + ttl);
                    continue;
                }
                info!(idle_ttl_secs = ttl.as_secs(), "walk server idle TTL expired");
                break;
            }
            Some(joined) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = joined {
                    warn!(error = ?error, "walk client connection task failed");
                }
            }
        }
    }

    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

/// Read one request from a connected socket and write one response.
async fn handle_stream(server: &WalkServer, mut stream: UnixStream) -> Result<bool, PrepareError> {
    let request: WalkRequest = match time::timeout(REQUEST_FRAME_TIMEOUT, ipc::recv(&mut stream))
        .await
    {
        Ok(Ok(request)) => request,
        Ok(Err(error)) => {
            let phase = server.phase_for_response().await;
            let response = WalkResponse::error(
                WalkErrorCode::BadRequest,
                error.to_string(),
                Some(phase),
                server.epoch.clone(),
            );
            let _ = time::timeout(RESPONSE_WRITE_TIMEOUT, ipc::send(&mut stream, &response)).await;
            return Ok(false);
        }
        Err(_) => {
            let phase = server.phase_for_response().await;
            let response = WalkResponse::error(
                WalkErrorCode::BadRequest,
                format!(
                    "walk IPC request frame was not completed within {} seconds",
                    REQUEST_FRAME_TIMEOUT.as_secs()
                ),
                Some(phase),
                server.epoch.clone(),
            );
            let _ = time::timeout(RESPONSE_WRITE_TIMEOUT, ipc::send(&mut stream, &response)).await;
            return Ok(false);
        }
    };
    if matches!(
        &request.body,
        WalkRequestBody::Health
            | WalkRequestBody::Show
            | WalkRequestBody::LlmTraceIndex
            | WalkRequestBody::LlmTrace { .. }
    ) && request.client_protocol != Some(server.epoch.protocol_version)
    {
        let phase = server
            .durable_version()
            .map(|version| version.phase())
            .unwrap_or_else(|_| server.observed.read().phase);
        let client = request
            .client_protocol
            .map_or_else(|| "missing".to_string(), |protocol| protocol.to_string());
        let response = WalkResponse::error(
            WalkErrorCode::BadRequest,
            format!(
                "walk protocol mismatch: client={client} server={}; restart the walk client before reading structured status",
                server.epoch.protocol_version
            ),
            Some(phase),
            server.epoch.clone(),
        );
        let _ = time::timeout(RESPONSE_WRITE_TIMEOUT, ipc::send(&mut stream, &response)).await;
        return Ok(false);
    }
    let (response, stop) = server.handle(request).await;
    match time::timeout(RESPONSE_WRITE_TIMEOUT, ipc::send(&mut stream, &response)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(error = ?error, stop, "walk client disconnected before receiving its response");
        }
        Err(_) => {
            warn!(stop, "walk client response write timed out");
        }
    }
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
                    WalkErrorCode::TransferPending,
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
            WalkRequestBody::Config => {
                let phase = self.phase_for_response().await;
                config::load(&self.epoch.repo_root)
                    .map(|config| WalkResponse::config(phase, config, self.epoch.clone()))
            }
            WalkRequestBody::EvaluationTraceIndex => Ok(match self.durable_version() {
                Ok(version) => {
                    let phase = version.phase();
                    trace::load_index(&self.epoch.repo_root, version, self.epoch.clone())
                        .map_or_else(
                            |error| {
                                WalkResponse::error(
                                    request_error_code(&error),
                                    error.to_string(),
                                    Some(phase),
                                    self.epoch.clone(),
                                )
                            },
                            WalkResponse::evaluation_trace_index,
                        )
                }
                Err(error) => WalkResponse::error(
                    request_error_code(&error),
                    error.to_string(),
                    None,
                    self.epoch.clone(),
                ),
            }),
            WalkRequestBody::EvaluationTrace { coordinate } => Ok(match self.durable_version() {
                Ok(version) => {
                    let phase = version.phase();
                    trace::load_run(
                        &self.epoch.repo_root,
                        coordinate,
                        version,
                        self.epoch.clone(),
                    )
                    .map_or_else(
                        |error| {
                            WalkResponse::error(
                                request_error_code(&error),
                                error.to_string(),
                                Some(phase),
                                self.epoch.clone(),
                            )
                        },
                        WalkResponse::evaluation_trace,
                    )
                }
                Err(error) => WalkResponse::error(
                    request_error_code(&error),
                    error.to_string(),
                    None,
                    self.epoch.clone(),
                ),
            }),
            WalkRequestBody::LlmTraceIndex => Ok(match self.durable_version() {
                Ok(version) => {
                    let phase = version.phase();
                    llm_trace::load_index(&self.epoch.repo_root, version, self.epoch.clone())
                        .map_or_else(
                            |error| {
                                WalkResponse::error(
                                    request_error_code(&error),
                                    error.to_string(),
                                    Some(phase),
                                    self.epoch.clone(),
                                )
                            },
                            WalkResponse::llm_trace_index,
                        )
                }
                Err(error) => WalkResponse::error(
                    request_error_code(&error),
                    error.to_string(),
                    None,
                    self.epoch.clone(),
                ),
            }),
            WalkRequestBody::LlmTrace { coordinate } => Ok(match self.durable_version() {
                Ok(version) => {
                    let phase = version.phase();
                    llm_trace::load_trace(
                        &self.epoch.repo_root,
                        coordinate,
                        version,
                        self.epoch.clone(),
                    )
                    .map_or_else(
                        |error| {
                            WalkResponse::error(
                                request_error_code(&error),
                                error.to_string(),
                                Some(phase),
                                self.epoch.clone(),
                            )
                        },
                        WalkResponse::llm_trace,
                    )
                }
                Err(error) => WalkResponse::error(
                    request_error_code(&error),
                    error.to_string(),
                    None,
                    self.epoch.clone(),
                ),
            }),
            WalkRequestBody::SessionHistory => {
                durable_history_for(&self.epoch.repo_root, self.epoch.clone())
                    .map(WalkResponse::history)
            }
            WalkRequestBody::OperationStatus { operation } => {
                match self.lookup_operation(operation).await {
                    Ok(Some(job)) if job.status.blocks_mutation() => {
                        self.durable_version().map(|version| {
                            WalkResponse::job(
                                version.phase(),
                                job,
                                "supervised operation status",
                                self.epoch.clone(),
                            )
                        })
                    }
                    Ok(Some(job)) => Ok(WalkResponse::job(
                        job_phase(&job),
                        job,
                        "supervised operation status",
                        self.epoch.clone(),
                    )),
                    Ok(None) => Err(PrepareError::InvalidBatchSelection {
                        detail: format!("walk operation {operation} was not found"),
                    }),
                    Err(error) => Err(error),
                }
            }
            WalkRequestBody::ShowDelta { verbose, color } => {
                let delta = self.delta.read().await;
                Ok(WalkResponse::delta(
                    delta.phase,
                    delta
                        .report(DeltaRenderStyle { verbose, color })
                        .to_string(),
                    delta.snapshot.clone(),
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
                        self.observed
                            .update(ControllerObservation::controller(&controller));
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
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_lanes_report(verbose).map(|message| {
                    WalkResponse::ok(WalkOkKind::LlmLanes, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::LlmFocus { lane } => {
                let phase = self.phase_for_response().await;
                let mut llm = self.llm.lock().await;
                llm.llm_focus(lane).map(|message| {
                    WalkResponse::ok(WalkOkKind::LlmFocus, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::LlmShow {
                session_id,
                lane,
                head,
                step,
            } => {
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_report(session_id.as_deref(), lane.as_deref(), head, step)
                    .map(|message| {
                        WalkResponse::ok(WalkOkKind::LlmShow, phase, message, self.epoch.clone())
                    })
            }
            WalkRequestBody::LlmTimeline { session_id, lane } => {
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_timeline(session_id.as_deref(), lane.as_deref())
                    .map(|message| {
                        WalkResponse::ok(
                            WalkOkKind::LlmTimeline,
                            phase,
                            message,
                            self.epoch.clone(),
                        )
                    })
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
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_prompt_report(
                    session_id.as_deref(),
                    lane.as_deref(),
                    step,
                    role.as_deref(),
                    message,
                    full,
                    json,
                )
                .map(|message| {
                    WalkResponse::ok(WalkOkKind::LlmPrompt, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::LlmProtocol {
                session_id,
                lane,
                json,
            } => {
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_protocol_report(session_id.as_deref(), lane.as_deref(), json)
                    .map(|message| {
                        WalkResponse::ok(
                            WalkOkKind::LlmProtocol,
                            phase,
                            message,
                            self.epoch.clone(),
                        )
                    })
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
                let phase = self.phase_for_response().await;
                let llm = self.llm.lock().await;
                llm.llm_tool_report(
                    session_id.as_deref(),
                    lane.as_deref(),
                    head,
                    step,
                    call,
                    name.as_deref(),
                    json,
                )
                .map(|message| {
                    WalkResponse::ok(WalkOkKind::LlmTool, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::LlmStep {
                guard,
                session_id,
                lane,
                step,
                source,
                watch,
                allow_workspace_mutation,
                model_id,
                provider,
                max_attempts,
                timeout_secs,
            } => {
                self.submit_llm_step(
                    request.client_epoch.as_ref(),
                    guard,
                    session_id,
                    lane,
                    step,
                    source,
                    watch,
                    allow_workspace_mutation,
                    model_id,
                    provider,
                    max_attempts,
                    timeout_secs,
                )
                .await
            }
            WalkRequestBody::LlmFinish {
                guard,
                session_id,
                lane,
                step,
                watch,
                allow_workspace_mutation,
                model_id,
                provider,
                max_steps,
                max_attempts,
                timeout_secs,
            } => {
                self.submit_llm_finish(
                    request.client_epoch.as_ref(),
                    guard,
                    session_id,
                    lane,
                    step,
                    watch,
                    allow_workspace_mutation,
                    model_id,
                    provider,
                    max_steps,
                    max_attempts,
                    timeout_secs,
                )
                .await
            }
            WalkRequestBody::LlmBack { lane, steps } => {
                let phase = self.phase_for_response().await;
                let mut llm = self.llm.lock().await;
                llm.llm_move(lane.as_deref(), steps, super::controller::LlmMove::Back)
                    .map(|message| {
                        WalkResponse::ok(WalkOkKind::LlmBack, phase, message, self.epoch.clone())
                    })
            }
            WalkRequestBody::LlmForward { lane, steps } => {
                let phase = self.phase_for_response().await;
                let mut llm = self.llm.lock().await;
                llm.llm_move(lane.as_deref(), steps, super::controller::LlmMove::Forward)
                    .map(|message| {
                        WalkResponse::ok(WalkOkKind::LlmForward, phase, message, self.epoch.clone())
                    })
            }
            WalkRequestBody::LlmHead { lane } => {
                let phase = self.phase_for_response().await;
                let mut llm = self.llm.lock().await;
                llm.llm_head(lane.as_deref()).map(|message| {
                    WalkResponse::ok(WalkOkKind::LlmHead, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::Replay { index, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller.replay_report(index, tail).map(|message| {
                    WalkResponse::ok(WalkOkKind::Replay, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::ReplayBack { steps, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller.replay_back(steps, tail).map(|message| {
                    WalkResponse::ok(WalkOkKind::ReplayBack, phase, message, self.epoch.clone())
                })
            }
            WalkRequestBody::ReplayForward { steps, tail } => {
                let mut controller = self.controller.lock().await;
                let phase = controller.phase();
                controller.replay_forward(steps, tail).map(|message| {
                    WalkResponse::ok(
                        WalkOkKind::ReplayForward,
                        phase,
                        message,
                        self.epoch.clone(),
                    )
                })
            }
            WalkRequestBody::BranchLive {
                guard,
                reason,
                allow_provenance_record,
            } => {
                self.submit_branch_live(
                    request.client_epoch.as_ref(),
                    guard,
                    reason,
                    allow_provenance_record,
                )
                .await
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
            WalkRequestBody::ResolveJob { guard, resolution } => {
                self.submit_resolve_job(request.client_epoch.as_ref(), guard, resolution)
                    .await
            }
            WalkRequestBody::Files => {
                let controller = self.controller.lock().await;
                let phase = controller.phase();
                Ok(WalkResponse::ok(
                    WalkOkKind::Files,
                    phase,
                    controller.files_report(),
                    self.epoch.clone(),
                ))
            }
            WalkRequestBody::DbQuery { campaign, script } => {
                self.query_response(campaign, script).await
            }
        };
        match result {
            Ok(response)
                if stop_requested
                    && matches!(
                        &response,
                        WalkResponse::Status { snapshot, .. }
                            if snapshot.authority == WalkAuthority::Stopping
                    ) =>
            {
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
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::Start,
                    target_phase: Some(until),
                    watch: None,
                    allow_live_api: Some(allow_live_api),
                    allow_git_changes: None,
                    llm_source: None,
                    allow_workspace_mutation: None,
                    allow_provenance_record: None,
                },
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
        let llm = Arc::clone(&self.llm);
        let delta = Arc::clone(&self.delta);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let operation_root = self.operation_root.clone();
        let observed = Arc::clone(&self.observed);
        let job_id = job.job_id;
        let expected = job.expected.clone();
        let handle = tokio::spawn(run_start_job(
            controller,
            llm,
            delta,
            jobs,
            epoch,
            operation_root,
            job_id,
            expected,
            config,
            until,
            allow_live_api,
            observed,
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
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::Step,
                    target_phase: until,
                    watch: Some(watch),
                    allow_live_api: Some(allow_live_api),
                    allow_git_changes: Some(allow_git_changes),
                    llm_source: None,
                    allow_workspace_mutation: None,
                    allow_provenance_record: None,
                },
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
        let delta = Arc::clone(&self.delta);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let operation_root = self.operation_root.clone();
        let observed = Arc::clone(&self.observed);
        let job_id = job.job_id;
        let expected = job.expected.clone();
        let handle = tokio::spawn(run_step_job(
            controller,
            delta,
            jobs,
            epoch,
            operation_root,
            job_id,
            expected,
            until,
            watch,
            allow_live_api,
            allow_git_changes,
            observed,
        ));
        self.attach_job_handle(job_id, handle).await;
        Ok(WalkResponse::job(
            job_phase(&job),
            job,
            "accepted step job; use `walk status` to inspect progress",
            self.epoch.clone(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_llm_step(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        session_id: Option<String>,
        lane: Option<String>,
        step: Option<usize>,
        source: Prototype1StateWalkLlmStepSource,
        watch: bool,
        allow_workspace_mutation: bool,
        model_id: Option<String>,
        provider: Option<String>,
        max_attempts: u32,
        timeout_secs: u64,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let fingerprint = request_fingerprint(&(
            "llm_step",
            &session_id,
            &lane,
            step,
            source,
            watch,
            allow_workspace_mutation,
            &model_id,
            &provider,
            max_attempts,
            timeout_secs,
        ))?;
        let job = match self
            .register_job(
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::LlmStep,
                    target_phase: None,
                    watch: Some(watch),
                    allow_live_api: Some(source == Prototype1StateWalkLlmStepSource::Live),
                    allow_git_changes: None,
                    llm_source: Some(source),
                    allow_workspace_mutation: Some(allow_workspace_mutation),
                    allow_provenance_record: None,
                },
            )
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing llm_step operation; no duplicate tool/provider work submitted",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };
        let llm = Arc::clone(&self.llm);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let operation_root = self.operation_root.clone();
        let job_id = job.job_id;
        let phase = job_phase(&job);
        let handle = tokio::spawn(run_llm_step_job(
            llm,
            jobs,
            epoch,
            operation_root,
            job_id,
            phase,
            session_id,
            lane,
            step,
            source,
            watch,
            allow_workspace_mutation,
            model_id,
            provider,
            max_attempts,
            timeout_secs,
        ));
        self.attach_job_handle(job_id, handle).await;
        Ok(WalkResponse::job(
            job_phase(&job),
            job,
            "accepted llm_step job; use `walk status` to inspect progress",
            self.epoch.clone(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_llm_finish(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        session_id: Option<String>,
        lane: Option<String>,
        step: Option<usize>,
        watch: bool,
        allow_workspace_mutation: bool,
        model_id: Option<String>,
        provider: Option<String>,
        max_steps: usize,
        max_attempts: u32,
        timeout_secs: u64,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let fingerprint = request_fingerprint(&(
            "llm_finish",
            &session_id,
            &lane,
            step,
            watch,
            allow_workspace_mutation,
            &model_id,
            &provider,
            max_steps,
            max_attempts,
            timeout_secs,
        ))?;
        let job = match self
            .register_job(
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::LlmFinish,
                    target_phase: None,
                    watch: Some(watch),
                    allow_live_api: Some(true),
                    allow_git_changes: None,
                    llm_source: Some(Prototype1StateWalkLlmStepSource::Live),
                    allow_workspace_mutation: Some(allow_workspace_mutation),
                    allow_provenance_record: None,
                },
            )
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing llm_finish operation; no duplicate provider work submitted",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };
        let llm = Arc::clone(&self.llm);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let operation_root = self.operation_root.clone();
        let job_id = job.job_id;
        let phase = job_phase(&job);
        let handle = tokio::spawn(run_llm_finish_job(
            llm,
            jobs,
            epoch,
            operation_root,
            job_id,
            phase,
            session_id,
            lane,
            step,
            watch,
            allow_workspace_mutation,
            model_id,
            provider,
            max_steps,
            max_attempts,
            timeout_secs,
        ));
        self.attach_job_handle(job_id, handle).await;
        Ok(WalkResponse::job(
            job_phase(&job),
            job,
            "accepted llm_finish job; use `walk status` to inspect progress",
            self.epoch.clone(),
        ))
    }

    async fn submit_branch_live(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        reason: String,
        allow_provenance_record: bool,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let operation = guard.operation;
        let fingerprint = request_fingerprint(&("branch_live", &reason, allow_provenance_record))?;
        let job = match self
            .register_job(
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::BranchLive,
                    target_phase: None,
                    watch: None,
                    allow_live_api: None,
                    allow_git_changes: None,
                    llm_source: None,
                    allow_workspace_mutation: None,
                    allow_provenance_record: Some(allow_provenance_record),
                },
            )
            .await?
        {
            JobAdmission::Accepted(job) => job,
            JobAdmission::Duplicate(job) => {
                return Ok(WalkResponse::job(
                    job_phase(&job),
                    job,
                    "attached to the existing branch_live operation; no duplicate provenance write performed",
                    self.epoch.clone(),
                ));
            }
            JobAdmission::Rejected(response) => return Ok(response),
        };
        let (status, message) = if allow_provenance_record {
            match self.controller.lock().await.record_replay_branch(reason) {
                Ok(message) => (WalkJobStatus::Succeeded, message),
                Err(error) => (
                    WalkJobStatus::Indeterminate,
                    format!(
                        "branch-live provenance write failed after explicit admission and may be partially durable: {error}; inspect replay provenance before abandoning this job"
                    ),
                ),
            }
        } else {
            (
                WalkJobStatus::Failed,
                "walk branch-live writes provenance; rerun with `--allow provenance-record`"
                    .to_string(),
            )
        };
        finish_job(
            &self.jobs,
            &self.operation_root,
            &self.epoch,
            job.job_id,
            status,
            Some(job.phase_before),
            message,
            None,
        )
        .await;
        let completed = self.operation_job(operation).await.unwrap_or(job);
        Ok(WalkResponse::job(
            job_phase(&completed),
            completed,
            "completed branch_live operation",
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
            .register_job(
                guard,
                b"reset".to_vec(),
                JobIntent {
                    command: WalkJobKind::Reset,
                    target_phase: None,
                    watch: None,
                    allow_live_api: None,
                    allow_git_changes: None,
                    llm_source: None,
                    allow_workspace_mutation: None,
                    allow_provenance_record: None,
                },
            )
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
            self.observed
                .update(ControllerObservation::controller(&controller));
            (phase, format!("reset walk to {phase} - {}", phase.detail()))
        };
        self.llm.lock().await.reset();
        let published = PublishedDelta::not_recorded(phase, job.expected.clone(), Some(job.job_id));
        finish_job(
            &self.jobs,
            &self.operation_root,
            &self.epoch,
            job.job_id,
            WalkJobStatus::Succeeded,
            Some(phase),
            message,
            None,
        )
        .await;
        let completed = self.operation_job(operation).await.unwrap_or(job);
        if completed.status == WalkJobStatus::Succeeded {
            replace_delta(&self.delta, published).await;
        }
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
                WalkOkKind::RecoverInspect,
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
            .register_job(
                guard,
                fingerprint,
                JobIntent {
                    command: WalkJobKind::Recover,
                    target_phase: None,
                    watch: None,
                    allow_live_api: None,
                    allow_git_changes: None,
                    llm_source: None,
                    allow_workspace_mutation: None,
                    allow_provenance_record: None,
                },
            )
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
                        Ok(()) => {
                            let phase = controller.phase();
                            self.observed
                                .update(ControllerObservation::controller(&controller));
                            phase
                        }
                        Err(source) => {
                            self.mark_stopping().await;
                            let detail = format!(
                                "recovery resolution committed but the server could not refresh its controller; restart the walk server before further mutation: {source}"
                            );
                            finish_job(
                                &self.jobs,
                                &self.operation_root,
                                &self.epoch,
                                job.job_id,
                                WalkJobStatus::Indeterminate,
                                Some(job.phase_before),
                                detail.clone(),
                                None,
                            )
                            .await;
                            return Err(PrepareError::InvalidBatchSelection { detail });
                        }
                    }
                };
                finish_job(
                    &self.jobs,
                    &self.operation_root,
                    &self.epoch,
                    job.job_id,
                    WalkJobStatus::Succeeded,
                    Some(phase),
                    message,
                    None,
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
                let (status, message) =
                    classify_job_error(&self.epoch.repo_root, &job.expected, &error);
                finish_job(
                    &self.jobs,
                    &self.operation_root,
                    &self.epoch,
                    job.job_id,
                    status,
                    Some(job.phase_before),
                    message.clone(),
                    None,
                )
                .await;
                Err(error)
            }
        }
    }

    async fn submit_resolve_job(
        &self,
        client_epoch: Option<&ServerEpoch>,
        guard: MutationGuard,
        resolution: WalkJobResolutionKind,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        let actual = self.durable_version()?;
        if guard.expected != actual {
            return Ok(WalkResponse::conflict(
                WalkErrorCode::StaleVersion,
                format!(
                    "job resolution for operation {} expected controller session {:?}, but durable state is {:?}",
                    guard.operation, guard.expected, actual
                ),
                actual,
                self.epoch.clone(),
            ));
        }

        let mut jobs = self.jobs.lock().await;
        if jobs.stopping {
            return Ok(WalkResponse::conflict(
                WalkErrorCode::ServerStopping,
                "walk job resolution refused because the server is stopping",
                actual,
                self.epoch.clone(),
            ));
        }
        if let Some(stored) = jobs.completed.get(&guard.operation)
            && stored.snapshot.status == WalkJobStatus::Abandoned
            && stored
                .snapshot
                .resolution
                .as_ref()
                .map(|receipt| receipt.kind)
                == Some(resolution)
        {
            return Ok(WalkResponse::job(
                job_phase(&stored.snapshot),
                stored.snapshot.clone(),
                "job was already durably abandoned; no duplicate resolution was written",
                self.epoch.clone(),
            ));
        }
        let Some(active) = jobs.active.as_mut() else {
            return Ok(WalkResponse::conflict(
                WalkErrorCode::OperationConflict,
                format!(
                    "operation {} is not the server's indeterminate job",
                    guard.operation
                ),
                actual,
                self.epoch.clone(),
            ));
        };
        if active.snapshot.operation_id != guard.operation {
            return Ok(WalkResponse::conflict(
                WalkErrorCode::RecoveryInProgress,
                format!(
                    "operation {} cannot be resolved while indeterminate operation {} remains authoritative",
                    guard.operation, active.snapshot.operation_id
                ),
                actual,
                self.epoch.clone(),
            ));
        }
        if active.snapshot.status == WalkJobStatus::Abandoned
            && active
                .snapshot
                .resolution
                .as_ref()
                .map(|receipt| receipt.kind)
                == Some(resolution)
        {
            return Ok(WalkResponse::job(
                job_phase(&active.snapshot),
                active.snapshot.clone(),
                "job was already durably abandoned; no duplicate resolution was written",
                self.epoch.clone(),
            ));
        }
        if active.snapshot.status != WalkJobStatus::Indeterminate {
            let code = if active.snapshot.status.is_active() {
                WalkErrorCode::JobActive
            } else {
                WalkErrorCode::OperationConflict
            };
            return Ok(WalkResponse::conflict(
                code,
                format!(
                    "operation {} is {:?}, not indeterminate; only an indeterminate job can be abandoned",
                    guard.operation, active.snapshot.status
                ),
                actual,
                self.epoch.clone(),
            ));
        }

        let resolved_at = now_rfc3339();
        let mut terminal = active.snapshot.clone();
        terminal.status = WalkJobStatus::Abandoned;
        terminal.updated_at = resolved_at.clone();
        terminal.finished_at = Some(resolved_at.clone());
        terminal.message = Some(format!(
            "operator abandoned indeterminate operation {}; prior effects remain possible and the durable evidence must be preserved",
            guard.operation
        ));
        terminal.resolution = Some(WalkJobResolutionReceipt {
            kind: resolution,
            observed: actual,
            resolved_at,
        });
        let stored = StoredOperation {
            snapshot: terminal.clone(),
            fingerprint: active.fingerprint.clone(),
        };
        if let Err(error) = persist_terminal_operation(&self.operation_root, &self.epoch, &stored) {
            if let Some(winner) = self.load_operation(guard.operation)?
                && !winner.stored.snapshot.status.blocks_mutation()
            {
                active.snapshot = winner.stored.snapshot.clone();
                active.fingerprint = winner.stored.fingerprint;
                active.handle = None;
                return Ok(WalkResponse::job(
                    job_phase(&active.snapshot),
                    active.snapshot.clone(),
                    format!(
                        "job resolution did not overwrite the terminal outcome already published by another server: {error}"
                    ),
                    self.epoch.clone(),
                ));
            }
            return Err(error);
        }
        active.snapshot = terminal.clone();
        active.handle = None;
        Ok(WalkResponse::job(
            job_phase(&terminal),
            terminal,
            "durably abandoned indeterminate job; later mutations remain responsible for inspecting its preserved evidence",
            self.epoch.clone(),
        ))
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
        decode_operation_record(&path, operation, &self.epoch, &bytes).map(Some)
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
        guard: MutationGuard,
        fingerprint: Vec<u8>,
        intent: JobIntent,
    ) -> Result<JobAdmission, PrepareError> {
        let command = intent.command;
        let mut jobs = self.jobs.lock().await;
        if jobs
            .completed
            .get(&guard.operation)
            .is_some_and(|stored| stored.snapshot.status.blocks_mutation())
            && let Some(record) = self.load_operation(guard.operation)?
            && !record.stored.snapshot.status.blocks_mutation()
        {
            jobs.completed.insert(guard.operation, record.stored);
        }
        let restored = jobs.active.as_ref().is_some_and(|active| {
            active.snapshot.operation_id == guard.operation && active.restored
        });
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
                if restored {
                    let actual = self.durable_version()?;
                    return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                        WalkErrorCode::OperationRestart,
                        format!(
                            "operation {} was admitted by an earlier walk-server incarnation; inspect the restored indeterminate job before resolving it",
                            guard.operation
                        ),
                        actual,
                        self.epoch.clone(),
                    )));
                }
                return Ok(JobAdmission::Duplicate(existing.snapshot));
            }
            let actual = self.durable_version()?;
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::OperationConflict,
                format!(
                    "operation {} was already used for a different {command} payload",
                    guard.operation
                ),
                actual,
                self.epoch.clone(),
            )));
        }

        if let Some(record) = self.load_operation(guard.operation)? {
            let same = record.stored.fingerprint == fingerprint
                && record.stored.snapshot.command == command;
            if same && !record.stored.snapshot.status.blocks_mutation() {
                return Ok(JobAdmission::Duplicate(record.stored.snapshot));
            }
            let actual = self.durable_version()?;
            let code = if same {
                WalkErrorCode::OperationRestart
            } else {
                WalkErrorCode::OperationConflict
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

        let durable = durable_state_for(&self.epoch.repo_root)?;
        let actual = durable.version.clone();
        if jobs.stopping {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::ServerStopping,
                format!("walk {command} refused because the server is stopping"),
                actual,
                self.epoch.clone(),
            )));
        }
        if let Some(active) = jobs
            .active
            .as_ref()
            .filter(|active| active.snapshot.status.blocks_mutation())
        {
            let code = if active.snapshot.status == WalkJobStatus::Indeterminate {
                WalkErrorCode::RecoveryInProgress
            } else {
                WalkErrorCode::JobActive
            };
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                code,
                format!(
                    "walk {command} refused because job {} ({}) is still {:?}",
                    active.snapshot.job_id, active.snapshot.command, active.snapshot.status
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        if let Some(blocker) = admission_blocker(command, &durable) {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::RecoveryInProgress,
                format!(
                    "walk {command} refused because durable controller recovery is required: {}",
                    blocker.detail
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        if command != WalkJobKind::Recover
            && let Some(detail) = self.controller.lock().await.blocker_detail()
        {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::RecoveryInProgress,
                format!("walk {command} refused because {detail}"),
                actual,
                self.epoch.clone(),
            )));
        }
        if guard.expected != actual {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::StaleVersion,
                format!(
                    "walk {command} operation {} expected controller session {:?}, but durable state is {:?}",
                    guard.operation, guard.expected, actual
                ),
                actual,
                self.epoch.clone(),
            )));
        }
        if command == WalkJobKind::Recover && actual.cursor().is_none() {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::BadRequest,
                "walk recover refused because durable controller recovery requires a committed session cursor",
                actual,
                self.epoch.clone(),
            )));
        }
        if command == WalkJobKind::Start
            && actual == SessionVersion::empty()
            && let Err(error) = validate_fresh_session(&self.epoch.repo_root)
        {
            return Ok(JobAdmission::Rejected(WalkResponse::conflict(
                WalkErrorCode::RecoveryInProgress,
                format!("walk start refused because fresh-session admission failed: {error}"),
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
            command,
            status: WalkJobStatus::Running,
            phase_before,
            phase_after: None,
            target_phase: intent.target_phase,
            watch: intent.watch,
            allow_live_api: intent.allow_live_api,
            allow_git_changes: intent.allow_git_changes,
            llm_source: intent.llm_source,
            allow_workspace_mutation: intent.allow_workspace_mutation,
            allow_provenance_record: intent.allow_provenance_record,
            started_at: now.clone(),
            updated_at: now,
            finished_at: None,
            message: None,
            receipt: None,
            resolution: None,
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
                    WalkErrorCode::OperationRestart
                } else {
                    WalkErrorCode::OperationConflict
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
            restored: false,
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
        let operation_root = self.operation_root.clone();
        let epoch = self.epoch.clone();
        let monitor = tokio::spawn(async move {
            let outcome = handle.await;
            finish_unsettled_job(&jobs, &operation_root, &epoch, job_id, outcome).await;
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
            .filter(|active| active.snapshot.status.blocks_mutation())
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

    async fn lookup_operation(
        &self,
        operation: OperationId,
    ) -> Result<Option<WalkJobSnapshot>, PrepareError> {
        let durable = self.reconcile_operation(operation).await?;
        let memory = self.operation_job(operation).await;
        if durable
            .as_ref()
            .is_some_and(|job| !job.status.blocks_mutation())
        {
            return Ok(durable);
        }
        Ok(memory.or(durable))
    }

    async fn reconcile_operation(
        &self,
        operation: OperationId,
    ) -> Result<Option<WalkJobSnapshot>, PrepareError> {
        let Some(record) = self.load_operation(operation)? else {
            return Ok(None);
        };
        let snapshot = record.stored.snapshot.clone();
        if !snapshot.status.blocks_mutation() {
            let mut jobs = self.jobs.lock().await;
            if let Some(active) = jobs
                .active
                .as_mut()
                .filter(|active| active.snapshot.operation_id == operation)
                .filter(|active| active.snapshot.status.blocks_mutation())
            {
                active.snapshot = snapshot.clone();
                active.fingerprint = record.stored.fingerprint.clone();
                active.handle = None;
            }
            if let Some(completed) = jobs.completed.get_mut(&operation) {
                *completed = record.stored;
            }
        }
        Ok(Some(snapshot))
    }

    async fn reconcile_active_operation(&self) -> Result<(), PrepareError> {
        let operation = self
            .jobs
            .lock()
            .await
            .active
            .as_ref()
            .filter(|active| active.snapshot.status.blocks_mutation())
            .map(|active| active.snapshot.operation_id);
        if let Some(operation) = operation {
            self.reconcile_operation(operation).await?;
        }
        Ok(())
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
            return self
                .durable_version()
                .map(|version| version.phase())
                .unwrap_or_else(|_| job_phase(&job));
        }
        self.controller.lock().await.phase()
    }

    async fn status_response(&self, heading: &str) -> Result<WalkResponse, PrepareError> {
        self.reconcile_active_operation().await?;
        let durable = durable_state_for(&self.epoch.repo_root)?;
        let (job, stopping) = {
            let jobs = self.jobs.lock().await;
            (
                jobs.active.as_ref().map(|active| active.snapshot.clone()),
                jobs.stopping,
            )
        };
        let session_exists = durable.version.session_id().is_some();
        let controller = self.controller.try_lock().ok();
        let observation = controller.as_ref().map_or_else(
            || self.observed.read(),
            |controller| {
                let observation = ControllerObservation::capture(
                    controller,
                    &self.epoch.repo_root,
                    session_exists,
                );
                self.observed.update(observation.clone());
                observation
            },
        );
        let controller_summary = controller.as_ref().map(|controller| controller.describe());
        let controller_blocker = observation.blocker.clone().map(|detail| WalkBlocker {
            code: WalkBlockerCode::ControllerBlocked,
            detail,
        });
        let fresh_blocker = (!session_exists)
            .then(|| match &observation.fresh {
                FreshAdmission::Blocked(detail) => Some(WalkBlocker {
                    code: WalkBlockerCode::ControllerBlocked,
                    detail: detail.clone(),
                }),
                FreshAdmission::Unchecked => Some(WalkBlocker {
                    code: WalkBlockerCode::ControllerBlocked,
                    detail:
                        "fresh-session admission has not been checked; retry status before mutating"
                            .to_string(),
                }),
                FreshAdmission::Ready => None,
            })
            .flatten();
        let drift_blocker = (session_exists
            && observation.attached
            && observation.phase != durable.version.phase())
        .then(|| WalkBlocker {
            code: WalkBlockerCode::ControllerBlocked,
            detail: format!(
                "controller cache is at {}, but the durable session cursor is at {}; refresh or recover before mutating",
                observation.phase,
                durable.version.phase()
            ),
        });
        let controller_attached = observation.attached;
        let position = if session_exists && durable.version.cursor().is_some() {
            WalkPosition::Session {
                version: durable.version.clone(),
            }
        } else if session_exists {
            WalkPosition::Unpositioned {
                version: durable.version.clone(),
            }
        } else if job.as_ref().is_some_and(|job| job.status.blocks_mutation())
            || observation.phase == WalkPhase::Empty
        {
            WalkPosition::NoSession
        } else {
            WalkPosition::Reconstruction {
                phase: observation.phase,
            }
        };
        let blocker = if stopping {
            Some(WalkBlocker {
                code: WalkBlockerCode::ServerStopping,
                detail: "walk server is stopping and will not admit another mutation".to_string(),
            })
        } else if let Some(job) = job.as_ref().filter(|job| job.status.blocks_mutation()) {
            let (code, detail) = if job.status == WalkJobStatus::Indeterminate {
                (
                    WalkBlockerCode::JobIndeterminate,
                    format!(
                        "job {} ({}) has an indeterminate outcome; inspect its evidence and explicitly abandon the job before further mutation",
                        job.job_id, job.command
                    ),
                )
            } else {
                (
                    WalkBlockerCode::JobActive,
                    format!("job {} ({}) is {:?}", job.job_id, job.command, job.status),
                )
            };
            Some(WalkBlocker { code, detail })
        } else if !self.gate.allows_mutation() {
            Some(WalkBlocker {
                code: WalkBlockerCode::TransferPending,
                detail: "successor endpoint is online, but predecessor controller release is not durable"
                    .to_string(),
            })
        } else if durable.blocker.is_some() {
            durable.blocker.clone()
        } else if controller_blocker.is_some() {
            controller_blocker
        } else if fresh_blocker.is_some() {
            fresh_blocker
        } else if drift_blocker.is_some() {
            drift_blocker
        } else {
            None
        };
        let authority = authority_for(blocker.as_ref());
        let actions = actions_for(&position, controller_attached, authority, blocker.as_ref());
        let snapshot = WalkSessionSnapshot {
            position,
            controller_attached,
            authority,
            job: job.clone(),
            actions,
            blocker,
        };
        let message = self.render_status_message(heading, job.as_ref(), controller_summary);
        Ok(WalkResponse::status(snapshot, message, self.epoch.clone()))
    }

    async fn query_response(
        &self,
        campaign: Option<CampaignId>,
        script: String,
    ) -> Result<WalkResponse, PrepareError> {
        let campaign = match campaign {
            Some(campaign) => campaign,
            None => identity::load_parent_identity_optional(&self.epoch.repo_root)?
                .map(|identity| identity.campaign_id().clone())
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "cannot infer campaign id for db_query; provide a campaign id or select a parent checkout containing '{}'",
                        identity::parent_identity_relpath().display()
                    ),
                })?,
        };
        let repo_root = self.epoch.repo_root.clone();
        let result =
            tokio::task::spawn_blocking(move || run_snapshot_query(repo_root, campaign, script))
                .await
                .map_err(|source| PrepareError::DatabaseSetup {
                    phase: "prototype1_state_walk_db_query_task",
                    detail: format!("immutable database query task failed: {source}"),
                })??;
        let version = self.durable_version()?;
        let phase = self
            .latest_job()
            .await
            .filter(|job| job.status.blocks_mutation())
            .as_ref()
            .map_or_else(|| version.phase(), job_phase);
        Ok(WalkResponse::query(
            phase,
            result,
            version,
            self.epoch.clone(),
        ))
    }

    async fn show_response(&self) -> Result<WalkResponse, PrepareError> {
        if self
            .latest_job()
            .await
            .is_some_and(|job| job.status.blocks_mutation())
        {
            return self.status_response("walk state").await;
        }
        let mut controller = self.controller.lock().await;
        controller.refresh_from_disk()?;
        let session_exists = self.durable_version()?.session_id().is_some();
        self.observed.update(ControllerObservation::capture(
            &controller,
            &self.epoch.repo_root,
            session_exists,
        ));
        Ok(WalkResponse::ok(
            WalkOkKind::Show,
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
                .filter(|active| active.snapshot.status.blocks_mutation())
            {
                let code = if active.snapshot.status.is_active() {
                    WalkErrorCode::JobActive
                } else {
                    WalkErrorCode::RecoveryInProgress
                };
                let version = self.durable_version()?;
                return Ok(WalkResponse::conflict(
                    code,
                    format!(
                        "walk stop refused while job {} ({}) is {:?}; wait for the job to finish, or inspect and explicitly recover/abandon any unresolved durable attempt before stopping the server",
                        active.snapshot.job_id, active.snapshot.command, active.snapshot.status
                    ),
                    version,
                    self.epoch.clone(),
                ));
            }
            jobs.stopping = true;
        }

        self.status_response("walk server stopping").await
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

    /// Inspect the durable session journal directly. This never takes the
    /// controller mutex and therefore remains available while a live job owns
    /// the in-memory typestate value.
    fn durable_version(&self) -> Result<SessionVersion, PrepareError> {
        durable_version_for(&self.epoch.repo_root)
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
                | WalkRequestBody::ResolveJob { .. }
        ),
    }
}

fn authority_for(blocker: Option<&WalkBlocker>) -> WalkAuthority {
    match blocker.map(|blocker| blocker.code) {
        None => WalkAuthority::Active,
        Some(WalkBlockerCode::TransferPending) => WalkAuthority::TransferPending,
        Some(WalkBlockerCode::JobActive) => WalkAuthority::JobActive,
        Some(WalkBlockerCode::SessionAbandoned) => WalkAuthority::Abandoned,
        Some(WalkBlockerCode::ServerStopping) => WalkAuthority::Stopping,
        Some(
            WalkBlockerCode::JournalDamaged
            | WalkBlockerCode::JobIndeterminate
            | WalkBlockerCode::AttemptPending
            | WalkBlockerCode::AttemptIndeterminate
            | WalkBlockerCode::ControllerBlocked,
        ) => WalkAuthority::RecoveryRequired,
    }
}

fn actions_for(
    position: &WalkPosition,
    controller_attached: bool,
    authority: WalkAuthority,
    blocker: Option<&WalkBlocker>,
) -> Vec<WalkAction> {
    let phase = position.phase();
    let mut actions = vec![
        WalkAction {
            kind: WalkActionKind::Inspect,
            edge: None,
            target: None,
            enabled: true,
            requires_live_api: false,
            requires_git_changes: false,
            blocker: None,
        },
        WalkAction {
            kind: WalkActionKind::Query,
            edge: None,
            target: None,
            enabled: true,
            requires_live_api: false,
            requires_git_changes: false,
            blocker: None,
        },
    ];
    let mutation_enabled = authority == WalkAuthority::Active;
    let blocker_code = (!mutation_enabled)
        .then(|| blocker.map(|blocker| blocker.code))
        .flatten();
    match position {
        WalkPosition::NoSession | WalkPosition::Reconstruction { .. } => {
            actions.push(WalkAction {
                kind: WalkActionKind::Start,
                edge: None,
                target: Some(WalkPhase::R3),
                enabled: mutation_enabled,
                requires_live_api: false,
                requires_git_changes: false,
                blocker: blocker_code,
            });
        }
        WalkPosition::Session { .. } if !controller_attached => {
            actions.push(WalkAction {
                kind: WalkActionKind::Start,
                edge: None,
                target: Some(phase),
                enabled: mutation_enabled,
                requires_live_api: false,
                requires_git_changes: false,
                blocker: blocker_code,
            });
        }
        WalkPosition::Session { .. } => {
            actions.extend(
                ControlEdge::ALL
                    .into_iter()
                    .filter(|edge| edge.from() == phase)
                    .map(|edge| {
                        edge_action(WalkActionKind::Step, edge, mutation_enabled, blocker_code)
                    }),
            );
            actions.push(WalkAction {
                kind: WalkActionKind::Reset,
                edge: None,
                target: Some(WalkPhase::Empty),
                enabled: mutation_enabled,
                requires_live_api: false,
                requires_git_changes: false,
                blocker: blocker_code,
            });
        }
        WalkPosition::Unpositioned { .. } | WalkPosition::Legacy { .. } => (),
    }
    if authority == WalkAuthority::RecoveryRequired
        && matches!(position, WalkPosition::Session { .. })
        && blocker.map(|blocker| blocker.code) != Some(WalkBlockerCode::JobIndeterminate)
    {
        actions.push(WalkAction {
            kind: WalkActionKind::Recover,
            edge: None,
            target: Some(phase),
            enabled: true,
            requires_live_api: false,
            requires_git_changes: false,
            blocker: None,
        });
    }
    let stop_enabled = !matches!(
        authority,
        WalkAuthority::JobActive | WalkAuthority::Stopping
    ) && blocker.map(|blocker| blocker.code)
        != Some(WalkBlockerCode::JobIndeterminate);
    actions.push(WalkAction {
        kind: WalkActionKind::Stop,
        edge: None,
        target: None,
        enabled: stop_enabled,
        requires_live_api: false,
        requires_git_changes: false,
        blocker: if stop_enabled {
            None
        } else {
            blocker.map(|blocker| blocker.code)
        },
    });
    actions
}

fn edge_action(
    kind: WalkActionKind,
    edge: ControlEdge,
    enabled: bool,
    blocker: Option<WalkBlockerCode>,
) -> WalkAction {
    WalkAction {
        kind,
        edge: Some(edge),
        target: Some(edge.to()),
        enabled,
        requires_live_api: edge.requires_live(),
        requires_git_changes: edge.requires_checkout(),
        blocker,
    }
}

fn request_error_code(error: &PrepareError) -> WalkErrorCode {
    match error {
        PrepareError::RecoveryInProgress { .. } => WalkErrorCode::RecoveryInProgress,
        _ => WalkErrorCode::RequestFailed,
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

struct DurableSessionState {
    version: SessionVersion,
    blocker: Option<WalkBlocker>,
}

fn durable_version_for(repo_root: &Path) -> Result<SessionVersion, PrepareError> {
    durable_state_for(repo_root).map(|state| state.version)
}

fn durable_history_for(
    repo_root: &Path,
    epoch: ServerEpoch,
) -> Result<WalkSessionHistory, PrepareError> {
    let Some(parent) = identity::load_parent_identity_optional(repo_root)? else {
        return Ok(WalkSessionHistory::empty(None, epoch));
    };
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let store = Store::for_manifest(&manifest);
    let journal_path = store.paths(&parent).journal().to_path_buf();
    let history = store
        .inspect_history(&parent, epoch.clone())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_session_history",
            detail: source.to_string(),
        })?;
    Ok(match history {
        Some(history) => history,
        None => WalkSessionHistory::empty(Some(journal_path), epoch),
    })
}

fn durable_state_for(repo_root: &Path) -> Result<DurableSessionState, PrepareError> {
    let Some(parent) = identity::load_parent_identity_optional(repo_root)? else {
        return Ok(DurableSessionState {
            version: SessionVersion::empty(),
            blocker: None,
        });
    };
    let manifest = campaign_manifest_path(parent.campaign_id())?;
    let snapshot = Store::for_manifest(&manifest)
        .inspect(&parent)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_session_snapshot",
            detail: source.to_string(),
        })?;
    let Some(snapshot) = snapshot else {
        return Ok(DurableSessionState {
            version: SessionVersion::empty(),
            blocker: None,
        });
    };
    let version = SessionVersion {
        session_id: snapshot
            .created
            .as_ref()
            .map(|created| created.session_id()),
        cursor: snapshot.cursor,
        journal_revision: snapshot.journal_revision,
    };
    let blocker = if let Some(damage) = snapshot.damage {
        Some(WalkBlocker {
            code: WalkBlockerCode::JournalDamaged,
            detail: format!("controller journal is damaged: {}", damage_detail(&damage)),
        })
    } else if let Some(detail) = snapshot.abandoned {
        Some(WalkBlocker {
            code: WalkBlockerCode::SessionAbandoned,
            detail,
        })
    } else {
        snapshot
            .attempts
            .iter()
            .rev()
            .find_map(|attempt| match attempt {
                Attempt::Pending { intent, .. } => Some(WalkBlocker {
                    code: WalkBlockerCode::AttemptPending,
                    detail: format!(
                        "transition {} from {} has no terminal receipt",
                        intent.transition_id, intent.expected
                    ),
                }),
                Attempt::Finished(receipt)
                    if matches!(receipt.result, AttemptResult::Indeterminate { .. }) =>
                {
                    Some(WalkBlocker {
                        code: WalkBlockerCode::AttemptIndeterminate,
                        detail: format!(
                            "transition {} reached an indeterminate effect boundary",
                            receipt.intent.transition_id
                        ),
                    })
                }
                Attempt::Finished(_) | Attempt::Recovered { .. } => None,
            })
    };
    Ok(DurableSessionState { version, blocker })
}

fn admission_blocker(command: WalkJobKind, durable: &DurableSessionState) -> Option<&WalkBlocker> {
    (command != WalkJobKind::Recover)
        .then(|| durable.blocker.as_ref())
        .flatten()
}

fn damage_detail(damage: &Damage) -> String {
    match damage {
        Damage::Truncated { line, .. } => format!("truncated record at line {line}"),
        Damage::Malformed { line, detail } => {
            format!("malformed record at line {line}: {detail}")
        }
        Damage::Sequence { line, detail } => {
            format!("invalid record sequence at line {line}: {detail}")
        }
    }
}

async fn run_start_job(
    controller: Arc<Mutex<WalkController>>,
    llm: Arc<Mutex<LlmInspector>>,
    delta: Arc<RwLock<PublishedDelta>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    operation_root: PathBuf,
    job_id: u64,
    expected: SessionVersion,
    config: WalkStartConfig,
    until: WalkPhase,
    allow_live_api: bool,
    observed: Arc<ControllerCache>,
) {
    let (result, observation) = {
        let mut controller = controller.lock().await;
        let result = match controller
            .start_version(config, until, allow_live_api, &expected)
            .await
        {
            Ok(report) => {
                let phase = controller.phase();
                match report
                    .transition_edges()
                    .and_then(|edges| report.exact_version().map(|version| (edges, version)))
                {
                    Ok((edges, version)) => {
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
                        let message =
                            format!("started walk at {} - {}", report.to(), report.to().detail());
                        PublishedDelta::capture(&controller, version.clone(), Some(job_id))
                            .map(|published| {
                                (
                                    phase,
                                    event,
                                    message,
                                    report.from(),
                                    report.to(),
                                    edges,
                                    version,
                                    published,
                                )
                            })
                            .map_err(|error| (phase, TransitionFailure::Receipt(error)))
                    }
                    Err(error) => Err((phase, TransitionFailure::Receipt(error))),
                }
            }
            Err(error) => Err((controller.phase(), TransitionFailure::Attempt(error))),
        };
        (result, ControllerObservation::controller(&controller))
    };
    if result.is_ok() {
        llm.lock().await.reset();
    }
    observed.update(observation);
    match result {
        Ok((phase, event, message, phase_before, phase_after, edges, version, published)) => {
            let event_projection = match record_walk_event(&epoch, event) {
                Ok(projection) => projection,
                Err(error) => WalkEventProjection::Failed {
                    detail: error.to_string(),
                },
            };
            let receipt = WalkTransitionReceipt {
                phase_before,
                phase_after,
                edges,
                version,
                event_projection: event_projection.clone(),
            };
            let message = match event_projection {
                WalkEventProjection::Failed { ref detail } => format!(
                    "{message}\nprojection_warning=transition committed, but owner-DB walk-event projection failed: {detail}"
                ),
                _ => message,
            };
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                WalkJobStatus::Succeeded,
                Some(phase),
                message,
                Some(receipt),
            )
            .await;
            publish_delta(&delta, &jobs, job_id, published).await;
        }
        Err((phase, TransitionFailure::Attempt(error))) => {
            let (status, message) = classify_job_error(&epoch.repo_root, &expected, &error);
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                status,
                Some(phase),
                message,
                None,
            )
            .await
        }
        Err((phase, TransitionFailure::Receipt(error))) => {
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                WalkJobStatus::Indeterminate,
                Some(phase),
                format!(
                    "walk transition returned success, but its typed edge receipt could not be reconstructed: {error}; inspect the committed journal before abandoning this job"
                ),
                None,
            )
            .await
        }
    }
}

async fn run_step_job(
    controller: Arc<Mutex<WalkController>>,
    delta: Arc<RwLock<PublishedDelta>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    operation_root: PathBuf,
    job_id: u64,
    expected: SessionVersion,
    until: Option<WalkPhase>,
    client_watch: bool,
    allow_live_api: bool,
    allow_git_changes: bool,
    observed: Arc<ControllerCache>,
) {
    let (result, observation) = {
        let mut controller = controller.lock().await;
        let result = match controller
            .step_version(until, allow_live_api, allow_git_changes, &expected)
            .await
        {
            Ok(report) => {
                let phase = controller.phase();
                match report
                    .transition_edges()
                    .and_then(|edges| report.exact_version().map(|version| (edges, version)))
                {
                    Ok((edges, version)) => {
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
                        PublishedDelta::capture(&controller, version.clone(), Some(job_id))
                            .map(|published| {
                                (
                                    phase,
                                    event,
                                    message,
                                    report.from(),
                                    report.to(),
                                    edges,
                                    version,
                                    published,
                                )
                            })
                            .map_err(|error| (phase, TransitionFailure::Receipt(error)))
                    }
                    Err(error) => Err((phase, TransitionFailure::Receipt(error))),
                }
            }
            Err(error) => Err((controller.phase(), TransitionFailure::Attempt(error))),
        };
        (result, ControllerObservation::controller(&controller))
    };
    observed.update(observation);
    match result {
        Ok((phase, event, message, phase_before, phase_after, edges, version, published)) => {
            let event_projection = match record_walk_event(&epoch, event) {
                Ok(projection) => projection,
                Err(error) => WalkEventProjection::Failed {
                    detail: error.to_string(),
                },
            };
            let receipt = WalkTransitionReceipt {
                phase_before,
                phase_after,
                edges,
                version,
                event_projection: event_projection.clone(),
            };
            let message = match event_projection {
                WalkEventProjection::Failed { ref detail } => format!(
                    "{message}\nprojection_warning=transition committed, but owner-DB walk-event projection failed: {detail}"
                ),
                _ => message,
            };
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                WalkJobStatus::Succeeded,
                Some(phase),
                message,
                Some(receipt),
            )
            .await;
            publish_delta(&delta, &jobs, job_id, published).await;
        }
        Err((phase, TransitionFailure::Attempt(error))) => {
            let (status, message) = classify_job_error(&epoch.repo_root, &expected, &error);
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                status,
                Some(phase),
                message,
                None,
            )
            .await
        }
        Err((phase, TransitionFailure::Receipt(error))) => {
            finish_job(
                &jobs,
                &operation_root,
                &epoch,
                job_id,
                WalkJobStatus::Indeterminate,
                Some(phase),
                format!(
                    "walk transition returned success, but its typed edge receipt could not be reconstructed: {error}; inspect the committed journal before abandoning this job"
                ),
                None,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_llm_step_job(
    llm: Arc<Mutex<LlmInspector>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    operation_root: PathBuf,
    job_id: u64,
    phase: WalkPhase,
    session_id: Option<String>,
    lane: Option<String>,
    step: Option<usize>,
    source: Prototype1StateWalkLlmStepSource,
    watch: bool,
    allow_workspace_mutation: bool,
    model_id: Option<String>,
    provider: Option<String>,
    max_attempts: u32,
    timeout_secs: u64,
) {
    let result = {
        let mut llm = llm.lock().await;
        llm.llm_step(
            session_id.as_deref(),
            lane.as_deref(),
            step,
            source,
            watch,
            allow_workspace_mutation,
            model_id.as_deref(),
            provider.as_deref(),
            max_attempts,
            timeout_secs,
        )
        .await
    };
    let (status, message) = match result {
        Ok(message) => (WalkJobStatus::Succeeded, message),
        Err(error) => (
            WalkJobStatus::Indeterminate,
            format!(
                "llm_step failed after supervised admission and may have performed provider or tool effects: {error}; inspect its persisted trace before abandoning this job"
            ),
        ),
    };
    finish_job(
        &jobs,
        &operation_root,
        &epoch,
        job_id,
        status,
        Some(phase),
        message,
        None,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn run_llm_finish_job(
    llm: Arc<Mutex<LlmInspector>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    operation_root: PathBuf,
    job_id: u64,
    phase: WalkPhase,
    session_id: Option<String>,
    lane: Option<String>,
    step: Option<usize>,
    watch: bool,
    allow_workspace_mutation: bool,
    model_id: Option<String>,
    provider: Option<String>,
    max_steps: usize,
    max_attempts: u32,
    timeout_secs: u64,
) {
    let result = {
        let mut llm = llm.lock().await;
        llm.llm_finish(
            session_id.as_deref(),
            lane.as_deref(),
            step,
            watch,
            allow_workspace_mutation,
            model_id.as_deref(),
            provider.as_deref(),
            max_steps,
            max_attempts,
            timeout_secs,
        )
        .await
    };
    let (status, message) = match result {
        Ok(message) => (WalkJobStatus::Succeeded, message),
        Err(error) => (
            WalkJobStatus::Indeterminate,
            format!(
                "llm_finish failed after supervised admission and may have performed provider or tool effects: {error}; inspect its persisted trace before abandoning this job"
            ),
        ),
    };
    finish_job(
        &jobs,
        &operation_root,
        &epoch,
        job_id,
        status,
        Some(phase),
        message,
        None,
    )
    .await;
}

fn classify_job_error(
    repo_root: &Path,
    expected: &SessionVersion,
    error: &PrepareError,
) -> (WalkJobStatus, String) {
    let detail = error.to_string();
    match durable_state_for(repo_root) {
        Ok(state)
            if state.blocker.as_ref().is_some_and(|blocker| {
                matches!(
                    blocker.code,
                    WalkBlockerCode::JournalDamaged
                        | WalkBlockerCode::AttemptPending
                        | WalkBlockerCode::AttemptIndeterminate
                )
            }) =>
        {
            (
                WalkJobStatus::Indeterminate,
                format!(
                    "walk transition failed at an unresolved durable effect boundary: {detail}; inspect and resolve the controller journal before abandoning this job"
                ),
            )
        }
        Ok(state) if state.version.cursor() != expected.cursor() => (
            WalkJobStatus::Indeterminate,
            format!(
                "walk transition returned an error ({detail}), but the durable controller cursor changed from the admitted version; preserve and inspect the committed evidence before abandoning this job"
            ),
        ),
        Ok(_) => (WalkJobStatus::Failed, detail),
        Err(source) => (
            WalkJobStatus::Indeterminate,
            format!(
                "walk transition failed ({detail}), and durable state could not be inspected to prove that no effects occurred: {source}; preserve the evidence before abandoning this job"
            ),
        ),
    }
}

async fn finish_job(
    jobs: &Mutex<JobRegistry>,
    operation_root: &Path,
    epoch: &ServerEpoch,
    job_id: u64,
    status: WalkJobStatus,
    phase_after: Option<WalkPhase>,
    message: String,
    receipt: Option<WalkTransitionReceipt>,
) {
    let mut jobs = jobs.lock().await;
    let Some(active) = jobs
        .active
        .as_mut()
        .filter(|active| active.snapshot.job_id == job_id)
    else {
        return;
    };
    if !active.snapshot.status.is_active() {
        return;
    }
    if active.snapshot.status == WalkJobStatus::CancelRequested {
        return;
    }
    let mut terminal = active.snapshot.clone();
    terminal.status = status;
    terminal.phase_after = phase_after;
    terminal.updated_at = now_rfc3339();
    terminal.finished_at = Some(now_rfc3339());
    terminal.message = Some(message);
    terminal.receipt = receipt;
    let stored = StoredOperation {
        snapshot: terminal.clone(),
        fingerprint: active.fingerprint.clone(),
    };
    if let Err(error) = persist_terminal_operation(operation_root, epoch, &stored) {
        let path = operation_root.join(format!("{}.json", terminal.operation_id));
        if let Ok(winner) = read_operation_record(&path, terminal.operation_id, epoch)
            && !winner.stored.snapshot.status.blocks_mutation()
        {
            let transferred = completed_handoff(&winner.stored.snapshot);
            active.snapshot = winner.stored.snapshot;
            active.fingerprint = winner.stored.fingerprint;
            active.handle = None;
            if transferred {
                jobs.stopping = true;
            }
            return;
        }
        terminal.status = WalkJobStatus::Indeterminate;
        terminal.receipt = None;
        terminal.resolution = None;
        terminal.message = Some(format!(
            "walk job reached intended terminal state {status:?}, but its durable operational receipt could not be published: {error}; effects may have occurred, so inspect durable state and explicitly abandon this job before further mutation"
        ));
    }
    let transferred = completed_handoff(&terminal);
    active.snapshot = terminal;
    active.handle = None;
    if transferred {
        // Terminal operation publication and predecessor admission fencing
        // share this mutex. Once the R12->R13b receipt is observable, no
        // stale client can win a final mutation before successor retirement.
        jobs.stopping = true;
    }
}

fn completed_handoff(snapshot: &WalkJobSnapshot) -> bool {
    snapshot.status == WalkJobStatus::Succeeded
        && snapshot.receipt.as_ref().is_some_and(|receipt| {
            receipt.edges.contains(&ControlEdge::R12ToR13b)
                && receipt.version.phase() == WalkPhase::R13b
        })
}

async fn publish_delta(
    delta: &RwLock<PublishedDelta>,
    jobs: &Mutex<JobRegistry>,
    job_id: u64,
    published: PublishedDelta,
) {
    let completed = {
        let jobs = jobs.lock().await;
        jobs.active
            .as_ref()
            .filter(|active| active.snapshot.job_id == job_id)
            .map(|active| &active.snapshot)
            .or_else(|| {
                jobs.completed
                    .values()
                    .map(|stored| &stored.snapshot)
                    .find(|snapshot| snapshot.job_id == job_id)
            })
            .is_some_and(|snapshot| {
                published.source_job == Some(job_id)
                    && snapshot.status == WalkJobStatus::Succeeded
                    && snapshot
                        .receipt
                        .as_ref()
                        .is_some_and(|receipt| receipt.version == published.snapshot.version)
            })
    };
    if completed {
        replace_delta(delta, published).await;
    }
}

async fn replace_delta(delta: &RwLock<PublishedDelta>, published: PublishedDelta) {
    let mut current = delta.write().await;
    if published.source_job >= current.source_job {
        *current = published;
    }
}

async fn finish_unsettled_job(
    jobs: &Mutex<JobRegistry>,
    operation_root: &Path,
    epoch: &ServerEpoch,
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
    finish_job(
        jobs,
        operation_root,
        epoch,
        job_id,
        WalkJobStatus::Indeterminate,
        None,
        message,
        None,
    )
    .await;
}

fn persist_terminal_operation(
    operation_root: &Path,
    epoch: &ServerEpoch,
    stored: &StoredOperation,
) -> Result<(), PrepareError> {
    let operation = stored.snapshot.operation_id;
    let path = operation_root.join(format!("{operation}.json"));
    let lock_path = operation_root.join(format!("{operation}.lock"));
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| operation_error("open lock", &lock_path, source))?;
    lock_operation(&lock, &lock_path)?;
    let bytes = fs::read(&path).map_err(|source| operation_error("read", &path, source))?;
    let admitted: DurableOperation =
        serde_json::from_slice(&bytes).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_operation_parse",
            detail: format!(
                "failed to parse admitted operation record '{}': {source}",
                path.display()
            ),
        })?;
    if admitted.schema_version != OPERATION_SCHEMA_VERSION
        || admitted.epoch.repo_root != epoch.repo_root
        || admitted.stored.snapshot.operation_id != stored.snapshot.operation_id
        || admitted.stored.snapshot.expected != stored.snapshot.expected
        || admitted.stored.snapshot.command != stored.snapshot.command
        || admitted.stored.fingerprint != stored.fingerprint
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "admitted operation record '{}' changed identity before terminal publication",
                path.display()
            ),
        });
    }
    if !terminal_transition_allowed(admitted.stored.snapshot.status, stored.snapshot.status) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "durable operation {} cannot transition from {:?} to {:?}; another process may already have published its terminal outcome",
                operation, admitted.stored.snapshot.status, stored.snapshot.status
            ),
        });
    }
    let record = DurableOperation {
        schema_version: OPERATION_SCHEMA_VERSION.to_string(),
        epoch: admitted.epoch,
        stored: stored.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(PrepareError::Serialize)?;
    durable_io::write_atomic(&path, &bytes)
        .map_err(|source| operation_error("publish terminal", &path, source))
}

fn terminal_transition_allowed(from: WalkJobStatus, to: WalkJobStatus) -> bool {
    match to {
        WalkJobStatus::Abandoned => matches!(
            from,
            WalkJobStatus::Running | WalkJobStatus::CancelRequested | WalkJobStatus::Indeterminate
        ),
        WalkJobStatus::Succeeded
        | WalkJobStatus::Failed
        | WalkJobStatus::Cancelled
        | WalkJobStatus::Indeterminate => {
            matches!(
                from,
                WalkJobStatus::Running | WalkJobStatus::CancelRequested
            )
        }
        WalkJobStatus::Running | WalkJobStatus::CancelRequested => false,
    }
}

#[cfg(unix)]
fn lock_operation(file: &fs::File, path: &Path) -> Result<(), PrepareError> {
    let result = unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(file), libc::LOCK_EX) };
    if result == 0 {
        return Ok(());
    }
    Err(operation_error(
        "lock",
        path,
        std::io::Error::last_os_error(),
    ))
}

#[cfg(not(unix))]
fn lock_operation(_file: &fs::File, path: &Path) -> Result<(), PrepareError> {
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "durable operation publication requires cross-process locking for '{}'",
            path.display()
        ),
    })
}

fn record_walk_event(
    epoch: &ServerEpoch,
    input: WalkEventInput,
) -> Result<WalkEventProjection, PrepareError> {
    let Some(identity) = identity::load_parent_identity_optional(&epoch.repo_root)? else {
        return Ok(WalkEventProjection::NotApplicable {
            detail: "checkout has no parent identity".to_string(),
        });
    };
    let db_path = owner_db_path(identity.campaign_id().as_str())?;
    if !db_path.is_file() {
        return Ok(WalkEventProjection::NotApplicable {
            detail: format!("owner database '{}' does not exist", db_path.display()),
        });
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
    })?;
    Ok(WalkEventProjection::Recorded)
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
    if let Some(resolution) = &job.resolution {
        lines.push(format!("job_resolution={:?}", resolution.kind));
        lines.push(format!(
            "job_resolution_revision={}",
            resolution.observed.journal_revision()
        ));
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
        WalkJobStatus::Indeterminate => "indeterminate",
        WalkJobStatus::Abandoned => "abandoned",
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
    use std::{ffi::OsString, io::Read, str::FromStr};

    #[cfg(target_os = "linux")]
    use std::{os::unix::process::CommandExt, process::Command};

    use flate2::read::GzDecoder;
    use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};
    use tempfile::tempdir;

    use crate::{
        cli::{
            Prototype1StateWalkAuditScope, Prototype1StateWalkLlmStepSource,
            prototype1_state::{
                channel::{Envelope, ToParent},
                driver::control::PredecessorRelease,
                event::{ContentHash, RecordedAt, RuntimeId},
                identity::{ParentIdentity, write_parent_identity},
                invocation::{self, InvocationAuthority},
                journal::{Streams, SuccessorHandoffEntry},
                profile::{RunMode, RunProfileCommitment},
                session::{Claim, Cursor, Outcome, SessionId, Store},
                walk::{
                    endpoint,
                    epoch::ServerEpoch,
                    protocol::{WalkOkPayload, WalkSessionEventKind},
                },
            },
        },
        replay::tool_loop::{FsToolLoopStore, ToolLoopSession, ToolLoopStore},
        test_support::env_guard_os,
    };

    use super::*;

    fn test_server(repo_root: &Path, gate: MutationGate) -> WalkServer {
        let epoch = ServerEpoch::capture(repo_root).expect("capture server epoch");
        let operation_root = repo_root.join("walk-operations");
        fs::create_dir_all(&operation_root).expect("create operation directory");
        let jobs = restore_job_registry(&operation_root, &epoch, JobRestoreScope::Repository, None)
            .expect("restore job registry");
        let controller = WalkController::new(repo_root.to_path_buf());
        let delta = PublishedDelta::capture(&controller, SessionVersion::empty(), None)
            .expect("capture initial delta");
        let observation = ControllerObservation::capture(&controller, repo_root, false);
        WalkServer {
            epoch,
            controller: Arc::new(Mutex::new(controller)),
            llm: Arc::new(Mutex::new(LlmInspector::new(repo_root.to_path_buf()))),
            delta: Arc::new(RwLock::new(delta)),
            jobs: Arc::new(Mutex::new(jobs)),
            gate,
            operation_root,
            observed: Arc::new(ControllerCache::new(observation)),
        }
    }

    fn test_guard(value: u128) -> MutationGuard {
        MutationGuard {
            operation: OperationId::for_test(value),
            expected: SessionVersion::empty(),
        }
    }

    fn test_version(phase: WalkPhase, value: u128) -> SessionVersion {
        let evidence = format!("test-session-{value}");
        SessionVersion {
            session_id: Some(SessionId::for_test(value)),
            cursor: Some(
                Cursor::new(phase, ContentHash::of(&evidence)).expect("valid test cursor"),
            ),
            journal_revision: 1,
        }
    }

    fn test_intent(command: WalkJobKind) -> JobIntent {
        JobIntent {
            command,
            target_phase: None,
            watch: None,
            allow_live_api: None,
            allow_git_changes: None,
            llm_source: None,
            allow_workspace_mutation: None,
            allow_provenance_record: None,
        }
    }

    fn test_step_intent(target: WalkPhase) -> JobIntent {
        JobIntent {
            target_phase: Some(target),
            watch: Some(false),
            ..test_intent(WalkJobKind::Step)
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

    fn decode_hex(path: &Path) -> Vec<u8> {
        let encoded = fs::read_to_string(path).expect("read hex-encoded historical fixture");
        let encoded: String = encoded
            .chars()
            .filter(|value| !value.is_whitespace())
            .collect();
        assert_eq!(encoded.len() % 2, 0, "historical fixture hex is complete");
        let bytes: Vec<u8> = (0..encoded.len())
            .step_by(2)
            .map(|offset| {
                u8::from_str_radix(&encoded[offset..offset + 2], 16)
                    .expect("decode historical fixture hex")
            })
            .collect();
        bytes
    }

    fn inflate_hex(path: &Path) -> Vec<u8> {
        let compressed = decode_hex(path);
        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .expect("inflate historical fixture");
        decoded
    }

    fn journal_parent(bytes: &[u8]) -> ParentIdentity {
        let first = bytes
            .split(|value| *value == b'\n')
            .next()
            .expect("historical journal has a Created entry");
        let value: serde_json::Value =
            serde_json::from_slice(first).expect("decode historical Created entry");
        serde_json::from_value(value["parent"].clone())
            .expect("historical Created entry carries a parent identity")
    }

    #[test]
    fn protocol_v5_operation_record_defaults_new_supervision_fields() {
        let repo = tempdir().expect("repo tempdir");
        let epoch = ServerEpoch::capture(repo.path()).expect("capture epoch");
        let operation = OperationId::for_test(9_001);
        let value = serde_json::json!({
            "schema_version": "prototype1-walk-operation.v1",
            "epoch": epoch,
            "stored": {
                "snapshot": {
                    "job_id": 7,
                    "operation_id": operation,
                    "expected": {
                        "session_id": null,
                        "cursor": null,
                        "journal_revision": 0
                    },
                    "command": "step",
                    "status": "succeeded",
                    "phase_before": "r5",
                    "phase_after": "r6",
                    "target_phase": "r6",
                    "watch": true,
                    "allow_live_api": true,
                    "allow_git_changes": false,
                    "started_at": "2026-07-12T00:00:00Z",
                    "updated_at": "2026-07-12T00:01:00Z",
                    "finished_at": "2026-07-12T00:01:00Z",
                    "message": "legacy terminal operation"
                },
                "fingerprint": [1, 2, 3]
            }
        });

        let record: DurableOperation =
            serde_json::from_value(value).expect("protocol-v5 durable operation");
        assert_eq!(record.stored.snapshot.command, WalkJobKind::Step);
        assert_eq!(record.stored.snapshot.status, WalkJobStatus::Succeeded);
        assert!(record.stored.snapshot.llm_source.is_none());
        assert!(record.stored.snapshot.receipt.is_none());
        assert!(record.stored.snapshot.resolution.is_none());
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
                    guard: test_guard(5),
                    reason: "test transfer gate".to_string(),
                    allow_provenance_record: true,
                },
            ),
            (
                "llm_step",
                WalkRequestBody::LlmStep {
                    guard: test_guard(6),
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
                    guard: test_guard(7),
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
            client_protocol: None,
            client_epoch: None,
            body,
        }
    }

    async fn health_over_socket(socket: &Path) -> Result<WalkResponse, PrepareError> {
        let mut stream = ipc::connect(socket).await?;
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: None,
                body: WalkRequestBody::Health,
            },
        )
        .await?;
        ipc::recv(&mut stream).await
    }

    async fn request_over_socket(
        socket: &Path,
        body: WalkRequestBody,
    ) -> Result<WalkResponse, PrepareError> {
        let mut stream = ipc::connect(socket).await?;
        ipc::send(&mut stream, &walk_request(body)).await?;
        ipc::recv(&mut stream).await
    }

    #[tokio::test]
    async fn closed_gate_admits_reads_and_blocks_transfer_requests() {
        let repo = tempdir().expect("repo tempdir");
        let gate = MutationGate::closed();
        let server = test_server(repo.path(), gate.clone());

        for body in [
            WalkRequestBody::Health,
            WalkRequestBody::Show,
            WalkRequestBody::SessionHistory,
            WalkRequestBody::EvaluationTraceIndex,
        ] {
            let (response, stop) = server.handle(walk_request(body)).await;
            assert!(!stop, "read-only request must not stop the server");
            assert_eq!(
                response.phase(),
                Some(WalkPhase::Empty),
                "closed gate rejected a read-only request: {response:?}"
            );
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
                    assert_eq!(
                        code,
                        WalkErrorCode::TransferPending,
                        "wrong rejection for {kind}"
                    );
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
            client_protocol: None,
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
                client_protocol: None,
                client_epoch: Some(pending.epoch.clone()),
                body: WalkRequestBody::Stop,
            })
            .await;
        assert!(stop, "closed pending endpoint must permit local shutdown");
        match response {
            WalkResponse::Status { snapshot, .. } => {
                assert!(snapshot.job.is_none(), "shutdown must not invent a job");
                assert_eq!(snapshot.authority, WalkAuthority::Stopping);
            }
            other => panic!("closed endpoint did not return local shutdown status: {other:?}"),
        }
    }

    #[tokio::test]
    async fn prepared_service_answers_health_before_transfer() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("successor.sock");
        let listener = UnixListener::bind(&socket).expect("bind successor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture successor endpoint");
        let prepared = PreparedServer {
            listener,
            endpoint: endpoint.clone(),
            epoch: ServerEpoch::capture(repo.path()).expect("capture successor epoch"),
            idle_ttl: None,
            publish: false,
            restore: JobRestoreScope::Repository,
        };
        let serving = tokio::spawn(serve_prepared(prepared, MutationGate::closed()));

        await_responsive(&endpoint, Duration::from_secs(1))
            .await
            .expect("closed-gate successor must answer Health");
        let blocked = request_over_socket(
            endpoint.socket(),
            WalkRequestBody::Step {
                guard: test_guard(9_001),
                until: None,
                watch: false,
                allow_live_api: false,
                allow_git_changes: false,
            },
        )
        .await
        .expect("query closed successor gate");
        assert!(matches!(
            blocked,
            WalkResponse::Error {
                code: WalkErrorCode::TransferPending,
                ..
            }
        ));

        let health = health_over_socket(endpoint.socket())
            .await
            .expect("observe successor epoch");
        let WalkResponse::Status { epoch, .. } = health else {
            panic!("Health must return successor epoch");
        };
        let mut stream = ipc::connect(endpoint.socket())
            .await
            .expect("connect successor Stop");
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_protocol: Some(WALK_PROTOCOL_VERSION),
                client_epoch: Some(epoch),
                body: WalkRequestBody::Stop,
            },
        )
        .await
        .expect("send successor Stop");
        let _: WalkResponse = ipc::recv(&mut stream)
            .await
            .expect("receive successor Stop");
        serving
            .await
            .expect("join successor service")
            .expect("serve successor");
        assert!(!endpoint.owns_socket());
    }

    #[tokio::test]
    async fn session_history_does_not_wait_for_the_controller() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let expected_epoch = server.epoch.clone();
        let _controller = server.controller.lock().await;

        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::SessionHistory)),
        )
        .await
        .expect("session history must not wait for the controller lock");

        assert!(!stop);
        let WalkResponse::History { history } = response else {
            panic!("expected session-history response");
        };
        assert_eq!(history.version, SessionVersion::empty());
        assert!(history.journal_path.is_none());
        assert!(history.origin.is_none());
        assert!(history.profile.is_none());
        assert!(history.events.is_empty());
        assert!(history.damage.is_none());
        assert!(history.abandonment.is_none());
        assert_eq!(history.epoch, expected_epoch);
    }

    #[tokio::test]
    async fn evaluation_trace_does_not_wait_for_the_controller() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let _controller = server.controller.lock().await;

        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::EvaluationTraceIndex)),
        )
        .await
        .expect("evaluation trace must not wait for the controller lock");

        assert!(!stop);
        let WalkResponse::Error { phase, detail, .. } = response else {
            panic!("empty checkout should return a typed trace error");
        };
        assert_eq!(phase, Some(WalkPhase::Empty));
        assert!(
            detail.contains("parent identity")
                || detail.contains("campaign")
                || detail.contains("run manifest"),
            "unexpected trace setup error: {detail}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn llm_inspection_does_not_wait_for_an_active_controller_job() {
        let root = tempdir().expect("test root");
        let eval_home = root.path().join("eval-home");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let repo = root.path().join("repo");
        let campaign = CampaignId::from("walk-live-llm-inspection");
        let parent = ParentIdentity::root_bootstrap(
            campaign.clone(),
            "node-root",
            "instance-1",
            "branch-1",
            None,
        );
        write_parent_identity(&repo, &parent).expect("write parent identity");
        let store = FsToolLoopStore::new(
            eval_home
                .join("campaigns")
                .join(campaign.as_str())
                .join("prototype1/debug/tool-loop"),
        );
        let mut session = ToolLoopSession::new("session-live", "headless-tui", repo.join("lane-a"));
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("write session");

        let server = test_server(&repo, MutationGate::open());
        let job = accepted(
            server
                .register_job(
                    test_guard(10_005),
                    b"step".to_vec(),
                    test_step_intent(WalkPhase::R6),
                )
                .await
                .expect("register active step"),
        );
        assert_eq!(job.status, WalkJobStatus::Running);
        let _controller = server.controller.lock().await;

        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::LlmLanes { verbose: true })),
        )
        .await
        .expect("llm lanes must not wait for the controller lock");
        assert!(!stop);
        let WalkResponse::Ok {
            result: WalkOkPayload::LlmLanes { report },
            ..
        } = response
        else {
            panic!("expected llm-lanes response");
        };
        assert!(report.contains("lane-a"));
        assert!(report.contains("session-live"));

        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::LlmTraceIndex)),
        )
        .await
        .expect("typed LLM index must not wait for the controller lock");
        assert!(!stop);
        let WalkResponse::LlmTraceIndex { index } = response else {
            panic!("expected typed LLM trace index");
        };
        assert_eq!(index.campaign, campaign);
        assert_eq!(index.lanes.len(), 1);
        assert_eq!(index.lanes[0].lane_id, "lane-a");
        assert_eq!(index.lanes[0].sessions.len(), 1);
        assert!(index.issues.is_empty());

        let coordinate = llm_trace::LlmTraceCoordinate {
            session_id: "session-live".to_string(),
            step: None,
        };
        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::LlmTrace { coordinate })),
        )
        .await
        .expect("typed LLM detail must not wait for the controller lock");
        assert!(!stop);
        let WalkResponse::LlmTrace { snapshot } = response else {
            panic!("expected typed LLM trace detail");
        };
        assert_eq!(snapshot.session.value.session_id, "session-live");
        assert!(matches!(
            snapshot.resume,
            llm_trace::LlmArtifact::Missing { .. }
        ));
        assert!(snapshot.timeline.is_empty());

        let (response, stop) = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::LlmShow {
                session_id: Some("session-live".to_string()),
                lane: None,
                head: false,
                step: None,
            })),
        )
        .await
        .expect("llm show must not wait for the controller lock");
        assert!(!stop);
        let WalkResponse::Ok {
            result: WalkOkPayload::LlmShow { report },
            ..
        } = response
        else {
            panic!("expected llm-show response");
        };
        assert!(report.contains("session: session-live"));
        assert!(report.contains("head: -"));
        assert_eq!(
            server.latest_job().await.map(|job| job.status),
            Some(WalkJobStatus::Running)
        );
    }

    #[tokio::test]
    async fn start_rejects_epoch_config_root_mismatch_before_job_admission() {
        let repo = tempdir().expect("server repo");
        let other = tempdir().expect("other repo");
        let server = test_server(repo.path(), MutationGate::open());

        let (response, stop) = server
            .handle(WalkRequest {
                client_protocol: None,
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
    async fn start_rejects_failed_fresh_admission_before_operation_persistence() {
        let repo = tempdir().expect("server repo");
        let server = test_server(repo.path(), MutationGate::open());
        let guard = test_guard(41);
        let operation = guard.operation;
        let operation_path = server.operation_path(operation);

        let (response, stop) = server
            .handle(WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: Some(server.epoch.clone()),
                body: WalkRequestBody::Start {
                    guard,
                    config: WalkStartConfig {
                        campaign: None,
                        repo_root: Some(repo.path().to_path_buf()),
                    },
                    until: WalkPhase::R3,
                    allow_live_api: false,
                },
            })
            .await;

        assert!(!stop);
        let WalkResponse::Error {
            code,
            detail,
            version: Some(version),
            ..
        } = response
        else {
            panic!("failed fresh admission must return a typed conflict");
        };
        assert_eq!(code, WalkErrorCode::RecoveryInProgress);
        assert!(
            detail.contains("fresh-session admission failed"),
            "{detail}"
        );
        assert_eq!(version, SessionVersion::empty());
        assert!(server.latest_job().await.is_none());
        assert!(
            server
                .load_operation(operation)
                .expect("inspect rejected operation")
                .is_none()
        );
        assert!(
            !operation_path.exists(),
            "rejected fresh admission must not create a durable operation record"
        );
        assert!(
            fs::read_dir(&server.operation_root)
                .expect("read operation directory")
                .next()
                .is_none(),
            "rejected fresh admission must leave the operation directory empty"
        );
    }

    #[tokio::test]
    async fn recover_without_session_rejects_before_operation_persistence() {
        let repo = tempdir().expect("server repo");
        let server = test_server(repo.path(), MutationGate::open());
        server.observed.update(ControllerObservation {
            phase: WalkPhase::R5,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Blocked("R5 cannot begin a fresh session".to_string()),
        });

        let status = server
            .status_response("walk server online")
            .await
            .expect("blocked status");
        let WalkResponse::Status { snapshot, .. } = status else {
            panic!("expected status response");
        };
        assert!(
            snapshot
                .actions
                .iter()
                .all(|action| action.kind != WalkActionKind::Recover),
            "a checkout without a durable session must not advertise recovery"
        );

        let guard = test_guard(42);
        let operation = guard.operation;
        let (response, stop) = server
            .handle(WalkRequest {
                client_protocol: Some(
                    crate::cli::prototype1_state::walk::epoch::WALK_PROTOCOL_VERSION,
                ),
                client_epoch: Some(server.epoch.clone()),
                body: WalkRequestBody::Recover {
                    directive: RecoveryDirective::AdmitEpoch,
                    guard: Some(guard),
                },
            })
            .await;

        assert!(!stop);
        let WalkResponse::Error { code, detail, .. } = response else {
            panic!("unavailable recovery must return a typed error");
        };
        assert_eq!(code, WalkErrorCode::BadRequest);
        assert!(detail.contains("committed session cursor"), "{detail}");
        assert!(server.latest_job().await.is_none());
        assert!(
            server
                .load_operation(operation)
                .expect("inspect rejected recovery")
                .is_none(),
            "unavailable recovery must not create an operation record"
        );
    }

    #[tokio::test]
    async fn effectful_debug_requests_are_supervised_and_durable() {
        for expected in ["branch_live", "llm_step", "llm_finish"] {
            let repo = tempdir().expect("repo tempdir");
            let (kind, body) = transfer_cases(repo.path())
                .into_iter()
                .find(|(kind, _)| *kind == expected)
                .expect("effectful debug request");
            let server = test_server(repo.path(), MutationGate::open());
            let request = WalkRequest {
                client_protocol: None,
                client_epoch: Some(server.epoch.clone()),
                body,
            };
            let (response, stop) = server.handle(request).await;
            assert!(!stop, "{kind} submission must not stop the server");
            let operation = match response {
                WalkResponse::Job { job, .. } => {
                    assert_eq!(job.command.as_str(), kind);
                    job.operation_id
                }
                other => panic!("{kind} did not enter the job registry: {other:?}"),
            };
            let terminal = tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    let job = server
                        .operation_job(operation)
                        .await
                        .expect("supervised operation remains observable");
                    if !job.status.is_active() {
                        break job;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("effectful debug job must settle");
            assert_eq!(
                server
                    .load_operation(operation)
                    .expect("load durable operation")
                    .expect("durable operation exists")
                    .stored
                    .snapshot,
                terminal
            );
        }
    }

    #[tokio::test]
    async fn branch_live_without_capability_records_failed_job_only() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let (response, stop) = server
            .handle(WalkRequest {
                client_protocol: None,
                client_epoch: Some(server.epoch.clone()),
                body: WalkRequestBody::BranchLive {
                    guard: test_guard(8),
                    reason: "must not be written".to_string(),
                    allow_provenance_record: false,
                },
            })
            .await;
        assert!(!stop);
        let WalkResponse::Job { job, .. } = response else {
            panic!("branch_live did not return its supervised receipt");
        };
        assert_eq!(job.status, WalkJobStatus::Failed);
        assert_eq!(job.allow_provenance_record, Some(false));
        assert!(
            job.message
                .as_deref()
                .is_some_and(|message| { message.contains("--allow provenance-record") })
        );
        assert_eq!(
            server
                .load_operation(job.operation_id)
                .expect("load branch operation")
                .expect("branch operation receipt")
                .stored
                .snapshot,
            job
        );
    }

    #[tokio::test]
    async fn show_reports_active_job_without_waiting_for_controller() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
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
            (WalkResponse::Status { snapshot, .. }, false) => {
                let active = snapshot.job.as_ref().expect("active job snapshot");
                assert_eq!(snapshot.phase(), WalkPhase::Empty);
                assert_eq!(active.job_id, job.job_id);
                assert_eq!(active.status, WalkJobStatus::Running);
            }
            other => panic!("show did not report the active job: {other:?}"),
        }
    }

    #[tokio::test]
    async fn delta_reports_prior_result_without_waiting_for_controller() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let snapshot = WalkDeltaSnapshot {
            version: SessionVersion::empty(),
            state: WalkDeltaState::Recorded {
                from: WalkPhase::Empty,
                edges: Vec::new(),
            },
        };
        *server.delta.write().await = PublishedDelta {
            source_job: None,
            phase: WalkPhase::Empty,
            snapshot: snapshot.clone(),
            plain: "prior plain delta".to_string(),
            verbose: "prior verbose delta".to_string(),
            color: "prior color delta".to_string(),
            verbose_color: "prior verbose color delta".to_string(),
        };
        server
            .register_job(
                test_guard(10_002),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register test job");
        let _controller = server.controller.lock().await;

        let handled = tokio::time::timeout(
            Duration::from_millis(100),
            server.handle(walk_request(WalkRequestBody::ShowDelta {
                verbose: true,
                color: false,
            })),
        )
        .await
        .expect("delta must not wait for the live job's controller lock");

        let (
            WalkResponse::Delta {
                phase,
                report,
                snapshot: observed,
                ..
            },
            false,
        ) = handled
        else {
            panic!("show delta did not return the prior completed result: {handled:?}");
        };
        assert_eq!(phase, WalkPhase::Empty);
        assert_eq!(report, "prior verbose delta");
        assert_eq!(observed, snapshot);
    }

    #[tokio::test]
    async fn delta_publication_requires_terminal_receipt() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10_003),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register test job");
        let published = PublishedDelta {
            source_job: Some(job.job_id),
            phase: WalkPhase::Empty,
            snapshot: WalkDeltaSnapshot {
                version: SessionVersion::empty(),
                state: WalkDeltaState::Recorded {
                    from: WalkPhase::Empty,
                    edges: Vec::new(),
                },
            },
            plain: "completed delta".to_string(),
            verbose: "completed delta".to_string(),
            color: "completed delta".to_string(),
            verbose_color: "completed delta".to_string(),
        };

        publish_delta(&server.delta, &server.jobs, job.job_id, published.clone()).await;
        assert!(matches!(
            &server.delta.read().await.snapshot.state,
            WalkDeltaState::NotRecorded
        ));

        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active test job");
            active.snapshot.status = WalkJobStatus::Succeeded;
            active.snapshot.receipt = Some(WalkTransitionReceipt {
                phase_before: WalkPhase::Empty,
                phase_after: WalkPhase::Empty,
                edges: Vec::new(),
                version: SessionVersion::empty(),
                event_projection: WalkEventProjection::Unknown,
            });
        }
        publish_delta(&server.delta, &server.jobs, job.job_id, published).await;
        let delta = server.delta.read().await;
        assert_eq!(delta.plain, "completed delta");
        assert!(matches!(
            &delta.snapshot.state,
            WalkDeltaState::Recorded { .. }
        ));
    }

    #[tokio::test]
    async fn older_job_cannot_replace_newer_delta() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let first = server
            .register_job(
                test_guard(10_004),
                b"first-step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register first test job");
        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active first job");
            active.snapshot.status = WalkJobStatus::Succeeded;
            active.snapshot.receipt = Some(WalkTransitionReceipt {
                phase_before: WalkPhase::Empty,
                phase_after: WalkPhase::Empty,
                edges: Vec::new(),
                version: SessionVersion::empty(),
                event_projection: WalkEventProjection::Unknown,
            });
        }
        let second = server
            .register_job(
                test_guard(10_005),
                b"second-step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register second test job");
        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active second job");
            active.snapshot.status = WalkJobStatus::Succeeded;
            active.snapshot.receipt = Some(WalkTransitionReceipt {
                phase_before: WalkPhase::Empty,
                phase_after: WalkPhase::Empty,
                edges: Vec::new(),
                version: SessionVersion::empty(),
                event_projection: WalkEventProjection::Unknown,
            });
        }
        let published = |job_id, report: &str| PublishedDelta {
            source_job: Some(job_id),
            phase: WalkPhase::Empty,
            snapshot: WalkDeltaSnapshot {
                version: SessionVersion::empty(),
                state: WalkDeltaState::Recorded {
                    from: WalkPhase::Empty,
                    edges: Vec::new(),
                },
            },
            plain: report.to_string(),
            verbose: report.to_string(),
            color: report.to_string(),
            verbose_color: report.to_string(),
        };

        publish_delta(
            &server.delta,
            &server.jobs,
            second.job_id,
            published(second.job_id, "newer delta"),
        )
        .await;
        publish_delta(
            &server.delta,
            &server.jobs,
            first.job_id,
            published(first.job_id, "older delta"),
        )
        .await;

        let delta = server.delta.read().await;
        assert_eq!(delta.source_job, Some(second.job_id));
        assert_eq!(delta.plain, "newer delta");
    }

    #[test]
    fn cleared_delta_retains_observed_durable_version() {
        let version = SessionVersion {
            session_id: None,
            cursor: None,
            journal_revision: 7,
        };
        let delta = PublishedDelta::not_recorded(WalkPhase::Empty, version.clone(), Some(7));

        assert_eq!(delta.source_job, Some(7));
        assert_eq!(delta.phase, WalkPhase::Empty);
        assert_eq!(delta.snapshot.version, version);
        assert!(matches!(delta.snapshot.state, WalkDeltaState::NotRecorded));
    }

    #[test]
    fn contended_cache_does_not_claim_an_attached_controller() {
        let cache = ControllerCache::new(ControllerObservation {
            phase: WalkPhase::R4c,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Ready,
        });
        let _write = cache.current.write().expect("hold cache writer");

        let observation = cache.read();

        assert_eq!(observation.phase, WalkPhase::Empty);
        assert!(!observation.attached);
        assert!(
            observation
                .blocker
                .as_deref()
                .is_some_and(|detail| detail.contains("cache is contended"))
        );
    }

    #[tokio::test]
    async fn pre_session_health_uses_cached_reconstruction_while_controller_is_locked() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        server.observed.update(ControllerObservation {
            phase: WalkPhase::R4c,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Ready,
        });
        let _controller = server.controller.lock().await;

        let response = tokio::time::timeout(
            Duration::from_millis(100),
            server.status_response("walk server online"),
        )
        .await
        .expect("health must not wait for the controller lock")
        .expect("health response");
        let WalkResponse::Status { snapshot, .. } = response else {
            panic!("expected status response");
        };
        assert_eq!(snapshot.phase(), WalkPhase::R4c);
        assert!(matches!(
            &snapshot.position,
            WalkPosition::Reconstruction {
                phase: WalkPhase::R4c
            }
        ));
        assert_eq!(snapshot.version(), SessionVersion::empty());
        assert!(snapshot.controller_attached);
        assert!(
            snapshot.actions.iter().any(|action| {
                action.kind == WalkActionKind::Start
                    && action.target == Some(WalkPhase::R3)
                    && action.enabled
            }),
            "setup reconstruction must advertise the R3 controller-session claim"
        );
        assert!(
            snapshot
                .actions
                .iter()
                .all(|action| !matches!(action.kind, WalkActionKind::Step | WalkActionKind::Reset)),
            "pre-session reconstruction must not advertise session-only controls"
        );

        server.observed.update(ControllerObservation {
            phase: WalkPhase::R5,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Blocked(
                "pre-session loop artifacts reconstruct through r5; preserve this run as read-only or migrate it explicitly"
                    .to_string(),
            ),
        });
        let response = server
            .status_response("walk server online")
            .await
            .expect("historical status response");
        let WalkResponse::Status { snapshot, .. } = response else {
            panic!("expected historical status response");
        };
        assert_eq!(snapshot.phase(), WalkPhase::R5);
        assert_eq!(snapshot.authority, WalkAuthority::RecoveryRequired);
        assert_eq!(
            snapshot.blocker.as_ref().map(|blocker| blocker.code),
            Some(WalkBlockerCode::ControllerBlocked)
        );
        assert!(snapshot.actions.iter().any(|action| {
            action.kind == WalkActionKind::Start
                && !action.enabled
                && action.blocker == Some(WalkBlockerCode::ControllerBlocked)
        }));
        assert!(
            snapshot
                .actions
                .iter()
                .all(|action| action.kind != WalkActionKind::Recover),
            "a blocked pre-session checkout has no durable recovery target"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cursorless_v1_status_serializes_through_the_production_store() {
        let root = tempdir().expect("test root");
        let eval_home = root.path().join("eval-home");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let repo = root.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        let campaign = CampaignId::from("walk-cursorless-v1");
        let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: campaign.clone(),
            parent_id: "node-parent".to_string(),
            node_id: "node-parent".to_string(),
            generation: 1,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: Some("node-root".to_string()),
            parent_node_id: Some("node-root".to_string()),
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("artifact-parent".to_string()),
            created_at: "2026-06-30T21:38:52Z".to_string(),
        });
        write_parent_identity(&repo, &parent).expect("write parent identity");
        let profile = RunProfileCommitment {
            schema_version: "prototype1-run-profile-commitment.v1".to_string(),
            profile_path: PathBuf::from("prototype1/run-profile.toml"),
            sha256: "a".repeat(64),
            source_path: Some(PathBuf::from("operator-profile.toml")),
            admitted_at: "2026-06-30T20:00:00Z".to_string(),
        };
        let cursor =
            Cursor::new(WalkPhase::R3, ContentHash::of("v1 origin")).expect("valid origin cursor");
        // No real v1 journal survived in the fixture corpus. This synthetic compatibility
        // shape is still persisted with the canonical serializer and read only by production code.
        let claim = Claim::historical(
            ContentHash::of("cursorless-v1-fixture"),
            parent.clone(),
            profile,
            RunMode::Step,
            cursor,
            ServerEpoch::capture(&repo).expect("capture fixture epoch"),
        );
        let manifest = campaign_manifest_path(&campaign).expect("campaign manifest path");
        let session_id = Store::for_manifest(&manifest)
            .write_v1_active(&claim)
            .expect("persist cursorless v1 session");
        let server = test_server(&repo, MutationGate::open());
        server
            .controller
            .lock()
            .await
            .refresh_from_disk()
            .expect("cursorless active-owner session remains inspectable");

        let response = server
            .status_response("walk server online")
            .await
            .expect("cursorless status response");
        let encoded = serde_json::to_value(&response).expect("serialize v9 status response");
        let decoded: WalkResponse =
            serde_json::from_value(encoded.clone()).expect("decode serialized v9 status response");
        assert!(matches!(decoded, WalkResponse::Status { .. }));
        let WalkResponse::Status { snapshot, .. } = response else {
            panic!("expected status response");
        };

        assert_eq!(snapshot.phase(), WalkPhase::Empty);
        assert_eq!(snapshot.version().session_id(), Some(session_id));
        assert_eq!(snapshot.version().cursor(), None);
        assert_eq!(snapshot.version().journal_revision(), 2);
        assert!(!snapshot.controller_attached);
        assert_eq!(snapshot.authority, WalkAuthority::RecoveryRequired);
        assert!(matches!(
            snapshot.blocker.as_ref(),
            Some(WalkBlocker {
                code: WalkBlockerCode::ControllerBlocked,
                detail,
            }) if detail.contains("owner is recorded at fence 1")
        ));
        assert_eq!(encoded["snapshot"]["phase"], "empty");
        assert_eq!(
            encoded["snapshot"]["version"],
            serde_json::to_value(snapshot.version()).expect("serialize expected version")
        );
        assert_eq!(encoded["snapshot"]["position"]["source"], "unpositioned");
        assert!(
            snapshot.actions.iter().all(|action| !matches!(
                action.kind,
                WalkActionKind::Start
                    | WalkActionKind::Step
                    | WalkActionKind::Reset
                    | WalkActionKind::Recover
            )),
            "cursorless sessions cannot satisfy any controller-mutation precondition"
        );
    }

    #[tokio::test]
    async fn indeterminate_status_uses_durable_phase() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        server.observed.update(ControllerObservation {
            phase: WalkPhase::R4c,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Ready,
        });
        server
            .register_job(
                test_guard(10_000),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register test job");
        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active test job");
            active.snapshot.status = WalkJobStatus::Indeterminate;
            active.snapshot.phase_before = WalkPhase::R6;
            active.snapshot.phase_after = None;
        }

        let response = server
            .status_response("walk server online")
            .await
            .expect("status response");
        let WalkResponse::Status { snapshot, .. } = response else {
            panic!("expected status response");
        };
        assert_eq!(snapshot.phase(), WalkPhase::Empty);
        assert!(matches!(&snapshot.position, WalkPosition::NoSession));
        assert_eq!(snapshot.version(), SessionVersion::empty());

        let (response, stop) = server
            .handle(walk_request(WalkRequestBody::OperationStatus {
                operation: OperationId::for_test(10_000),
            }))
            .await;
        assert!(!stop);
        let WalkResponse::Job { phase, .. } = response else {
            panic!("expected operation status response");
        };
        assert_eq!(phase, WalkPhase::Empty);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn indeterminate_session_status_does_not_advertise_recover() {
        let root = tempdir().expect("test root");
        let eval_home = root.path().join("eval-home");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let repo = root.path().join("repo");
        fs::create_dir_all(&repo).expect("create repo");
        let campaign = CampaignId::from("walk-indeterminate-session");
        let parent = ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_string(),
            campaign_id: campaign.clone(),
            parent_id: "node-parent".to_string(),
            node_id: "node-parent".to_string(),
            generation: 1,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: Some("node-root".to_string()),
            parent_node_id: Some("node-root".to_string()),
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("artifact-parent".to_string()),
            created_at: "2026-06-30T21:38:52Z".to_string(),
        });
        write_parent_identity(&repo, &parent).expect("write parent identity");
        let profile = RunProfileCommitment {
            schema_version: "prototype1-run-profile-commitment.v1".to_string(),
            profile_path: PathBuf::from("prototype1/run-profile.toml"),
            sha256: "a".repeat(64),
            source_path: Some(PathBuf::from("operator-profile.toml")),
            admitted_at: "2026-06-30T20:00:00Z".to_string(),
        };
        let cursor = Cursor::new(
            WalkPhase::R6,
            ContentHash::of("indeterminate session cursor"),
        )
        .expect("valid session cursor");
        let claim = Claim::historical(
            ContentHash::of("indeterminate-session-fixture"),
            parent,
            profile,
            RunMode::Step,
            cursor,
            ServerEpoch::capture(&repo).expect("capture fixture epoch"),
        );
        let manifest = campaign_manifest_path(&campaign).expect("campaign manifest path");
        let lease = match Store::for_manifest(&manifest)
            .claim(claim)
            .expect("claim fixture session")
        {
            Outcome::Acquired(lease) => lease,
            Outcome::Conflict(_) | Outcome::Recoverable(_) => {
                panic!("fresh fixture session must acquire normally")
            }
        };
        assert!(lease.release().is_ok(), "release fixture session");

        let server = test_server(&repo, MutationGate::open());
        let expected = server.durable_version().expect("read durable session");
        server
            .register_job(
                MutationGuard {
                    operation: OperationId::for_test(10_001),
                    expected: expected.clone(),
                },
                b"step".to_vec(),
                test_step_intent(WalkPhase::R7),
            )
            .await
            .map(accepted)
            .expect("register test job");
        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active test job");
            active.snapshot.status = WalkJobStatus::Indeterminate;
            active.snapshot.phase_before = WalkPhase::R6;
        }
        server.observed.update(ControllerObservation {
            phase: WalkPhase::R6,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Ready,
        });
        let _controller = server.controller.lock().await;

        let response = server
            .status_response("walk server online")
            .await
            .expect("status response");
        let WalkResponse::Status { snapshot, .. } = response else {
            panic!("expected status response");
        };

        assert_eq!(snapshot.version(), expected);
        assert!(matches!(snapshot.position, WalkPosition::Session { .. }));
        assert_eq!(snapshot.authority, WalkAuthority::RecoveryRequired);
        assert_eq!(
            snapshot.blocker.as_ref().map(|blocker| blocker.code),
            Some(WalkBlockerCode::JobIndeterminate)
        );
        assert!(
            snapshot
                .actions
                .iter()
                .all(|action| action.kind != WalkActionKind::Recover),
            "an indeterminate job must be resolved before session recovery can be admitted"
        );
    }

    #[tokio::test]
    async fn verified_audit_refreshes_cached_attachment() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        server.observed.update(ControllerObservation {
            phase: WalkPhase::R4c,
            attached: true,
            blocker: None,
            fresh: FreshAdmission::Ready,
        });

        let (response, stop) = server
            .handle(walk_request(WalkRequestBody::Audit {
                campaign: None,
                scope: Prototype1StateWalkAuditScope::R0ToR1,
                transition: None,
                verify: true,
                verbose: false,
                with_note: false,
            }))
            .await;

        assert!(!stop);
        assert!(matches!(response, WalkResponse::Audit { .. }));
        assert_eq!(server.observed.read().phase, WalkPhase::Empty);
    }

    #[tokio::test]
    async fn idle_ttl_waits_for_active_job() {
        let repo = tempdir().expect("repo tempdir");
        let listener = UnixListener::bind(repo.path().join("walk.sock")).expect("bind walk socket");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10_001),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .map(accepted)
            .expect("register active job");
        let jobs = Arc::clone(&server.jobs);
        let operation_root = server.operation_root.clone();
        let epoch = server.epoch.clone();
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
            &operation_root,
            &epoch,
            job.job_id,
            WalkJobStatus::Failed,
            None,
            "test job settled".to_string(),
            None,
        )
        .await;
        tokio::time::timeout(Duration::from_millis(200), serving)
            .await
            .expect("server should apply TTL after the job settles")
            .expect("accept loop task")
            .expect("accept loop result");
    }

    #[tokio::test]
    async fn disconnected_health_client_does_not_terminate_accept_loop() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("walk.sock");
        let listener = UnixListener::bind(&socket).expect("bind walk socket");
        let server = test_server(repo.path(), MutationGate::open());
        let jobs = Arc::clone(&server.jobs);
        let jobs_guard = jobs.lock().await;
        let serving = tokio::spawn(accept_loop(server, listener, None));

        let mut disconnected = ipc::connect(&socket).await.expect("connect first client");
        ipc::send(&mut disconnected, &walk_request(WalkRequestBody::Health))
            .await
            .expect("send first health request");
        drop(disconnected);
        drop(jobs_guard);

        let later_health =
            tokio::time::timeout(Duration::from_millis(250), health_over_socket(&socket)).await;
        serving.abort();
        let _ = serving.await;

        let response = later_health
            .expect("later health request must not time out")
            .expect("later health request must reach the same server");
        assert!(matches!(response, WalkResponse::Status { .. }));
    }

    #[tokio::test]
    async fn successor_retirement_waits_for_predecessor_job_receipt() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("predecessor.sock");
        let listener = UnixListener::bind(&socket).expect("bind predecessor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture predecessor endpoint");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10_002),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R13b),
            )
            .await
            .map(accepted)
            .expect("register predecessor handoff job");
        let jobs = Arc::clone(&server.jobs);
        let operation_root = server.operation_root.clone();
        let epoch = server.epoch.clone();
        let owned = endpoint.clone();
        let serving = tokio::spawn(async move {
            let result = accept_loop(server, listener, None).await;
            owned.cleanup().expect("cleanup retired predecessor");
            result
        });
        let retiring = tokio::spawn({
            let endpoint = endpoint.clone();
            async move { retire_predecessor(&endpoint).await }
        });

        tokio::time::sleep(Duration::from_millis(5_250)).await;
        assert!(
            !retiring.is_finished(),
            "predecessor retirement must outlive the former five-second deadline while its exact outer handoff job is active"
        );
        finish_job(
            &jobs,
            &operation_root,
            &epoch,
            job.job_id,
            WalkJobStatus::Succeeded,
            Some(WalkPhase::R13b),
            "handoff receipt published".to_string(),
            Some(WalkTransitionReceipt {
                phase_before: WalkPhase::R12,
                phase_after: WalkPhase::R13b,
                edges: vec![ControlEdge::R12ToR13b],
                version: test_version(WalkPhase::R13b, 10_002),
                event_projection: WalkEventProjection::Recorded,
            }),
        )
        .await;

        tokio::time::timeout(Duration::from_secs(1), retiring)
            .await
            .expect("predecessor retirement must complete")
            .expect("retirement task")
            .expect("retire predecessor");
        tokio::time::timeout(Duration::from_secs(1), serving)
            .await
            .expect("predecessor server must stop")
            .expect("predecessor server task")
            .expect("predecessor accept loop");
        assert!(!endpoint.owns_socket());
    }

    #[tokio::test]
    async fn successor_retirement_waits_for_stop_response() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("predecessor.sock");
        let listener = UnixListener::bind(&socket).expect("bind predecessor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture predecessor endpoint");
        let server = test_server(repo.path(), MutationGate::open());
        let jobs = Arc::clone(&server.jobs);
        let jobs_guard = jobs.lock().await;
        let owned = endpoint.clone();
        let serving = tokio::spawn(async move {
            let result = accept_loop(server, listener, None).await;
            owned.cleanup().expect("cleanup retired predecessor");
            result
        });
        let retiring = tokio::spawn({
            let endpoint = endpoint.clone();
            async move { retire_predecessor(&endpoint).await }
        });

        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(
            !retiring.is_finished(),
            "predecessor retirement must not fail while an accepted Stop waits for terminal-persistence synchronization"
        );
        drop(jobs_guard);

        tokio::time::timeout(Duration::from_secs(1), retiring)
            .await
            .expect("predecessor retirement must complete")
            .expect("retirement task")
            .expect("retire predecessor");
        tokio::time::timeout(Duration::from_secs(1), serving)
            .await
            .expect("predecessor server must stop")
            .expect("predecessor server task")
            .expect("predecessor accept loop");
        assert!(!endpoint.owns_socket());
    }

    #[tokio::test]
    async fn successor_retirement_rejects_indeterminate_predecessor() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("predecessor.sock");
        let listener = UnixListener::bind(&socket).expect("bind predecessor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture predecessor endpoint");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10_003),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R13b),
            )
            .await
            .map(accepted)
            .expect("register predecessor handoff job");
        {
            let mut jobs = server.jobs.lock().await;
            let active = jobs.active.as_mut().expect("active predecessor job");
            assert_eq!(active.snapshot.job_id, job.job_id);
            active.snapshot.status = WalkJobStatus::Indeterminate;
        }
        let owned = endpoint.clone();
        let serving = tokio::spawn(async move {
            let result = accept_loop(server, listener, None).await;
            owned.cleanup().expect("cleanup predecessor endpoint");
            result
        });

        let error = tokio::time::timeout(Duration::from_secs(1), retire_predecessor(&endpoint))
            .await
            .expect("non-drain retirement error must be immediate")
            .expect_err("indeterminate predecessor must not be retired");
        let detail = error.to_string();
        assert!(detail.contains("recovery_in_progress"), "{detail}");
        assert!(detail.contains("Indeterminate"), "{detail}");
        assert!(
            !serving.is_finished(),
            "rejected retirement must leave the predecessor service online"
        );

        serving.abort();
        let _ = serving.await;
        endpoint.cleanup().expect("cleanup aborted predecessor");
    }

    #[test]
    fn stage7_retirement_replays_outer_receipt_gap() {
        const FORMER_RETIRE_MS: i64 = 5_000;

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/tests/fixtures/prototype1-stage7-handoff-retirement-20260715");
        let predecessor_bytes =
            inflate_hex(&fixture.join("predecessor-control-journal.jsonl.gz.hex"));
        let successor_bytes = inflate_hex(&fixture.join("successor-control-journal.jsonl.gz.hex"));
        let predecessor = journal_parent(&predecessor_bytes);
        let successor = journal_parent(&successor_bytes);
        assert_eq!(predecessor.generation(), 0);
        assert_eq!(successor.generation(), 1);
        assert_eq!(
            successor.previous_parent_id(),
            Some(predecessor.parent_id())
        );

        let temp = tempdir().expect("historical replay tempdir");
        let invocation_path = temp.path().join("successor-invocation.json");
        fs::write(
            &invocation_path,
            decode_hex(&fixture.join("successor-invocation.json.hex")),
        )
        .expect("install exact historical successor invocation");
        let InvocationAuthority::Successor(invocation) =
            invocation::load_authority(&invocation_path)
                .expect("load historical successor invocation")
        else {
            panic!("historical invocation must carry successor authority");
        };
        let attempt = invocation
            .predecessor_attempt()
            .expect("historical successor preserves predecessor attempt");
        assert_eq!(attempt.fence().to_string(), "12");

        let store = Store::new(temp.path().join("control"));
        let predecessor_path = store.paths(&predecessor).journal().to_path_buf();
        let successor_path = store.paths(&successor).journal().to_path_buf();
        fs::create_dir_all(
            predecessor_path
                .parent()
                .expect("predecessor journal parent"),
        )
        .expect("create predecessor replay directory");
        fs::create_dir_all(successor_path.parent().expect("successor journal parent"))
            .expect("create successor replay directory");
        fs::write(&predecessor_path, predecessor_bytes)
            .expect("install predecessor historical journal");
        fs::write(&successor_path, successor_bytes).expect("install successor historical journal");

        let epoch = ServerEpoch::capture(temp.path()).expect("capture replay presentation epoch");
        let predecessor_history = store
            .inspect_history(&predecessor, epoch.clone())
            .expect("replay predecessor history")
            .expect("predecessor history exists");
        let successor_history = store
            .inspect_history(&successor, epoch)
            .expect("replay successor history")
            .expect("successor history exists");
        assert!(predecessor_history.damage.is_none());
        assert!(successor_history.damage.is_none());
        assert_eq!(
            predecessor_history.version.session_id(),
            Some(attempt.session())
        );

        let predecessor_release = predecessor_history
            .events
            .iter()
            .find(|event| {
                matches!(
                    event.kind,
                    WalkSessionEventKind::Released {
                        fence: 12,
                        ready: None
                    }
                )
            })
            .expect("replay exact predecessor release");
        let successor_ready = successor_history
            .events
            .iter()
            .find_map(|event| match &event.kind {
                WalkSessionEventKind::Released {
                    fence: 1,
                    ready: Some(ready),
                } => Some((event, ready)),
                _ => None,
            })
            .expect("replay atomic successor Ready release");
        assert_eq!(
            successor_ready.1.runtime_id.to_string(),
            invocation.runtime_id().to_string()
        );
        assert_eq!(successor_ready.1.commit.cursor.phase, WalkPhase::R4c);
        assert!(successor_ready.0.recorded_at_ms < predecessor_release.recorded_at_ms);

        let operation: DurableOperation =
            serde_json::from_slice(&decode_hex(&fixture.join("predecessor-operation.json.hex")))
                .expect("decode historical predecessor operation");
        let snapshot = &operation.stored.snapshot;
        assert_eq!(
            snapshot.operation_id.to_string(),
            "50d299f7-ee5c-4c28-91fd-8bd31af9aaf7"
        );
        assert_eq!(snapshot.status, WalkJobStatus::Succeeded);
        let receipt = snapshot
            .receipt
            .as_ref()
            .expect("historical operation has terminal handoff receipt");
        assert_eq!(receipt.phase_before, WalkPhase::R12);
        assert_eq!(receipt.phase_after, WalkPhase::R13b);
        assert!(receipt.edges.contains(&ControlEdge::R12ToR13b));
        let finished = chrono::DateTime::parse_from_rfc3339(
            snapshot
                .finished_at
                .as_deref()
                .expect("operation finish time"),
        )
        .expect("parse operation finish time")
        .timestamp_millis();
        assert!(
            finished - predecessor_release.recorded_at_ms > FORMER_RETIRE_MS,
            "historical outer receipt must settle after the former retirement deadline"
        );

        let channel: Envelope<ToParent> = serde_json::from_str(
            fs::read_to_string(fixture.join("successor-ready-channel.jsonl"))
                .expect("read historical Ready channel")
                .trim(),
        )
        .expect("decode historical Ready channel");
        assert_eq!(channel.runtime_id(), invocation.runtime_id());
        let ToParent::SuccessorReady {
            controller: Some(channel_ready),
            ..
        } = channel.body()
        else {
            panic!("historical channel must carry typed successor Ready");
        };
        assert_eq!(
            channel_ready.commit().session_id().to_string(),
            successor_ready.1.commit.session_id.to_string()
        );
        assert_eq!(
            channel_ready.commit().transition_id().to_string(),
            successor_ready.1.commit.transition_id.to_string()
        );
        assert_eq!(
            channel_ready.commit().fence().to_string(),
            successor_ready.1.commit.fence.to_string()
        );
        assert_eq!(
            channel_ready.commit().cursor().phase(),
            successor_ready.1.commit.cursor.phase
        );
    }

    #[test]
    fn v12_handoff_operation_replays_target_reached_before_second_claim_failure() {
        use crate::cli::prototype1_state::{
            journal::{JournalEntry, PrototypeJournal},
            successor::State as SuccessorState,
            walk::protocol::WalkAttemptResult,
        };
        use sha2::{Digest, Sha256};

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/tests/fixtures/prototype1-v12-target-reached-handoff-20260716");
        let predecessor_bytes =
            inflate_hex(&fixture.join("predecessor-control-journal.jsonl.gz.hex"));
        let successor_bytes = inflate_hex(&fixture.join("successor-control-journal.jsonl.gz.hex"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&predecessor_bytes)),
            "933132a4076b666c7b947741631d5dc9dca6f1757b7c492bb1726f69b724dcf2"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&successor_bytes)),
            "ff5030308ebef6e89baee9313ef2b9eac3558da3c746b97abe79914673cdd3f5"
        );
        let predecessor = journal_parent(&predecessor_bytes);
        let successor = journal_parent(&successor_bytes);
        assert_eq!(predecessor.generation(), 0);
        assert_eq!(successor.generation(), 1);
        assert_eq!(
            successor.previous_parent_id(),
            Some(predecessor.parent_id())
        );

        let temp = tempdir().expect("historical replay tempdir");
        let invocation_path = temp.path().join("successor-invocation.json");
        let invocation_bytes = decode_hex(&fixture.join("successor-invocation.json.hex"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&invocation_bytes)),
            "2779a6cdd2f929a1f33d1ee300cde0176e3ca74766f705003081a8580831336e"
        );
        fs::write(&invocation_path, invocation_bytes)
            .expect("install exact historical successor invocation");
        let InvocationAuthority::Successor(invocation) =
            invocation::load_authority(&invocation_path)
                .expect("load historical successor invocation")
        else {
            panic!("historical invocation must carry successor authority");
        };
        let attempt = invocation
            .predecessor_attempt()
            .expect("historical successor preserves predecessor attempt");
        assert_eq!(
            attempt.session().to_string(),
            "d9d5ddbc-1f71-4a73-900a-d2d3d34a5bf7"
        );
        assert_eq!(
            attempt.transition().to_string(),
            "d1b66192-ee2b-51cb-afcb-109bd6a0a7ec"
        );
        assert_eq!(attempt.fence().to_string(), "18");
        assert!(!attempt.allow_live_api());
        assert!(attempt.allow_git_changes());

        let store = Store::new(temp.path().join("control"));
        let predecessor_path = store.paths(&predecessor).journal().to_path_buf();
        let successor_path = store.paths(&successor).journal().to_path_buf();
        fs::create_dir_all(
            predecessor_path
                .parent()
                .expect("predecessor journal parent"),
        )
        .expect("create predecessor replay directory");
        fs::create_dir_all(successor_path.parent().expect("successor journal parent"))
            .expect("create successor replay directory");
        fs::write(&predecessor_path, predecessor_bytes)
            .expect("install predecessor historical journal");
        fs::write(&successor_path, successor_bytes).expect("install successor historical journal");

        let epoch = ServerEpoch::capture(temp.path()).expect("capture replay presentation epoch");
        let predecessor_history = store
            .inspect_history(&predecessor, epoch.clone())
            .expect("replay predecessor history")
            .expect("predecessor history exists");
        let successor_history = store
            .inspect_history(&successor, epoch)
            .expect("replay successor history")
            .expect("successor history exists");
        assert!(predecessor_history.damage.is_none());
        assert!(successor_history.damage.is_none());
        assert_eq!(predecessor_history.version.phase(), WalkPhase::R13b);
        assert_eq!(predecessor_history.version.journal_revision(), 61);
        assert_eq!(successor_history.version.phase(), WalkPhase::R4c);
        assert_eq!(successor_history.version.journal_revision(), 7);

        let predecessor_snapshot = store
            .inspect(&predecessor)
            .expect("inspect predecessor session")
            .expect("predecessor session exists");
        let committed = predecessor_snapshot
            .committed_handoff(attempt.session(), attempt.fence())
            .expect("production replay identifies the exact committed handoff");
        assert_eq!(committed.intent().transition_id(), attempt.transition());
        assert!(matches!(
            committed.result(),
            crate::cli::prototype1_state::session::AttemptResult::Committed {
                phase: WalkPhase::R13b,
                ..
            }
        ));

        let finished = predecessor_history
            .events
            .iter()
            .find(|event| {
                matches!(
                    &event.kind,
                    WalkSessionEventKind::AttemptFinished { receipt }
                        if receipt.transition_id == attempt.transition()
                            && receipt.fence == attempt.fence().get()
                            && matches!(
                                receipt.result,
                                WalkAttemptResult::Committed {
                                    phase: WalkPhase::R13b,
                                    ..
                                }
                            )
                )
            })
            .expect("replay committed predecessor handoff receipt");
        let predecessor_release = predecessor_history
            .events
            .iter()
            .find(|event| {
                matches!(
                    event.kind,
                    WalkSessionEventKind::Released {
                        fence: 18,
                        ready: None
                    }
                )
            })
            .expect("replay clean predecessor release");
        let successor_ready = successor_history
            .events
            .iter()
            .find_map(|event| match &event.kind {
                WalkSessionEventKind::Released {
                    fence: 1,
                    ready: Some(ready),
                } => Some((event, ready)),
                _ => None,
            })
            .expect("replay atomic successor Ready release");
        assert_eq!(
            successor_ready.1.runtime_id.to_string(),
            invocation.runtime_id().to_string()
        );
        assert_eq!(successor_ready.1.commit.cursor.phase, WalkPhase::R4c);
        assert!(successor_ready.0.recorded_at_ms < finished.recorded_at_ms);
        assert!(finished.recorded_at_ms < predecessor_release.recorded_at_ms);

        let transition_path = temp.path().join("transition-journal.jsonl");
        let transition_bytes = inflate_hex(&fixture.join("transition-journal.jsonl.gz.hex"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&transition_bytes)),
            "8e2bf04f1506f2fe400892692b1b9a212bf1c6edcb0e98b230a12ed9c7d639d0"
        );
        fs::write(&transition_path, transition_bytes)
            .expect("install exact historical transition journal");
        let entries = PrototypeJournal::new(&transition_path)
            .load_entries()
            .expect("replay transition journal");
        let journal_ready = entries
            .iter()
            .find_map(|entry| match entry {
                JournalEntry::Successor(record)
                    if record.runtime_id == Some(invocation.runtime_id()) =>
                {
                    match &record.state {
                        SuccessorState::Ready {
                            controller: Some(ready),
                            ..
                        } => Some(ready),
                        _ => None,
                    }
                }
                _ => None,
            })
            .expect("replay typed successor Ready projection");
        journal_ready
            .validate_persisted()
            .expect("historical Ready receipt remains structurally valid");
        assert_eq!(
            journal_ready.commit().session_id(),
            successor_ready.1.commit.session_id
        );
        let acceptance = entries
            .iter()
            .find_map(|entry| match entry {
                JournalEntry::SuccessorHandoff(handoff)
                    if handoff.runtime_id == invocation.runtime_id() =>
                {
                    handoff.acceptance.as_ref()
                }
                _ => None,
            })
            .expect("replay typed successor handoff acceptance");
        assert_eq!(acceptance.ready(), journal_ready);
        assert_eq!(acceptance.attempt(), attempt);

        let operation_bytes = decode_hex(&fixture.join("predecessor-operation.json.hex"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&operation_bytes)),
            "d0d473aedf274fba24e371bf77234a7d082f71ca42efd66a806bf6a37a63d5e8"
        );
        let operation: DurableOperation = serde_json::from_slice(&operation_bytes)
            .expect("decode historical predecessor operation");
        let snapshot = &operation.stored.snapshot;
        assert_eq!(
            snapshot.operation_id.to_string(),
            "ee7bae91-f6a9-4e49-8121-27b6ac0aef5c"
        );
        assert_eq!(snapshot.job_id, 8);
        assert_eq!(snapshot.status, WalkJobStatus::Indeterminate);
        assert_eq!(snapshot.expected.session_id(), Some(attempt.session()));
        assert_eq!(snapshot.expected.phase(), WalkPhase::R12);
        assert_eq!(snapshot.expected.journal_revision(), 57);
        assert_eq!(snapshot.phase_before, WalkPhase::R12);
        assert_eq!(snapshot.phase_after, Some(WalkPhase::R13b));
        assert_eq!(snapshot.target_phase, Some(WalkPhase::R13b));
        assert!(snapshot.receipt.is_none());
        let message = snapshot
            .message
            .as_deref()
            .expect("historical operation preserves failure detail");
        assert!(message.contains("successor executable"), "{message}");
        assert!(
            message.contains("/home/brasides/code/ploke/target/debug/ploke-eval"),
            "{message}"
        );
        assert!(
            message.contains(
                "/home/brasides/.ploke-eval/setup-seeds/p1-v12-r12fix-keeponly-g35f-oropenai-3g1x3-p3-20260716-204252/target/debug/ploke-eval"
            ),
            "{message}"
        );
        assert!(
            message.contains("durable controller cursor changed"),
            "{message}"
        );
        let operation_finished = chrono::DateTime::parse_from_rfc3339(
            snapshot
                .finished_at
                .as_deref()
                .expect("operation finish time"),
        )
        .expect("parse operation finish time")
        .timestamp_millis();
        assert!(operation_finished > predecessor_release.recorded_at_ms);
    }

    #[tokio::test]
    async fn handoff_terminal_fences_predecessor_admission() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let handoff = server
            .register_job(
                test_guard(10_010),
                b"handoff-step".to_vec(),
                test_step_intent(WalkPhase::R13b),
            )
            .await
            .map(accepted)
            .expect("admit predecessor handoff job");
        finish_job(
            &server.jobs,
            &server.operation_root,
            &server.epoch,
            handoff.job_id,
            WalkJobStatus::Succeeded,
            Some(WalkPhase::R13b),
            "handoff committed".to_string(),
            Some(WalkTransitionReceipt {
                phase_before: WalkPhase::R12,
                phase_after: WalkPhase::R13b,
                edges: vec![ControlEdge::R12ToR13b],
                version: test_version(WalkPhase::R13b, 10_010),
                event_projection: WalkEventProjection::Recorded,
            }),
        )
        .await;

        let admission = server
            .register_job(
                test_guard(10_011),
                b"stale-predecessor-step".to_vec(),
                test_step_intent(WalkPhase::R14b),
            )
            .await
            .expect("post-transfer admission is a typed response");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error { code, .. }) => {
                assert_eq!(code, WalkErrorCode::ServerStopping);
            }
            JobAdmission::Accepted(_) | JobAdmission::Duplicate(_) => {
                panic!("a transferred predecessor admitted another mutation")
            }
            JobAdmission::Rejected(other) => panic!("wrong fencing response: {other:?}"),
        }
        assert!(server.jobs.lock().await.stopping);
    }

    #[tokio::test]
    async fn successor_retirement_cleans_stale_predecessor_socket() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("stale-predecessor.sock");
        let listener = UnixListener::bind(&socket).expect("bind predecessor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket)
            .expect("capture predecessor endpoint");
        drop(listener);

        assert!(
            endpoint.owns_socket(),
            "the stale inode must still be owned"
        );
        assert!(
            !socket_reachable(endpoint.socket()).expect("probe stale predecessor"),
            "a dropped listener must not remain reachable"
        );
        retire_predecessor(&endpoint)
            .await
            .expect("retire exact stale predecessor");
        assert!(!endpoint.owns_socket());
    }

    #[tokio::test]
    async fn successor_retirement_preserves_rebound_socket() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("rebound-predecessor.sock");
        let listener = UnixListener::bind(&socket).expect("bind predecessor socket");
        let endpoint = ServerEndpoint::from_bound(repo.path().to_path_buf(), socket.clone())
            .expect("capture predecessor endpoint");
        drop(listener);
        fs::remove_file(&socket).expect("unlink stale predecessor socket");
        let replacement = UnixListener::bind(&socket).expect("bind replacement socket");

        assert!(!endpoint.owns_socket(), "replacement must have a new inode");
        let error = retire_predecessor(&endpoint)
            .await
            .expect_err("retirement must reject replacement ownership");
        assert!(error.to_string().contains("changed ownership"), "{error}");
        assert!(
            socket_reachable(&socket).expect("probe replacement socket"),
            "replacement listener must remain reachable"
        );
        drop(replacement);
        fs::remove_file(&socket).expect("remove replacement socket");
    }

    #[tokio::test]
    async fn socket_status_requires_the_current_protocol() {
        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("walk.sock");
        let listener = UnixListener::bind(&socket).expect("bind walk socket");
        let server = test_server(repo.path(), MutationGate::open());
        let serving = tokio::spawn(accept_loop(server, listener, None));

        let missing = request_over_socket(&socket, WalkRequestBody::Health)
            .await
            .expect("missing-epoch response");
        assert!(matches!(
            missing,
            WalkResponse::Error {
                code: WalkErrorCode::BadRequest,
                ref detail,
                ..
            } if detail.contains(&format!("client=missing server={WALK_PROTOCOL_VERSION}"))
        ));

        let missing = request_over_socket(&socket, WalkRequestBody::LlmTraceIndex)
            .await
            .expect("missing-protocol typed LLM response");
        assert!(matches!(
            missing,
            WalkResponse::Error {
                code: WalkErrorCode::BadRequest,
                ref detail,
                ..
            } if detail.contains(&format!("client=missing server={WALK_PROTOCOL_VERSION}"))
        ));

        let mut old_epoch = ServerEpoch::capture(repo.path()).expect("capture old client epoch");
        old_epoch.protocol_version -= 1;
        let mut stream = ipc::connect(&socket).await.expect("connect old client");
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_protocol: Some(old_epoch.protocol_version),
                client_epoch: Some(old_epoch),
                body: WalkRequestBody::Show,
            },
        )
        .await
        .expect("send old client request");
        let outdated: WalkResponse = ipc::recv(&mut stream)
            .await
            .expect("outdated-protocol response");
        assert!(matches!(
            outdated,
            WalkResponse::Error {
                code: WalkErrorCode::BadRequest,
                ref detail,
                ..
            } if detail.contains(&format!(
                "client={} server={WALK_PROTOCOL_VERSION}",
                WALK_PROTOCOL_VERSION - 1
            ))
        ));

        let current = health_over_socket(&socket)
            .await
            .expect("current client status response");
        serving.abort();
        let _ = serving.await;

        assert!(matches!(current, WalkResponse::Status { .. }));
    }

    #[tokio::test]
    async fn partial_frame_client_does_not_block_later_health() {
        use tokio::io::AsyncWriteExt as _;

        let repo = tempdir().expect("repo tempdir");
        let socket = repo.path().join("walk.sock");
        let listener = UnixListener::bind(&socket).expect("bind walk socket");
        let server = test_server(repo.path(), MutationGate::open());
        let serving = tokio::spawn(accept_loop(server, listener, None));

        let mut stalled = ipc::connect(&socket).await.expect("connect stalled client");
        let frame_len = 16_u32.to_le_bytes();
        stalled
            .write_all(&frame_len[..2])
            .await
            .expect("send partial frame length");
        stalled.flush().await.expect("flush partial frame");

        let later_health =
            tokio::time::timeout(Duration::from_millis(250), health_over_socket(&socket)).await;
        drop(stalled);
        serving.abort();
        let _ = serving.await;

        let response = later_health
            .expect("partial frame must not head-of-line block a later health request")
            .expect("later health request must succeed");
        assert!(matches!(response, WalkResponse::Status { .. }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_clients_query_same_rows_and_revision() {
        let root = tempdir().expect("temp root");
        let eval_home = root.path().join("eval-home");
        let _env = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let campaign = CampaignId::from("walk-query-clients");
        let repo = root.path().join("parent");
        fs::create_dir_all(&repo).expect("create parent repo");
        let socket = root.path().join("walk-query.sock");
        let listener = UnixListener::bind(&socket).expect("bind walk socket");
        let server = test_server(&repo, MutationGate::open());
        let db_path = owner_db_path(campaign.as_str()).expect("owner DB path");
        publish_walk_event(
            &db_path,
            &server.epoch,
            campaign.as_str(),
            "first",
            "2026-07-13T00:00:00Z",
        );
        let serving = tokio::spawn(accept_loop(server.clone(), listener, None));
        let body = || WalkRequestBody::DbQuery {
            campaign: Some(campaign.clone()),
            script: "?[command] := *eval_walk_event { command }\n:order command".to_string(),
        };

        let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                request_over_socket(&socket, body()),
                request_over_socket(&socket, body())
            )
        })
        .await
        .expect("concurrent client queries must complete");
        let first = first.expect("first client query");
        let second = second.expect("second client query");
        let (first_result, second_result) = match (first, second) {
            (WalkResponse::Query { query: first }, WalkResponse::Query { query: second }) => {
                (first.result, second.result)
            }
            responses => panic!("expected two query responses, got {responses:?}"),
        };
        assert_eq!(first_result.revision, second_result.revision);
        assert_eq!(first_result.headers, second_result.headers);
        assert_eq!(first_result.row_count, second_result.row_count);
        assert_eq!(first_result.rows.len(), second_result.rows.len());
        for (first, second) in first_result.rows.iter().zip(&second_result.rows) {
            assert_eq!(first.cells, second.cells);
            assert_eq!(first.object, second.object);
        }

        publish_walk_event(
            &db_path,
            &server.epoch,
            campaign.as_str(),
            "second",
            "2026-07-13T00:00:01Z",
        );
        let changed = request_over_socket(&socket, body())
            .await
            .expect("query after owner publish");
        let WalkResponse::Query { query: changed } = changed else {
            panic!("expected changed query response");
        };
        assert_ne!(first_result.revision, changed.result.revision);
        assert_eq!(changed.result.headers, ["command"]);
        assert_eq!(changed.result.row_count, 2);
        assert_eq!(
            changed
                .result
                .rows
                .iter()
                .map(|row| row.object.clone())
                .collect::<Vec<_>>(),
            [
                serde_json::json!({"command": "first"}),
                serde_json::json!({"command": "second"}),
            ]
        );

        serving.abort();
        let _ = serving.await;
    }

    fn publish_walk_event(
        path: &Path,
        epoch: &ServerEpoch,
        campaign: &str,
        command: &str,
        recorded_at: &str,
    ) {
        crate::cli::prototype1_state::eval_store::write_walk_event_to_owner_db(
            path,
            crate::cli::prototype1_state::eval_store::WalkEventEvidence {
                campaign_id: campaign.to_string(),
                node_id: "node-parent".to_string(),
                parent_id: "node-parent".to_string(),
                generation: 0,
                branch_id: epoch
                    .active_branch
                    .clone()
                    .unwrap_or_else(|| "detached".to_string()),
                command: command.to_string(),
                status: "ok".to_string(),
                phase_before: Some("empty".to_string()),
                phase_after: "empty".to_string(),
                target_phase: None,
                watch: None,
                allow_live_api: None,
                allow_git_changes: None,
                transitions: Vec::new(),
                protocol_version: epoch.protocol_version,
                transition_graph_version: epoch.transition_graph_version.clone(),
                repo_root: epoch.repo_root.display().to_string(),
                exe_path: epoch.exe_path.display().to_string(),
                exe_modified_unix_ms: epoch.exe_modified_unix_ms,
                git_head: epoch.git_head.clone(),
                source_status_hash: epoch.source_status_hash.clone(),
                recorded_at: recorded_at.to_string(),
            },
        )
        .expect("publish walk event through owner writer");
    }

    #[tokio::test]
    async fn task_panic_releases_recovery_admission() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(10_002),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
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
                if snapshot.status == WalkJobStatus::Indeterminate {
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
        assert_eq!(
            server
                .load_operation(failed.operation_id)
                .expect("load panicked operation")
                .expect("panicked operation receipt")
                .stored
                .snapshot
                .status,
            WalkJobStatus::Indeterminate
        );

        let next = server
            .register_job(
                test_guard(10_003),
                b"next-step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .expect("classify follow-up after panic");
        assert!(
            matches!(
                next,
                JobAdmission::Rejected(WalkResponse::Error {
                    code: WalkErrorCode::RecoveryInProgress,
                    ..
                })
            ),
            "an indeterminate task panic must block ordinary follow-up work"
        );

        let resolution = server
            .submit_resolve_job(
                Some(&server.epoch),
                test_guard(10_002),
                WalkJobResolutionKind::Abandon,
            )
            .await
            .expect("abandon indeterminate panic");
        let WalkResponse::Job { job: abandoned, .. } = resolution else {
            panic!("expected abandoned job response");
        };
        assert_eq!(abandoned.status, WalkJobStatus::Abandoned);
        assert!(
            !abandoned.status.blocks_mutation(),
            "durable abandonment must release the indeterminate job blocker"
        );
        assert_eq!(
            abandoned.resolution.as_ref().map(|receipt| receipt.kind),
            Some(WalkJobResolutionKind::Abandon)
        );
        assert_eq!(
            server
                .load_operation(abandoned.operation_id)
                .expect("load abandoned operation")
                .expect("abandoned operation receipt")
                .stored
                .snapshot
                .status,
            WalkJobStatus::Abandoned
        );

        let recovery = server
            .register_job(
                test_guard(10_004),
                b"recover".to_vec(),
                test_intent(WalkJobKind::Recover),
            )
            .await
            .expect("recovery admission response");
        assert!(
            matches!(
                &recovery,
                JobAdmission::Rejected(WalkResponse::Error {
                    code: WalkErrorCode::BadRequest,
                    detail,
                    ..
                }) if detail.contains("committed session cursor")
            ),
            "job abandonment must not invent controller recovery authority"
        );
        assert!(
            server
                .load_operation(OperationId::for_test(10_004))
                .expect("inspect rejected recovery")
                .is_none(),
            "cursorless recovery rejection must not persist an operation"
        );
    }

    #[tokio::test]
    async fn restart_blocks_new_work_after_running_job() {
        let repo = tempdir().expect("repo tempdir");
        let first = test_server(repo.path(), MutationGate::open());
        first
            .register_job(
                test_guard(10_010),
                b"llm-finish".to_vec(),
                test_intent(WalkJobKind::LlmFinish),
            )
            .await
            .map(accepted)
            .expect("admit nested live job");
        drop(first);

        let restarted = test_server(repo.path(), MutationGate::open());
        let admission = restarted
            .register_job(
                test_guard(10_011),
                b"next-step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .expect("classify follow-up after restart");
        assert!(
            matches!(
                admission,
                JobAdmission::Rejected(WalkResponse::Error {
                    code: WalkErrorCode::RecoveryInProgress,
                    ..
                })
            ),
            "a durable running job must be reconstructed as a recovery blocker"
        );

        let active = restarted.latest_job().await.expect("restored job");
        assert_eq!(active.status, WalkJobStatus::Indeterminate);
        let resolution = restarted
            .submit_resolve_job(
                Some(&restarted.epoch),
                test_guard(10_010),
                WalkJobResolutionKind::Abandon,
            )
            .await
            .expect("abandon restored job");
        assert!(matches!(
            resolution,
            WalkResponse::Job {
                job: WalkJobSnapshot {
                    status: WalkJobStatus::Abandoned,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            restarted
                .register_job(
                    test_guard(10_012),
                    b"post-abandon-step".to_vec(),
                    test_step_intent(WalkPhase::R6),
                )
                .await
                .expect("admit after durable abandonment"),
            JobAdmission::Accepted(_)
        ));
    }

    #[tokio::test]
    async fn successor_restore_keeps_predecessor_job_non_authoritative() {
        let repo = tempdir().expect("repo tempdir");
        let predecessor = test_server(repo.path(), MutationGate::open());
        let template = predecessor
            .register_job(
                test_guard(10_020),
                b"template-step".to_vec(),
                test_step_intent(WalkPhase::R13b),
            )
            .await
            .map(accepted)
            .expect("admit operation template");
        finish_job(
            &predecessor.jobs,
            &predecessor.operation_root,
            &predecessor.epoch,
            template.job_id,
            WalkJobStatus::Succeeded,
            Some(WalkPhase::R13b),
            "template completed".to_string(),
            None,
        )
        .await;
        let operation = OperationId::for_test(10_021);
        let stored = StoredOperation {
            snapshot: WalkJobSnapshot {
                job_id: template.job_id + 1,
                operation_id: operation,
                expected: test_version(WalkPhase::R12, 1),
                ..template.clone()
            },
            fingerprint: b"predecessor-step".to_vec(),
        };
        assert!(
            predecessor
                .persist_operation(&stored)
                .expect("persist predecessor operation")
                .is_none(),
            "predecessor operation must be newly persisted"
        );

        let epoch = ServerEpoch::capture(repo.path()).expect("capture successor epoch");
        let operation_root = repo.path().join("walk-operations");
        let jobs = restore_job_registry(
            &operation_root,
            &epoch,
            JobRestoreScope::Successor {
                predecessor: SessionId::for_test(1),
            },
            Some(SessionId::for_test(2)),
        )
        .expect("restore successor session jobs");

        assert!(
            jobs.active.is_none(),
            "a predecessor-session job cannot become the successor recovery blocker"
        );
        assert_eq!(
            jobs.completed
                .get(&operation)
                .expect("foreign operation remains inspectable")
                .snapshot
                .status,
            WalkJobStatus::Running,
            "successor inspection must not rewrite predecessor evidence"
        );

        let mut terminal = stored.clone();
        terminal.snapshot.status = WalkJobStatus::Succeeded;
        terminal.snapshot.phase_after = Some(WalkPhase::R13b);
        terminal.snapshot.updated_at = now_rfc3339();
        terminal.snapshot.finished_at = Some(now_rfc3339());
        terminal.snapshot.message = Some("predecessor receipt published".to_string());
        persist_terminal_operation(&operation_root, &epoch, &terminal)
            .expect("publish predecessor terminal receipt");
        let successor = test_server(repo.path(), MutationGate::closed());
        *successor.jobs.lock().await = jobs;
        let replay = successor
            .register_job(
                MutationGuard {
                    operation,
                    expected: stored.snapshot.expected.clone(),
                },
                stored.fingerprint.clone(),
                test_step_intent(WalkPhase::R13b),
            )
            .await
            .expect("retry predecessor operation through successor");
        let JobAdmission::Duplicate(replayed) = replay else {
            panic!("terminal predecessor retry must replay its durable winner");
        };
        assert_eq!(replayed.status, WalkJobStatus::Succeeded);
        drop(successor);

        let unrelated = OperationId::for_test(10_022);
        let stored = StoredOperation {
            snapshot: WalkJobSnapshot {
                job_id: template.job_id + 2,
                operation_id: unrelated,
                expected: test_version(WalkPhase::R12, 3),
                ..template
            },
            fingerprint: b"unrelated-step".to_vec(),
        };
        assert!(
            predecessor
                .persist_operation(&stored)
                .expect("persist unrelated operation")
                .is_none(),
            "unrelated operation must be newly persisted"
        );
        drop(predecessor);

        let error = match restore_job_registry(
            &operation_root,
            &epoch,
            JobRestoreScope::Successor {
                predecessor: SessionId::for_test(1),
            },
            Some(SessionId::for_test(2)),
        ) {
            Ok(_) => panic!("unrelated unresolved operation must fail closed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(&unrelated.to_string()),
            "{error}"
        );
        assert!(error.to_string().contains("unrelated controller session"));
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
                test_guard(11),
                b"step".to_vec(),
                test_step_intent(WalkPhase::R6),
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
                client_protocol: None,
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
            WalkResponse::Error { code, detail, .. } => {
                assert_eq!(code, WalkErrorCode::JobActive);
                assert!(detail.contains("stop refused while job"), "{detail}")
            }
            other => panic!("active-job stop did not fail closed: {other:?}"),
        }
        let admission = server
            .register_job(
                test_guard(12),
                b"next-step".to_vec(),
                test_step_intent(WalkPhase::R6),
            )
            .await
            .expect("stopping rejection is a typed response");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error { code, detail, .. }) => {
                assert_eq!(code, WalkErrorCode::JobActive);
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
                client_protocol: None,
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
                client_protocol: None,
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
            (WalkResponse::Status { snapshot, .. }, false) => {
                assert_eq!(snapshot.version(), SessionVersion::empty());
                assert_eq!(snapshot.phase(), WalkPhase::Empty);
                assert!(matches!(snapshot.position, WalkPosition::NoSession));
            }
            other => panic!("health did not expose a durable version: {other:?}"),
        }
    }

    #[tokio::test]
    async fn health_exposes_missing_setup_blocker_and_actions() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let (open, _) = server.handle(walk_request(WalkRequestBody::Health)).await;
        let WalkResponse::Status { snapshot, .. } = open else {
            panic!("health did not return a session snapshot");
        };
        assert!(matches!(&snapshot.position, WalkPosition::NoSession));
        assert_eq!(snapshot.authority, WalkAuthority::RecoveryRequired);
        assert_eq!(
            snapshot.blocker.as_ref().map(|blocker| blocker.code),
            Some(WalkBlockerCode::ControllerBlocked)
        );
        assert!(snapshot.actions.iter().any(|action| {
            action.kind == WalkActionKind::Start
                && action.edge.is_none()
                && action.target == Some(WalkPhase::R3)
                && !action.enabled
                && action.blocker == Some(WalkBlockerCode::ControllerBlocked)
        }));

        let pending = test_server(repo.path(), MutationGate::closed());
        let (closed, _) = pending.handle(walk_request(WalkRequestBody::Health)).await;
        let WalkResponse::Status { snapshot, .. } = closed else {
            panic!("pending health did not return a session snapshot");
        };
        assert_eq!(snapshot.authority, WalkAuthority::TransferPending);
        assert_eq!(
            snapshot.blocker.as_ref().map(|blocker| blocker.code),
            Some(WalkBlockerCode::TransferPending)
        );
        assert!(snapshot.actions.iter().any(|action| {
            action.kind == WalkActionKind::Start
                && !action.enabled
                && action.blocker == Some(WalkBlockerCode::TransferPending)
        }));
        assert!(
            snapshot
                .actions
                .iter()
                .any(|action| { action.kind == WalkActionKind::Inspect && action.enabled })
        );
    }

    #[tokio::test]
    async fn operation_retry_is_idempotent_and_payload_reuse_conflicts() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let guard = test_guard(20);
        let first = accepted(
            server
                .register_job(
                    guard.clone(),
                    b"step-a".to_vec(),
                    test_intent(WalkJobKind::Step),
                )
                .await
                .expect("accept first operation"),
        );

        let duplicate = server
            .register_job(
                guard.clone(),
                b"step-a".to_vec(),
                test_intent(WalkJobKind::Step),
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
            .register_job(guard, b"step-b".to_vec(), test_intent(WalkJobKind::Step))
            .await
            .expect("classify operation conflict");
        match conflict {
            JobAdmission::Rejected(WalkResponse::Error { code, .. }) => {
                assert_eq!(code, WalkErrorCode::OperationConflict)
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
                    guard.clone(),
                    b"step-a".to_vec(),
                    test_intent(WalkJobKind::Step),
                )
                .await
                .expect("accept first-server operation"),
        );
        drop(first);

        let restarted = test_server(repo.path(), MutationGate::open());
        let admission = restarted
            .register_job(guard, b"step-a".to_vec(), test_intent(WalkJobKind::Step))
            .await
            .expect("classify restarted operation");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error { code, detail, .. }) => {
                assert_eq!(code, WalkErrorCode::OperationRestart);
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
    async fn terminal_operation_replays_after_server_restart() {
        let repo = tempdir().expect("repo tempdir");
        let guard = test_guard(22);
        let first = test_server(repo.path(), MutationGate::open());
        let job = first
            .register_job(
                guard.clone(),
                b"step-a".to_vec(),
                test_intent(WalkJobKind::Step),
            )
            .await
            .map(accepted)
            .expect("accept first-server operation");
        finish_job(
            &first.jobs,
            &first.operation_root,
            &first.epoch,
            job.job_id,
            WalkJobStatus::Succeeded,
            Some(WalkPhase::R6),
            "durable completion".to_string(),
            None,
        )
        .await;
        drop(first);

        let restarted = test_server(repo.path(), MutationGate::open());
        let mut retry = guard;
        retry.expected.journal_revision += 1;
        let replay = restarted
            .register_job(retry, b"step-a".to_vec(), test_intent(WalkJobKind::Step))
            .await
            .expect("replay terminal operation");
        match replay {
            JobAdmission::Duplicate(job) => {
                assert_eq!(job.status, WalkJobStatus::Succeeded);
                assert_eq!(job.phase_after, Some(WalkPhase::R6));
                assert_eq!(job.message.as_deref(), Some("durable completion"));
            }
            JobAdmission::Accepted(_) | JobAdmission::Rejected(_) => {
                panic!("terminal durable receipt was not replayed")
            }
        }
    }

    #[tokio::test]
    async fn operation_status_retains_terminal_failure_across_new_jobs_and_restart() {
        let repo = tempdir().expect("repo tempdir");
        let first = test_server(repo.path(), MutationGate::open());
        let failed = first
            .register_job(
                test_guard(22_001),
                b"failed-step".to_vec(),
                test_intent(WalkJobKind::Step),
            )
            .await
            .map(accepted)
            .expect("accept failed operation");
        finish_job(
            &first.jobs,
            &first.operation_root,
            &first.epoch,
            failed.job_id,
            WalkJobStatus::Failed,
            Some(WalkPhase::R6),
            "durable failure".to_string(),
            None,
        )
        .await;
        first
            .register_job(
                test_guard(22_002),
                b"next-step".to_vec(),
                test_intent(WalkJobKind::Step),
            )
            .await
            .map(accepted)
            .expect("accept next operation");

        let before_restart = first
            .lookup_operation(failed.operation_id)
            .await
            .expect("lookup prior operation")
            .expect("prior operation snapshot");
        assert_eq!(before_restart.status, WalkJobStatus::Failed);
        assert_eq!(before_restart.message.as_deref(), Some("durable failure"));
        drop(first);

        let restarted = test_server(repo.path(), MutationGate::open());
        let after_restart = restarted
            .handle(walk_request(WalkRequestBody::OperationStatus {
                operation: failed.operation_id,
            }))
            .await;
        assert!(matches!(
            after_restart,
            (
                WalkResponse::Job {
                    job: WalkJobSnapshot {
                        status: WalkJobStatus::Failed,
                        ..
                    },
                    ..
                },
                false
            )
        ));
    }

    #[tokio::test]
    async fn terminal_publication_preserves_admission_epoch_and_rejects_overwrite() {
        let repo = tempdir().expect("repo tempdir");
        let server = test_server(repo.path(), MutationGate::open());
        let job = server
            .register_job(
                test_guard(23),
                b"step-a".to_vec(),
                test_intent(WalkJobKind::Step),
            )
            .await
            .map(accepted)
            .expect("accept operation");
        let fingerprint = server
            .jobs
            .lock()
            .await
            .active
            .as_ref()
            .expect("active operation")
            .fingerprint
            .clone();
        let admitted_epoch = server
            .load_operation(job.operation_id)
            .expect("load admitted operation")
            .expect("admitted operation record")
            .epoch;
        let mut resolver_epoch = server.epoch.clone();
        resolver_epoch.git_head = Some("resolver-source-head".to_string());

        let mut abandoned = job.clone();
        abandoned.status = WalkJobStatus::Abandoned;
        abandoned.finished_at = Some(now_rfc3339());
        let abandoned = StoredOperation {
            snapshot: abandoned,
            fingerprint: fingerprint.clone(),
        };
        persist_terminal_operation(&server.operation_root, &resolver_epoch, &abandoned)
            .expect("publish abandonment");

        let published = server
            .load_operation(job.operation_id)
            .expect("load abandoned operation")
            .expect("abandoned operation record");
        assert_eq!(published.epoch, admitted_epoch);
        assert_eq!(published.stored.snapshot.status, WalkJobStatus::Abandoned);

        let mut succeeded = job;
        succeeded.status = WalkJobStatus::Succeeded;
        succeeded.finished_at = Some(now_rfc3339());
        let error = persist_terminal_operation(
            &server.operation_root,
            &server.epoch,
            &StoredOperation {
                snapshot: succeeded,
                fingerprint,
            },
        )
        .expect_err("late completion must not overwrite abandonment");
        assert!(error.to_string().contains("cannot transition"), "{error}");
        assert_eq!(
            server
                .load_operation(abandoned.snapshot.operation_id)
                .expect("reload operation")
                .expect("terminal operation record")
                .stored
                .snapshot
                .status,
            WalkJobStatus::Abandoned
        );
    }

    #[test]
    fn durable_controller_blocker_only_allows_recovery_job() {
        let durable = DurableSessionState {
            version: test_version(WalkPhase::R6, 61),
            blocker: Some(WalkBlocker {
                code: WalkBlockerCode::AttemptPending,
                detail: "pending transition".to_string(),
            }),
        };

        assert!(admission_blocker(WalkJobKind::Step, &durable).is_some());
        assert!(admission_blocker(WalkJobKind::LlmStep, &durable).is_some());
        assert!(admission_blocker(WalkJobKind::Recover, &durable).is_none());
    }

    #[test]
    fn controller_blocker_disables_mutating_actions() {
        let blocker = WalkBlocker {
            code: WalkBlockerCode::ControllerBlocked,
            detail: "reconstruction failed".to_string(),
        };
        let authority = authority_for(Some(&blocker));
        let position = WalkPosition::Session {
            version: test_version(WalkPhase::R6, 62),
        };
        let actions = actions_for(&position, true, authority, Some(&blocker));

        assert_eq!(authority, WalkAuthority::RecoveryRequired);
        assert!(
            actions
                .iter()
                .any(|action| { action.kind == WalkActionKind::Inspect && action.enabled })
        );
        assert!(
            actions
                .iter()
                .filter(|action| {
                    matches!(action.kind, WalkActionKind::Step | WalkActionKind::Reset)
                })
                .all(|action| {
                    !action.enabled && action.blocker == Some(WalkBlockerCode::ControllerBlocked)
                })
        );
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
            .register_job(guard, b"start".to_vec(), test_intent(WalkJobKind::Start))
            .await
            .expect("classify stale version");
        match admission {
            JobAdmission::Rejected(WalkResponse::Error {
                code,
                version: Some(actual),
                ..
            }) => {
                assert_eq!(code, WalkErrorCode::StaleVersion);
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
        let predecessor = SessionId::for_test(1);
        let first =
            prepare_successor(repo.path(), first_id, predecessor).expect("prepare first successor");
        let next =
            prepare_successor(repo.path(), next_id, predecessor).expect("prepare next successor");

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
        let predecessor = SessionId::for_test(1);
        let live = prepare_successor(repo.path(), runtime_id, predecessor)
            .expect("prepare live successor");
        let stale = live.endpoint().clone();

        let error = match prepare_successor(repo.path(), runtime_id, predecessor) {
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
        let rebound = prepare_successor(repo.path(), runtime_id, predecessor)
            .expect("rebind stale successor socket");
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
