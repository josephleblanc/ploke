//! Short-lived CLI client for the typestate walk server.
//!
//! Client commands connect to one Unix socket, send one framed request, print
//! one framed response, and exit. `start` is the only command that auto-spawns
//! the server when health probing reports it offline.

use std::{
    collections::BTreeMap,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_records::ids::CampaignId;
use tokio::net::UnixStream;

use crate::{
    campaign::campaign_manifest_path,
    cli::prototype1_state::{
        eval_store::{load_owner_eval_database, prototype1_eval_store_db_path},
        identity,
    },
    cli::{
        InspectOutputFormat, Prototype1StateWalkDbQueryCommand, Prototype1StateWalkLlmSubcommand,
        Prototype1StateWalkShowSubcommand, Prototype1StateWalkStartCommand,
        Prototype1StateWalkSubcommand, Prototype1StateWalkUseCommand,
    },
    spec::PrepareError,
};

use super::{
    args,
    epoch::ServerEpoch,
    ipc, paths,
    protocol::{WalkRequest, WalkRequestBody, WalkResponse},
    summary,
};

/// Execute a non-`serve` walk subcommand as a one-shot client request.
pub(crate) async fn run(command: Prototype1StateWalkSubcommand) -> Result<(), PrepareError> {
    match command {
        Prototype1StateWalkSubcommand::Serve(_) => {
            unreachable!("serve is handled before client dispatch")
        }
        Prototype1StateWalkSubcommand::Use(command) => use_context(command),
        Prototype1StateWalkSubcommand::Start(command) => start(command).await,
        Prototype1StateWalkSubcommand::Step(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let until = command.until;
            let watch = command.watch;
            let allow_git_changes = command.allow_git_changes();
            let socket_override = command.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let epoch = ServerEpoch::capture(&repo_root)?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Step {
                        until,
                        watch,
                        allow_git_changes,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Reset(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let epoch = ServerEpoch::capture(&repo_root)?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Reset,
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Files(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (_repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::Files,
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Show(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let body = match command.command {
                Some(Prototype1StateWalkShowSubcommand::Delta(delta)) => {
                    WalkRequestBody::ShowDelta {
                        verbose: delta.verbose,
                        color: format == InspectOutputFormat::Table && !delta.no_color,
                    }
                }
                None => WalkRequestBody::Show,
            };
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body,
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Audit(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::Audit {
                        campaign: command.campaign,
                        scope: command.scope,
                        transition: command.transition,
                        verify: command.verify,
                        verbose: command.verbose,
                        with_note: command.with_note,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Llm(command) => {
            let raw_json_message = matches!(
                &command.command,
                Prototype1StateWalkLlmSubcommand::Prompt(prompt) if prompt.json
            ) || matches!(
                &command.command,
                Prototype1StateWalkLlmSubcommand::Protocol(protocol) if protocol.json
            ) || matches!(
                &command.command,
                Prototype1StateWalkLlmSubcommand::Tool(tool) if tool.json
            );
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let body = match command.command {
                Prototype1StateWalkLlmSubcommand::Lanes(lanes) => WalkRequestBody::LlmLanes {
                    verbose: lanes.verbose,
                },
                Prototype1StateWalkLlmSubcommand::Focus(focus) => {
                    WalkRequestBody::LlmFocus { lane: focus.lane }
                }
                Prototype1StateWalkLlmSubcommand::Show(show) => WalkRequestBody::LlmShow {
                    session_id: show.session_id,
                    lane: show.lane,
                    head: show.head,
                    step: show.step,
                },
                Prototype1StateWalkLlmSubcommand::Timeline(timeline) => {
                    WalkRequestBody::LlmTimeline {
                        session_id: timeline.session_id,
                        lane: timeline.lane,
                    }
                }
                Prototype1StateWalkLlmSubcommand::Prompt(prompt) => WalkRequestBody::LlmPrompt {
                    session_id: prompt.session_id,
                    lane: prompt.lane,
                    step: prompt.step,
                    role: prompt.role,
                    message: prompt.message,
                    full: prompt.full,
                    json: prompt.json,
                },
                Prototype1StateWalkLlmSubcommand::Protocol(protocol) => {
                    WalkRequestBody::LlmProtocol {
                        session_id: protocol.session_id,
                        lane: protocol.lane,
                        json: protocol.json,
                    }
                }
                Prototype1StateWalkLlmSubcommand::Tool(tool) => WalkRequestBody::LlmTool {
                    session_id: tool.session_id,
                    lane: tool.lane,
                    head: tool.head,
                    step: tool.step,
                    call: tool.call,
                    name: tool.name,
                    json: tool.json,
                },
                Prototype1StateWalkLlmSubcommand::Step(step) => {
                    let allow_workspace_mutation = step.allow_workspace_mutation();
                    WalkRequestBody::LlmStep {
                        session_id: step.session_id,
                        lane: step.lane,
                        step: step.step,
                        source: step.source,
                        watch: step.watch,
                        allow_workspace_mutation,
                        model_id: step.model_id,
                        provider: step.provider,
                        max_attempts: step.max_attempts,
                        timeout_secs: step.timeout_secs,
                    }
                }
                Prototype1StateWalkLlmSubcommand::Finish(finish) => {
                    let allow_workspace_mutation = finish.allow_workspace_mutation();
                    WalkRequestBody::LlmFinish {
                        session_id: finish.session_id,
                        lane: finish.lane,
                        step: finish.step,
                        watch: finish.watch,
                        allow_workspace_mutation,
                        model_id: finish.model_id,
                        provider: finish.provider,
                        max_steps: finish.max_steps,
                        max_attempts: finish.max_attempts,
                        timeout_secs: finish.timeout_secs,
                    }
                }
                Prototype1StateWalkLlmSubcommand::Back(back) => WalkRequestBody::LlmBack {
                    lane: back.lane.lane,
                    steps: back.steps,
                },
                Prototype1StateWalkLlmSubcommand::Forward(forward) => WalkRequestBody::LlmForward {
                    lane: forward.lane.lane,
                    steps: forward.steps,
                },
                Prototype1StateWalkLlmSubcommand::Head(head) => {
                    WalkRequestBody::LlmHead { lane: head.lane }
                }
            };
            let client_epoch = if matches!(
                body,
                WalkRequestBody::LlmStep { .. } | WalkRequestBody::LlmFinish { .. }
            ) {
                Some(ServerEpoch::capture(&repo_root)?)
            } else {
                None
            };
            let response = send_request(&socket, WalkRequest { client_epoch, body }).await?;
            if raw_json_message && response.is_ok() {
                print_ok_message(&response);
            } else {
                print_response(&response, format, with_version)?;
            }
            response_result(response)
        }
        Prototype1StateWalkSubcommand::DbQuery(command) => run_db_query(command),
        Prototype1StateWalkSubcommand::Summary(command) => summary::run(command),
        Prototype1StateWalkSubcommand::Replay(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::Replay {
                        index: command.index,
                        tail: command.tail,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Back(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::ReplayBack {
                        steps: command.steps,
                        tail: command.tail,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Forward(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::ReplayForward {
                        steps: command.steps,
                        tail: command.tail,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::BranchLive(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let allow_provenance_record = command.allow_provenance_record();
            let reason = command.reason.clone();
            let socket_override = command.control.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            ensure_server(&repo_root, &socket, default_idle_ttl()).await?;
            let epoch = ServerEpoch::capture(&repo_root)?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::BranchLive {
                        reason,
                        allow_provenance_record,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Status(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (_repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            match health(&socket).await? {
                Health::Online(response) => {
                    print_response(&response, format, with_version)?;
                    Ok(())
                }
                Health::Offline => {
                    print_offline(&socket, format)?;
                    Ok(())
                }
            }
        }
        Prototype1StateWalkSubcommand::Stop(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (_repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::Stop,
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
    }
}

/// Start the server if needed, then create a new in-memory walk.
async fn start(command: Prototype1StateWalkStartCommand) -> Result<(), PrepareError> {
    let format = command.format;
    let with_version = command.with_version;
    let until = command.until;
    let idle_ttl = command.idle_ttl()?;
    let socket_override = command.socket.clone();
    let (repo_root, socket) =
        args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
    ensure_server(&repo_root, &socket, idle_ttl).await?;
    let epoch = ServerEpoch::capture(&repo_root)?;
    let mut config = command.start_config();
    config.repo_root = Some(repo_root.clone());
    let response = send_request(
        &socket,
        WalkRequest {
            client_epoch: Some(epoch),
            body: WalkRequestBody::Start { config, until },
        },
    )
    .await?;
    print_response(&response, format, with_version)?;
    response_result(response)
}

fn default_idle_ttl() -> Option<Duration> {
    Some(Duration::from_secs(args::DEFAULT_IDLE_TTL_SECS))
}

fn use_context(command: Prototype1StateWalkUseCommand) -> Result<(), PrepareError> {
    let repo_root = paths::resolve_use_repo_root(command.repo_root.as_deref())?;
    let socket = paths::socket_path(&repo_root, command.socket.as_deref())?;
    let context = paths::WalkContext {
        repo_root,
        socket: Some(socket),
    };
    let context_path = paths::save_context(&context)?;
    print_context(&context, &context_path, command.format)
}

fn run_db_query(command: Prototype1StateWalkDbQueryCommand) -> Result<(), PrepareError> {
    let repo_root = paths::resolve_repo_root(command.repo_root.as_deref())?;
    let campaign_id = resolve_db_query_campaign(&repo_root, command.campaign)?;
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
        .raw_query_params(&command.script, BTreeMap::new())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_run",
            detail: source.to_string(),
        })?;
    print_query_result(
        &repo_root,
        &campaign_id,
        &db_path,
        &command.script,
        &result,
        command.format,
    )
}

fn resolve_db_query_campaign(
    repo_root: &Path,
    campaign: Option<CampaignId>,
) -> Result<CampaignId, PrepareError> {
    if let Some(campaign) = campaign {
        return Ok(campaign);
    }
    identity::load_parent_identity_optional(repo_root)?.map_or_else(
        || {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "cannot infer campaign id for db_query; pass --campaign or run `walk use` with a parent checkout containing '{}'",
                    identity::parent_identity_relpath().display()
                ),
            })
        },
        |identity| Ok(identity.campaign_id().clone()),
    )
}

/// Ensure a healthy server is listening at `socket`, spawning one if absent.
async fn ensure_server(
    repo_root: &Path,
    socket: &Path,
    idle_ttl: Option<Duration>,
) -> Result<(), PrepareError> {
    match health(socket).await? {
        Health::Online(_) => return Ok(()),
        Health::Offline => {}
    }
    paths::ensure_socket_parent(socket)?;
    paths::remove_socket_file(socket)?;
    spawn_server(repo_root, socket, idle_ttl)?;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Health::Online(_) = health(socket).await? {
            return Ok(());
        }
    }
    Err(PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_server_startup",
        detail: format!(
            "walk server did not become healthy at '{}' within startup timeout",
            socket.display()
        ),
    })
}

/// Spawn the same binary in `walk serve` mode.
///
/// This is intentionally lighter than full daemonization in the first server
/// slice: stdio is detached, the child gets its own process group on Unix, and
/// the caller polls `Health` before sending the real request.
fn spawn_server(
    repo_root: &Path,
    socket: &Path,
    idle_ttl: Option<Duration>,
) -> Result<(), PrepareError> {
    let exe = std::env::current_exe().map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_current_exe",
        detail: source.to_string(),
    })?;
    let mut command = Command::new(exe);
    command
        .arg("loop")
        .arg("walk")
        .arg("serve")
        .arg("--repo-root")
        .arg(repo_root)
        .arg("--socket")
        .arg(socket);
    match idle_ttl {
        Some(ttl) => {
            command.arg("--ttl-secs").arg(ttl.as_secs().to_string());
        }
        None => {
            command.arg("--no-ttl");
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .spawn()
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_spawn_server",
            detail: source.to_string(),
        })?;
    Ok(())
}

/// Send one request and wait for one response.
async fn send_request(socket: &Path, request: WalkRequest) -> Result<WalkResponse, PrepareError> {
    let mut stream = ipc::connect(socket).await?;
    ipc::send(&mut stream, &request).await?;
    ipc::recv(&mut stream).await
}

/// Result of probing a walk socket without mutating state.
enum Health {
    Online(WalkResponse),
    Offline,
}

/// Probe whether a server is online by sending `WalkRequestBody::Health`.
async fn health(socket: &Path) -> Result<Health, PrepareError> {
    let mut stream = match UnixStream::connect(socket).await {
        Ok(stream) => stream,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Health::Offline);
        }
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
            let _ = paths::remove_socket_file(socket);
            return Ok(Health::Offline);
        }
        Err(source) => {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_state_walk_health_connect",
                detail: format!(
                    "failed to probe walk socket '{}': {source}",
                    socket.display()
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
    Ok(Health::Online(response))
}

fn print_context(
    context: &paths::WalkContext,
    context_path: &Path,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            let value = serde_json::json!({
                "type": "walk_context",
                "status": "saved",
                "repo_root": context.repo_root,
                "socket": context.socket,
                "context_path": context_path,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("walk context");
            println!("{}", "-".repeat(40));
            println!("status: saved");
            println!("repo_root: {}", context.repo_root.display());
            println!(
                "socket: {}",
                context
                    .socket
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!("context: {}", context_path.display());
        }
    }
    Ok(())
}

fn print_query_result(
    repo_root: &Path,
    campaign_id: &CampaignId,
    db_path: &Path,
    script: &str,
    result: &QueryResult,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            let rows = result
                .rows
                .iter()
                .map(|row| query_row_json(&result.headers, row))
                .collect::<Vec<_>>();
            let value = serde_json::json!({
                "type": "walk_db_query",
                "repo_root": repo_root,
                "campaign_id": campaign_id.as_str(),
                "db_path": db_path,
                "script": script,
                "headers": result.headers,
                "row_count": result.rows.len(),
                "rows": rows,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("walk db query");
            println!("{}", "-".repeat(40));
            println!("status: ok");
            println!("repo_root: {}", repo_root.display());
            println!("campaign_id: {}", campaign_id.as_str());
            println!("db_path: {}", db_path.display());
            println!("rows: {}", result.rows.len());
            print_multiline("script", script);
            if result.headers.is_empty() {
                println!("headers: -");
                return Ok(());
            }
            println!("headers: {}", result.headers.join(" | "));
            for (index, row) in result.rows.iter().enumerate() {
                let cells = row.iter().map(format_data_value).collect::<Vec<_>>();
                println!("row[{index}]: {}", cells.join(" | "));
            }
        }
    }
    Ok(())
}

fn query_row_json(headers: &[String], row: &[DataValue]) -> serde_json::Value {
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

fn format_data_value(value: &DataValue) -> String {
    match value {
        DataValue::Str(text) => text.to_string(),
        DataValue::Null => "null".to_string(),
        other => other.to_string(),
    }
}

/// Render one server response in table or JSON format.
fn print_response(
    response: &WalkResponse,
    format: InspectOutputFormat,
    with_version: bool,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(response).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => match response {
            WalkResponse::Ok {
                phase,
                message,
                epoch,
            } => {
                println!("walk");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("phase: {phase} - {}", phase.detail());
                print_multiline("message", message);
                if with_version {
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Audit { report, epoch, .. } => {
                println!("{}", report.render_table());
                if with_version {
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Error {
                code,
                detail,
                phase,
                epoch,
            } => {
                println!("walk");
                println!("{}", "-".repeat(40));
                println!("status: error");
                println!("code: {code}");
                println!(
                    "phase: {}",
                    phase
                        .map(|phase| format!("{phase} - {}", phase.detail()))
                        .unwrap_or_else(|| "-".to_string())
                );
                print_multiline("detail", detail);
                if with_version {
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
        },
    }
    Ok(())
}

fn print_multiline(label: &str, value: &str) {
    if value.contains('\n') {
        println!("{label}:");
        for line in value.lines() {
            println!("  {line}");
        }
    } else {
        println!("{label}: {value}");
    }
}

fn print_ok_message(response: &WalkResponse) {
    if let WalkResponse::Ok { message, .. } = response {
        println!("{message}");
    }
}

/// Render an offline status without treating it as a command failure.
fn print_offline(socket: &Path, format: InspectOutputFormat) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            let value = serde_json::json!({
                "type": "offline",
                "socket": socket,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("walk");
            println!("{}", "-".repeat(40));
            println!("status: offline");
            println!("socket: {}", socket.display());
        }
    }
    Ok(())
}

/// Convert a protocol response into the CLI process result.
fn response_result(response: WalkResponse) -> Result<(), PrepareError> {
    if response.is_ok() {
        Ok(())
    } else {
        Err(PrepareError::InvalidBatchSelection {
            detail: format!("walk request failed at {:?}", response.phase()),
        })
    }
}
