use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use tokio::net::UnixStream;

use crate::{
    cli::{InspectOutputFormat, Prototype1StateWalkStartCommand, Prototype1StateWalkSubcommand},
    spec::PrepareError,
};

use super::{
    args,
    epoch::ServerEpoch,
    ipc, paths,
    protocol::{WalkRequest, WalkRequestBody, WalkResponse},
};

pub(crate) async fn run(command: Prototype1StateWalkSubcommand) -> Result<(), PrepareError> {
    match command {
        Prototype1StateWalkSubcommand::Serve(_) => {
            unreachable!("serve is handled before client dispatch")
        }
        Prototype1StateWalkSubcommand::Start(command) => start(command).await,
        Prototype1StateWalkSubcommand::Step(command) => {
            let format = command.format;
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
            print_response(&response, format)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Show(command) => {
            let format = command.format;
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
            print_response(&response, format)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Status(command) => {
            let format = command.format;
            let socket_override = command.socket.clone();
            let (_repo_root, socket) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            match health(&socket).await? {
                Health::Online(response) => {
                    print_response(&response, format)?;
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
            print_response(&response, format)?;
            response_result(response)
        }
    }
}

async fn start(command: Prototype1StateWalkStartCommand) -> Result<(), PrepareError> {
    let format = command.format;
    let until = command.until;
    let socket_override = command.socket.clone();
    let (repo_root, socket) =
        args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
    ensure_server(&repo_root, &socket).await?;
    let epoch = ServerEpoch::capture(&repo_root)?;
    let response = send_request(
        &socket,
        WalkRequest {
            client_epoch: Some(epoch),
            body: WalkRequestBody::Start {
                config: command.start_config(),
                until,
            },
        },
    )
    .await?;
    print_response(&response, format)?;
    response_result(response)
}

async fn ensure_server(repo_root: &Path, socket: &Path) -> Result<(), PrepareError> {
    match health(socket).await? {
        Health::Online(_) => return Ok(()),
        Health::Offline => {}
    }
    paths::ensure_socket_parent(socket)?;
    paths::remove_socket_file(socket)?;
    spawn_server(repo_root, socket)?;
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

fn spawn_server(repo_root: &Path, socket: &Path) -> Result<(), PrepareError> {
    let exe = std::env::current_exe().map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_current_exe",
        detail: source.to_string(),
    })?;
    let mut command = Command::new(exe);
    command
        .arg("loop")
        .arg("prototype1-state-walk")
        .arg("serve")
        .arg("--repo-root")
        .arg(repo_root)
        .arg("--socket")
        .arg(socket)
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

async fn send_request(socket: &Path, request: WalkRequest) -> Result<WalkResponse, PrepareError> {
    let mut stream = ipc::connect(socket).await?;
    ipc::send(&mut stream, &request).await?;
    ipc::recv(&mut stream).await
}

enum Health {
    Online(WalkResponse),
    Offline,
}

async fn health(socket: &Path) -> Result<Health, PrepareError> {
    let mut stream = match UnixStream::connect(socket).await {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
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

fn print_response(
    response: &WalkResponse,
    format: InspectOutputFormat,
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
                println!("prototype1-state walk");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("phase: {phase}");
                println!("message: {message}");
                println!("protocol_version: {}", epoch.protocol_version);
                println!(
                    "transition_graph_version: {}",
                    epoch.transition_graph_version
                );
            }
            WalkResponse::Error {
                code,
                detail,
                phase,
                epoch,
            } => {
                println!("prototype1-state walk");
                println!("{}", "-".repeat(40));
                println!("status: error");
                println!("code: {code}");
                println!(
                    "phase: {}",
                    phase
                        .map(|phase| phase.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("detail: {detail}");
                println!("protocol_version: {}", epoch.protocol_version);
                println!(
                    "transition_graph_version: {}",
                    epoch.transition_graph_version
                );
            }
        },
    }
    Ok(())
}

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
            println!("prototype1-state walk");
            println!("{}", "-".repeat(40));
            println!("status: offline");
            println!("socket: {}", socket.display());
        }
    }
    Ok(())
}

fn response_result(response: WalkResponse) -> Result<(), PrepareError> {
    if response.is_ok() {
        Ok(())
    } else {
        Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-state walk request failed at {:?}",
                response.phase()
            ),
        })
    }
}
