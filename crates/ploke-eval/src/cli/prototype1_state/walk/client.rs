//! Short-lived CLI client for the typestate walk server.
//!
//! Client commands connect to one Unix socket, send one framed request, print
//! one framed response, and exit. `start` is the only command that auto-spawns
//! the server when health probing reports it offline.

use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use tokio::net::UnixStream;

use crate::{
    cli::{
        InspectOutputFormat, Prototype1StateWalkStartCommand, Prototype1StateWalkSubcommand,
        Prototype1StateWalkUseCommand,
    },
    spec::PrepareError,
};

use super::{
    args,
    epoch::ServerEpoch,
    ipc, paths,
    protocol::{WalkRequest, WalkRequestBody, WalkResponse},
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
            let socket_override = command.socket.clone();
            let (repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let epoch = ServerEpoch::capture(&repo_root)?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Step { until },
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
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (_repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_epoch: None,
                    body: WalkRequestBody::Show,
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
                println!("phase: {phase}");
                print_multiline("message", message);
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
                        .map(|phase| phase.to_string())
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
