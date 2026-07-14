//! Public read-only client helpers for the Prototype 1 walk server.
//!
//! This is a UI-facing facade over the private `loop walk` protocol. It does
//! not own transition semantics and it does not make database mirrors
//! authoritative.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

use crate::{
    campaign::campaign_manifest_path,
    cli::prototype1_state::{
        eval_store::{load_owner_eval_database, prototype1_eval_store_db_path},
        identity,
        walk::{
            ipc, paths,
            protocol::{WalkRequest, WalkRequestBody, WalkResponse},
        },
    },
    layout::{campaigns_dir, ploke_eval_home},
    spec::PrepareError,
};

pub use crate::cli::prototype1_state::walk::phase::WalkPhase;

/// Resolved local walk service endpoint for one parent checkout.
#[derive(Debug, Clone)]
pub struct WalkClient {
    repo_root: PathBuf,
    socket: PathBuf,
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
    pub edge: String,
    pub phase: WalkPhase,
    pub phase_id: String,
    pub detail: String,
    pub requires_watch: bool,
    pub requires_live_api: bool,
    pub requires_git_changes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkReplyStatus {
    Ok,
    Audit,
    Error,
}

/// UI-safe projection of one walk server response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkSnapshot {
    pub status: WalkReplyStatus,
    pub phase: Option<WalkPhase>,
    pub phase_id: Option<String>,
    pub phase_label: Option<String>,
    pub message: String,
    pub error_code: Option<String>,
    pub epoch: ServerEpochView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEpochView {
    pub protocol_version: u32,
    pub transition_graph_version: String,
    pub repo_root: PathBuf,
    pub exe_path: PathBuf,
    pub exe_modified_unix_ms: Option<u64>,
    pub git_head: Option<String>,
    pub source_status_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryResult {
    pub repo_root: PathBuf,
    pub campaign_id: String,
    pub db_path: PathBuf,
    pub script: String,
    pub headers: Vec<String>,
    pub row_count: usize,
    pub rows: Vec<DbQueryRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryRow {
    pub cells: Vec<serde_json::Value>,
    pub object: serde_json::Value,
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
        let socket = paths::socket_path(&repo_root, socket_override)?;
        Ok(Self { repo_root, socket })
    }

    /// Resolve a client endpoint using the same context/socket rules as
    /// `ploke-eval loop walk`.
    pub fn resolve(repo_root: Option<&Path>, socket: Option<&Path>) -> Result<Self, PrepareError> {
        let repo_root = paths::resolve_repo_root(repo_root)?;
        let socket = paths::socket_path(&repo_root, socket)?;
        Ok(Self { repo_root, socket })
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Probe the server without mutating state. `Ok(None)` means no server is
    /// currently listening at the resolved socket.
    pub async fn health(&self) -> Result<Option<WalkSnapshot>, PrepareError> {
        let mut stream = match UnixStream::connect(&self.socket).await {
            Ok(stream) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                let _ = paths::remove_socket_file(&self.socket);
                return Ok(None);
            }
            Err(source) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "prototype1_state_walk_health_connect",
                    detail: format!(
                        "failed to probe walk socket '{}': {source}",
                        self.socket.display()
                    ),
                });
            }
        };
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_epoch: None,
                body: WalkRequestBody::Health,
            },
        )
        .await?;
        let response = ipc::recv(&mut stream).await?;
        Ok(Some(WalkSnapshot::from_response(response)))
    }

    /// Read the current in-memory walk state from the server.
    pub async fn show(&self) -> Result<WalkSnapshot, PrepareError> {
        self.send_read_only(WalkRequestBody::Show).await
    }

    /// Run an immutable CozoScript query against the owner eval DB snapshot.
    pub fn query_db(
        &self,
        campaign: Option<&str>,
        script: &str,
    ) -> Result<DbQueryResult, PrepareError> {
        let campaign_id = self.resolve_campaign(campaign)?;
        query_campaign_db_at(self.repo_root.clone(), campaign_id, script)
    }

    /// Run an immutable CozoScript query for a campaign without requiring a
    /// live walk server or parent checkout.
    pub fn query_campaign_db(campaign: &str, script: &str) -> Result<DbQueryResult, PrepareError> {
        let campaign_id = CampaignId::from(campaign.trim());
        let repo_root = discover_walk_runs()?
            .into_iter()
            .find(|run| run.campaign_id == campaign_id.as_str())
            .and_then(|run| run.worktree_root.or(Some(run.prototype1_root)))
            .unwrap_or_else(|| {
                campaign_manifest_path(&campaign_id)
                    .ok()
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .unwrap_or_default()
            });
        query_campaign_db_at(repo_root, campaign_id, script)
    }

    async fn send_read_only(&self, body: WalkRequestBody) -> Result<WalkSnapshot, PrepareError> {
        let mut stream = ipc::connect(&self.socket).await?;
        ipc::send(
            &mut stream,
            &WalkRequest {
                client_epoch: None,
                body,
            },
        )
        .await?;
        let response = ipc::recv(&mut stream).await?;
        Ok(WalkSnapshot::from_response(response))
    }

    fn resolve_campaign(&self, campaign: Option<&str>) -> Result<CampaignId, PrepareError> {
        if let Some(campaign) = campaign.filter(|text| !text.trim().is_empty()) {
            return Ok(CampaignId::from(campaign.trim()));
        }
        identity::load_parent_identity_optional(&self.repo_root)?.map_or_else(
            || {
                Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "cannot infer campaign id for db query; provide a campaign id or use a parent checkout containing '{}'",
                        identity::parent_identity_relpath().display()
                    ),
                })
            },
            |identity| Ok(identity.campaign_id().clone()),
        )
    }
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

fn query_campaign_db_at(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    script: &str,
) -> Result<DbQueryResult, PrepareError> {
    let manifest = campaign_manifest_path(&campaign_id)?;
    let db_path = prototype1_eval_store_db_path(&manifest);
    if !db_path.exists() {
        return Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_db_missing",
            detail: format!("owner eval DB does not exist at '{}'", db_path.display()),
        });
    }
    let db = load_owner_eval_database(&db_path).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_db_query_open",
        detail: source.to_string(),
    })?;
    let result = db
        .raw_query_params(script, BTreeMap::new())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_run",
            detail: source.to_string(),
        })?;
    Ok(query_result_view(
        repo_root,
        campaign_id,
        db_path,
        script,
        &result,
    ))
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
                    .map(|step| NextStepInfo {
                        edge: step.edge.to_string(),
                        phase: step.phase,
                        phase_id: step.phase.as_str().to_string(),
                        detail: step.detail.to_string(),
                        requires_watch: false,
                        requires_live_api: step.edge.contains("--allow-live-api"),
                        requires_git_changes: step.edge.contains("--allow git-changes"),
                    })
                    .collect(),
            })
            .collect();
        Self { phases }
    }
}

impl WalkSnapshot {
    fn from_response(response: WalkResponse) -> Self {
        match response {
            WalkResponse::Ok {
                phase,
                message,
                epoch,
            } => Self::from_parts(WalkReplyStatus::Ok, Some(phase), message, None, epoch),
            WalkResponse::Audit {
                phase,
                report,
                epoch,
            } => Self::from_parts(
                WalkReplyStatus::Audit,
                Some(phase),
                report.render_table(),
                None,
                epoch,
            ),
            WalkResponse::Job {
                phase,
                job,
                message,
                epoch,
            } => {
                let mut lines = vec![message];
                if let Some(job_message) = job.message {
                    lines.push(job_message);
                }
                Self::from_parts(
                    WalkReplyStatus::Ok,
                    Some(phase),
                    lines.join("\n"),
                    None,
                    epoch,
                )
            }
            WalkResponse::Status {
                phase,
                message,
                epoch,
                ..
            } => Self::from_parts(WalkReplyStatus::Ok, Some(phase), message, None, epoch),
            WalkResponse::Error {
                code,
                detail,
                phase,
                epoch,
                ..
            } => Self::from_parts(WalkReplyStatus::Error, phase, detail, Some(code), epoch),
        }
    }

    fn from_parts(
        status: WalkReplyStatus,
        phase: Option<WalkPhase>,
        message: String,
        error_code: Option<String>,
        epoch: crate::cli::prototype1_state::walk::epoch::ServerEpoch,
    ) -> Self {
        Self {
            status,
            phase,
            phase_id: phase.map(|phase| phase.as_str().to_string()),
            phase_label: phase.map(|phase| phase.detail().to_string()),
            message,
            error_code,
            epoch: ServerEpochView {
                protocol_version: epoch.protocol_version,
                transition_graph_version: epoch.transition_graph_version,
                repo_root: epoch.repo_root,
                exe_path: epoch.exe_path,
                exe_modified_unix_ms: epoch.exe_modified_unix_ms,
                git_head: epoch.git_head,
                source_status_hash: epoch.source_status_hash,
            },
        }
    }
}

fn all_phases() -> [WalkPhase; 21] {
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
        WalkPhase::R14a,
        WalkPhase::R14b,
    ]
}

fn query_result_view(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    db_path: PathBuf,
    script: &str,
    result: &QueryResult,
) -> DbQueryResult {
    let rows = result
        .rows
        .iter()
        .map(|row| {
            let cells = row.iter().map(data_value_json).collect::<Vec<_>>();
            let object = query_row_object(&result.headers, row);
            DbQueryRow { cells, object }
        })
        .collect::<Vec<_>>();
    DbQueryResult {
        repo_root,
        campaign_id: campaign_id.to_string(),
        db_path,
        script: script.to_string(),
        headers: result.headers.clone(),
        row_count: rows.len(),
        rows,
    }
}

fn query_row_object(headers: &[String], row: &[DataValue]) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (header, value) in headers.iter().zip(row.iter()) {
        object.insert(header.clone(), data_value_json(value));
    }
    serde_json::Value::Object(object)
}

fn data_value_json(value: &DataValue) -> serde_json::Value {
    match value {
        DataValue::Bot => serde_json::json!({ "cozo": "bot" }),
        other => serde_json::Value::from(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

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
