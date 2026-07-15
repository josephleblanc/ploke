//! Short-lived CLI client for the typestate walk server.
//!
//! Client commands connect to one Unix socket, send framed requests, print
//! server responses, and exit. Live `start`/`step` requests are submitted as
//! server jobs; `step --watch` follows the accepted job by polling status until
//! it reaches a terminal state.

use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use crate::{
    cli::prototype1_state::driver::control::RecoveryDirective,
    cli::{
        InspectOutputFormat, Prototype1StateWalkDbQueryCommand, Prototype1StateWalkLlmSubcommand,
        Prototype1StateWalkShowSubcommand, Prototype1StateWalkStartCommand,
        Prototype1StateWalkSubcommand, Prototype1StateWalkTraceSubcommand,
        Prototype1StateWalkUseCommand,
    },
    spec::PrepareError,
    walk_client::WalkClient,
};

use super::{
    args,
    epoch::{ServerEpoch, WALK_PROTOCOL_VERSION},
    ipc, paths,
    protocol::{
        MutationGuard, OperationId, SessionVersion, WalkDeltaState, WalkJobResolutionKind,
        WalkJobSnapshot, WalkJobStatus, WalkPosition, WalkRequest, WalkRequestBody, WalkResponse,
    },
    summary,
    trace::{EvaluationRunCoordinate, EvaluationTraceState},
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
            let allow_live_api = command.allow_live_api;
            let allow_git_changes = command.allow_git_changes();
            let socket_override = command.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let (socket, epoch, guard) = mutation_guard(&client, command.operation_id).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Step {
                        guard,
                        until,
                        watch,
                        allow_live_api,
                        allow_git_changes,
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            if watch {
                return watch_active_job(
                    &repo_root,
                    socket_override.as_deref(),
                    format,
                    with_version,
                    response,
                )
                .await;
            }
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Reset(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            let (socket, epoch, guard) = mutation_guard(&client, command.operation_id).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Reset { guard },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Recover(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            if command.abandon_job.is_none()
                && command.directive() == RecoveryDirective::Inspect
                && command.operation_id.is_some()
            {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "read-only recovery inspection does not accept --operation-id"
                        .to_string(),
                });
            }
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            if let Some(operation) = command.abandon_job {
                let (socket, epoch, guard) = mutation_guard(&client, Some(operation)).await?;
                let response = send_request(
                    &socket,
                    WalkRequest {
                        client_protocol: Some(WALK_PROTOCOL_VERSION),
                        client_epoch: Some(epoch),
                        body: WalkRequestBody::ResolveJob {
                            guard,
                            resolution: WalkJobResolutionKind::Abandon,
                        },
                    },
                )
                .await?;
                print_response(&response, format, with_version)?;
                return response_result(response);
            }
            let directive = command.directive();
            if directive == RecoveryDirective::Inspect {
                let response = client
                    .send_read_only(WalkRequestBody::Recover {
                        directive,
                        guard: None,
                    })
                    .await?;
                print_response(&response, format, with_version)?;
                return recovery_result(response);
            }
            let (socket, epoch, guard) = mutation_guard(&client, command.operation_id).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::Recover {
                        directive,
                        guard: Some(guard),
                    },
                },
            )
            .await?;
            print_response(&response, format, with_version)?;
            recovery_result(response)
        }
        Prototype1StateWalkSubcommand::Files(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            let response = client.send_read_only(WalkRequestBody::Files).await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Config(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let response = client.send_read_only(WalkRequestBody::Config).await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Trace(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let body = match command.command {
                Prototype1StateWalkTraceSubcommand::List => WalkRequestBody::EvaluationTraceIndex,
                Prototype1StateWalkTraceSubcommand::Show(show) => {
                    let campaign = match show.campaign {
                        Some(campaign) => ploke_records::ids::CampaignId::from(campaign),
                        None => client.config().await?.identity.record.campaign_id,
                    };
                    WalkRequestBody::EvaluationTrace {
                        coordinate: EvaluationRunCoordinate {
                            campaign,
                            instance: ploke_records::ids::InstanceId(show.instance),
                            run_id: show.run_id,
                        },
                    }
                }
            };
            let response = client.send_read_only(body).await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Show(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let body = match command.command {
                Some(Prototype1StateWalkShowSubcommand::Delta(delta)) => {
                    WalkRequestBody::ShowDelta {
                        verbose: delta.verbose,
                        color: format == InspectOutputFormat::Table && !delta.no_color,
                    }
                }
                None => WalkRequestBody::Show,
            };
            let response = client.send_read_only(body).await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::SessionHistory(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let response = client
                .send_read_only(WalkRequestBody::SessionHistory)
                .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Audit(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let response = client
                .send_read_only(WalkRequestBody::Audit {
                    campaign: command.campaign,
                    scope: command.scope,
                    transition: command.transition,
                    verify: command.verify,
                    verbose: command.verbose,
                    with_note: command.with_note,
                })
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
            let (repo_root, _) =
                args::resolve_socket(command.control.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let operation = match &command.command {
                Prototype1StateWalkLlmSubcommand::Step(step) => step.operation_id,
                Prototype1StateWalkLlmSubcommand::Finish(finish) => finish.operation_id,
                _ => None,
            };
            let job_watch = match &command.command {
                Prototype1StateWalkLlmSubcommand::Step(step) => step.watch,
                Prototype1StateWalkLlmSubcommand::Finish(finish) => finish.watch,
                _ => false,
            };
            let mutation = if matches!(
                &command.command,
                Prototype1StateWalkLlmSubcommand::Step(_)
                    | Prototype1StateWalkLlmSubcommand::Finish(_)
            ) {
                Some(mutation_guard(&client, operation).await?)
            } else {
                None
            };
            let (socket, client_epoch, guard) = match mutation {
                Some((socket, epoch, guard)) => (socket, Some(epoch), Some(guard)),
                None => (client.resolved_socket()?, None, None),
            };
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
                        guard: guard.expect("effectful LLM step guard was probed"),
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
                        guard: guard.expect("effectful LLM finish guard was probed"),
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
            let response = if client_epoch.is_none() && retry_safe_read(&body) {
                client.send_read_only(body).await?
            } else {
                send_request(
                    &socket,
                    WalkRequest {
                        client_protocol: Some(WALK_PROTOCOL_VERSION),
                        client_epoch,
                        body,
                    },
                )
                .await?
            };
            if raw_json_message && response.is_ok() {
                print_ok_message(&response);
            } else {
                print_response(&response, format, with_version)?;
            }
            if job_watch {
                return watch_active_job(
                    &repo_root,
                    socket_override.as_deref(),
                    format,
                    with_version,
                    response,
                )
                .await;
            }
            response_result(response)
        }
        Prototype1StateWalkSubcommand::DbQuery(command) => run_db_query(command).await,
        Prototype1StateWalkSubcommand::Summary(command) => summary::run(command),
        Prototype1StateWalkSubcommand::Replay(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let response = client
                .send_read_only(WalkRequestBody::Replay {
                    index: command.index,
                    tail: command.tail,
                })
                .await?;
            print_response(&response, format, with_version)?;
            response_result(response)
        }
        Prototype1StateWalkSubcommand::Back(command) => {
            let format = command.control.format;
            let with_version = command.control.with_version;
            let socket_override = command.control.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let socket = client.resolved_socket()?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
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
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let socket = client.resolved_socket()?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
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
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            ensure_server(&client, default_idle_ttl()).await?;
            let (socket, epoch, guard) = mutation_guard(&client, command.operation_id).await?;
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(epoch),
                    body: WalkRequestBody::BranchLive {
                        guard,
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
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            match health(&client).await? {
                Health::Online { response, .. } => {
                    print_response(&response, format, with_version)?;
                    Ok(())
                }
                Health::Offline { socket } => {
                    print_offline(&socket, format)?;
                    Ok(())
                }
            }
        }
        Prototype1StateWalkSubcommand::Stop(command) => {
            let format = command.format;
            let with_version = command.with_version;
            let socket_override = command.socket.clone();
            let (repo_root, _) =
                args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
            let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
            let (socket, epoch) = match health(&client).await? {
                Health::Online { socket, response } => {
                    let epoch = stop_epoch(&repo_root, &response)?;
                    (socket, epoch)
                }
                Health::Offline { socket } => {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "prototype1_state_walk_stop",
                        detail: format!(
                            "walk server is offline at '{}'; no endpoint authority was available for stop",
                            socket.display()
                        ),
                    });
                }
            };
            let response = send_request(
                &socket,
                WalkRequest {
                    client_protocol: Some(WALK_PROTOCOL_VERSION),
                    client_epoch: Some(epoch),
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
    let allow_live_api = command.allow_live_api;
    let idle_ttl = command.idle_ttl()?;
    let socket_override = command.socket.clone();
    let (repo_root, _) = args::resolve_socket(command.repo_root_ref(), socket_override.as_deref())?;
    let client = WalkClient::resolve(Some(&repo_root), socket_override.as_deref())?;
    ensure_server(&client, idle_ttl).await?;
    let (socket, epoch, guard) = mutation_guard(&client, command.operation_id).await?;
    let mut config = command.start_config();
    config.repo_root = Some(repo_root.clone());
    let response = send_request(
        &socket,
        WalkRequest {
            client_protocol: Some(WALK_PROTOCOL_VERSION),
            client_epoch: Some(epoch),
            body: WalkRequestBody::Start {
                guard,
                config,
                until,
                allow_live_api,
            },
        },
    )
    .await?;
    print_response(&response, format, with_version)?;
    response_result(response)
}

async fn watch_active_job(
    repo_root: &Path,
    socket_override: Option<&Path>,
    format: InspectOutputFormat,
    with_version: bool,
    initial: WalkResponse,
) -> Result<(), PrepareError> {
    let Some((job_id, operation)) = active_job(&initial).map(|job| (job.job_id, job.operation_id))
    else {
        return response_result(initial);
    };
    let client = WalkClient::resolve(Some(repo_root), socket_override)?;
    let mut last = response_fingerprint(&initial);
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let response = client
            .send_read_only(WalkRequestBody::OperationStatus { operation })
            .await?;
        let fingerprint = response_fingerprint(&response);
        if fingerprint != last {
            print_response(&response, format, with_version)?;
            last = fingerprint;
        }
        match &response {
            WalkResponse::Job { job, .. }
                if job.operation_id == operation && job.status.is_active() => {}
            _ => return watched_job_result(response, job_id),
        }
    }
}

fn watched_job_result(response: WalkResponse, job_id: u64) -> Result<(), PrepareError> {
    if let Some(job) = job_by_id(&response, job_id) {
        match job.status {
            WalkJobStatus::Succeeded => return response_result(response),
            WalkJobStatus::Failed
            | WalkJobStatus::Cancelled
            | WalkJobStatus::Indeterminate
            | WalkJobStatus::Abandoned => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: job
                        .message
                        .clone()
                        .unwrap_or_else(|| format!("walk job {job_id} ended as {:?}", job.status)),
                });
            }
            WalkJobStatus::Running | WalkJobStatus::CancelRequested => {}
        }
    }
    response_result(response)
}

fn active_job(response: &WalkResponse) -> Option<&WalkJobSnapshot> {
    match response {
        WalkResponse::Job { job, .. } if job.status.is_active() => Some(job),
        WalkResponse::Status { snapshot, .. }
            if snapshot
                .job
                .as_ref()
                .is_some_and(|job| job.status.is_active()) =>
        {
            snapshot.job.as_ref()
        }
        _ => None,
    }
}

fn response_epoch(response: &WalkResponse) -> &ServerEpoch {
    response.epoch()
}

fn job_by_id(response: &WalkResponse, job_id: u64) -> Option<&WalkJobSnapshot> {
    match response {
        WalkResponse::Job { job, .. } if job.job_id == job_id => Some(job),
        WalkResponse::Status { snapshot, .. } => {
            snapshot.job.as_ref().filter(|job| job.job_id == job_id)
        }
        _ => None,
    }
}

fn response_fingerprint(response: &WalkResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|_| format!("{response:?}"))
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

async fn run_db_query(command: Prototype1StateWalkDbQueryCommand) -> Result<(), PrepareError> {
    let (repo_root, _) = args::resolve_socket(command.repo_root.as_deref(), None)?;
    let client = WalkClient::resolve(Some(&repo_root), None)?;
    ensure_server(&client, default_idle_ttl()).await?;
    let response = client
        .send_read_only(WalkRequestBody::DbQuery {
            campaign: command.campaign,
            script: command.script,
        })
        .await?;
    print_response(&response, command.format, true)?;
    response_result(response)
}

/// Ensure a healthy server is listening at `socket`, spawning one if absent.
async fn ensure_server(
    client: &WalkClient,
    idle_ttl: Option<Duration>,
) -> Result<(), PrepareError> {
    match health(client).await? {
        Health::Online { .. } => return Ok(()),
        Health::Offline { .. } => {}
    }
    let repo_root = client.repo_root();
    let socket = client.resolved_socket()?;
    paths::ensure_socket_parent(&socket)?;
    spawn_server(repo_root, &socket, idle_ttl)?;
    let pinned = WalkClient::resolve(Some(repo_root), Some(&socket))?;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Health::Online { .. } = health(&pinned).await? {
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

fn retry_safe_read(body: &WalkRequestBody) -> bool {
    matches!(
        body,
        WalkRequestBody::Health
            | WalkRequestBody::Files
            | WalkRequestBody::Config
            | WalkRequestBody::EvaluationTraceIndex
            | WalkRequestBody::EvaluationTrace { .. }
            | WalkRequestBody::Show
            | WalkRequestBody::SessionHistory
            | WalkRequestBody::OperationStatus { .. }
            | WalkRequestBody::ShowDelta { .. }
            | WalkRequestBody::Audit { .. }
            | WalkRequestBody::DbQuery { .. }
            | WalkRequestBody::LlmLanes { .. }
            | WalkRequestBody::LlmShow { .. }
            | WalkRequestBody::LlmTimeline { .. }
            | WalkRequestBody::LlmPrompt { .. }
            | WalkRequestBody::LlmProtocol { .. }
            | WalkRequestBody::LlmTool { .. }
            | WalkRequestBody::Replay { .. }
            | WalkRequestBody::Recover {
                directive: RecoveryDirective::Inspect,
                guard: None,
            }
    )
}

/// Result of probing a walk socket without mutating state.
enum Health {
    Online {
        socket: std::path::PathBuf,
        response: WalkResponse,
    },
    Offline {
        socket: std::path::PathBuf,
    },
}

/// Probe whether a server is online by sending `WalkRequestBody::Health`.
async fn health(client: &WalkClient) -> Result<Health, PrepareError> {
    match client.health_observation().await? {
        Some((socket, response)) => Ok(Health::Online { socket, response }),
        None => Ok(Health::Offline {
            socket: client.resolved_socket()?,
        }),
    }
}

/// Capture the endpoint epoch and exact durable controller-session version in
/// one non-mutating probe before submitting a live operation.
async fn mutation_guard(
    client: &WalkClient,
    operation: Option<OperationId>,
) -> Result<(std::path::PathBuf, ServerEpoch, MutationGuard), PrepareError> {
    match health(client).await? {
        Health::Online {
            socket,
            response: response @ WalkResponse::Status { .. },
        } => {
            let epoch = fresh_epoch(client.repo_root(), &response)?;
            let WalkResponse::Status { snapshot, .. } = response else {
                unreachable!("status response matched above")
            };
            Ok((
                socket,
                epoch,
                MutationGuard {
                    operation: operation.unwrap_or_else(OperationId::new),
                    expected: snapshot.version(),
                },
            ))
        }
        Health::Online { response, .. } => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk mutation requires a typed health/status version; endpoint returned {response:?}"
            ),
        }),
        Health::Offline { socket } => Err(PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_mutation_probe",
            detail: format!(
                "walk server is offline at '{}'; mutation admission requires an observed server epoch and durable session version",
                socket.display()
            ),
        }),
    }
}

/// Capture this client's repository epoch and bind it to the observed endpoint.
fn fresh_epoch(repo_root: &Path, response: &WalkResponse) -> Result<ServerEpoch, PrepareError> {
    let epoch = ServerEpoch::capture(repo_root)?;
    response_epoch(response).ensure_compatible_request(Some(&epoch))?;
    Ok(epoch)
}

/// Bind shutdown to the caller-selected repository while allowing stale-source cleanup.
fn stop_epoch(repo_root: &Path, response: &WalkResponse) -> Result<ServerEpoch, PrepareError> {
    let epoch = ServerEpoch::capture(repo_root)?;
    let server = response_epoch(response);
    if server.repo_root != epoch.repo_root {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "walk repository root mismatch: client='{}' server='{}'",
                epoch.repo_root.display(),
                server.repo_root.display()
            ),
        });
    }
    Ok(epoch)
}

#[cfg(test)]
mod epoch_tests {
    use super::*;
    use crate::cli::prototype1_state::walk::protocol::{WalkPosition, WalkSessionSnapshot};

    #[test]
    fn fresh_epoch_rejects_socket_for_different_repo() {
        let server = tempfile::tempdir().expect("server repo");
        let client = tempfile::tempdir().expect("client repo");
        let response = WalkResponse::Status {
            message: "online".to_string(),
            snapshot: WalkSessionSnapshot {
                position: WalkPosition::NoSession,
                controller_attached: false,
                authority: crate::cli::prototype1_state::walk::protocol::WalkAuthority::Active,
                job: None,
                blocker: None,
                actions: Vec::new(),
            },
            epoch: ServerEpoch::capture(server.path()).expect("server epoch"),
        };

        let error = fresh_epoch(client.path(), &response)
            .expect_err("an explicit socket cannot confer cross-repository authority");
        assert!(
            error.to_string().contains("repository root mismatch"),
            "{error}"
        );
    }
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
            println!("{}", render_response_json(response)?);
        }
        InspectOutputFormat::Table => match response {
            WalkResponse::Ok {
                phase,
                result,
                epoch,
            } => {
                println!("walk");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("operation: {}", result.kind().as_str());
                println!("phase: {phase} - {}", phase.detail());
                print_multiline("result", result.text());
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
            WalkResponse::Query { query } => {
                let result = &query.result;
                println!("walk db query");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("repo_root: {}", result.repo_root.display());
                println!("campaign_id: {}", result.campaign_id);
                println!("db_path: {}", result.db_path.display());
                println!("revision: {}", result.revision.as_str());
                println!("session_revision: {}", query.version.journal_revision());
                println!("rows: {}", result.row_count);
                print_multiline("script", &result.script);
                if result.headers.is_empty() {
                    println!("headers: -");
                } else {
                    println!("headers: {}", result.headers.join(" | "));
                    for (index, row) in result.rows.iter().enumerate() {
                        let cells = row
                            .cells
                            .iter()
                            .map(serde_json::Value::to_string)
                            .collect::<Vec<_>>();
                        println!("row[{index}]: {}", cells.join(" | "));
                    }
                }
                if with_version {
                    println!("protocol_version: {}", query.epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        query.epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Config {
                phase,
                config,
                epoch,
            } => {
                println!("walk configuration");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("phase: {phase} - {}", phase.detail());
                println!("identity_path: {}", config.identity.path.display());
                println!("campaign_id: {}", config.identity.record.campaign_id);
                println!("parent_id: {}", config.identity.record.parent_id);
                println!("node_id: {}", config.identity.record.node_id);
                println!("campaign_path: {}", config.campaign.path.display());
                println!("campaign_sha256: {}", config.campaign.content_hash.as_str());
                println!(
                    "setup_receipt: {}",
                    config.campaign.admission.path.display()
                );
                println!("setup_plan_sha256: {}", config.campaign.admission.plan_hash);
                println!(
                    "setup_root: {}",
                    config.campaign.admission.setup_root.display()
                );
                println!("setup_started_at: {}", config.campaign.admission.started_at);
                println!(
                    "setup_completed_head: {}",
                    config.campaign.admission.completed_head
                );
                println!("model_id: {}", config.campaign.resolved.model_id);
                println!("route_source: {:?}", config.campaign.resolved.route_source);
                println!("provider_selection: {}", config.campaign.provider);
                println!("provider_source: admitted_campaign_manifest");
                println!("profile_name: {}", config.profile.record.name);
                println!(
                    "profile_path: {}",
                    config.profile.commitment.profile_path.display()
                );
                println!("profile_sha256: {}", config.profile.commitment.sha256);
                println!(
                    "profile_source_reported: {}",
                    config
                        .profile
                        .reported_source
                        .as_deref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("run_mode: {:?}", config.control.mode);
                println!("parallel_cap: {}", config.control.parallel_cap.value);
                println!(
                    "parallel_cap_source: {:?}",
                    config.control.parallel_cap.source
                );
                println!("patch_cap: {}", config.control.patch_cap.value);
                println!("patch_cap_source: {:?}", config.control.patch_cap.source);
                if with_version {
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::EvaluationTraceIndex { index } => {
                println!("walk evaluation traces");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("campaign_id: {}", index.campaign);
                println!("instances_root: {}", index.instances_root.display());
                println!("authority: {:?}", index.authority);
                println!("completed_runs: {}", index.runs.len());
                for (position, entry) in index.runs.iter().enumerate() {
                    println!(
                        "run[{position}]: instance={} run_id={} role={:?} updated_at={} registration_sha256={}",
                        entry.coordinate.instance,
                        entry.coordinate.run_id,
                        entry.registration.value.frozen_spec.run_role,
                        entry.registration.value.lifecycle.updated_at,
                        entry.registration.source.content_sha256,
                    );
                    println!("run[{position}].campaign_id: {}", entry.coordinate.campaign);
                }
                if with_version {
                    print_session_version(&index.version);
                    println!("protocol_version: {}", index.epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        index.epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::EvaluationTrace { snapshot } => {
                println!("walk evaluation trace");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("campaign_id: {}", snapshot.coordinate.campaign);
                println!("instance: {}", snapshot.coordinate.instance);
                println!("run_id: {}", snapshot.coordinate.run_id);
                println!("authority: {:?}", snapshot.authority);
                match &snapshot.trace {
                    EvaluationTraceState::NotCompleted { registration } => {
                        println!(
                            "execution_status: {:?}",
                            registration.value.lifecycle.execution_status
                        );
                        println!("trace_state: lifecycle_only");
                        println!("sources: 1");
                        println!(
                            "source[0]: kind={:?} path={} sha256={}",
                            registration.source.kind,
                            registration.source.path.display(),
                            registration.source.content_sha256
                        );
                    }
                    EvaluationTraceState::Completed { trace } => {
                        println!("execution_status: completed");
                        println!("turns: {}", trace.run.value.turn_count());
                        println!("tool_calls: {}", trace.run.value.tool_call_count());
                        println!(
                            "failed_tool_calls: {}",
                            trace.run.value.failed_tool_call_count()
                        );
                        println!("sealed_turn_summary: {}", trace.turn.is_some());
                        println!(
                            "model_exchanges: {}",
                            trace
                                .exchanges
                                .as_ref()
                                .map_or(0, |exchanges| exchanges.value.len())
                        );
                        println!("protocol_artifacts: {}", trace.protocol.len());
                        println!("sources: {}", trace.sources().count());
                        for (position, source) in trace.sources().enumerate() {
                            println!(
                                "source[{position}]: kind={:?} path={} sha256={}",
                                source.kind,
                                source.path.display(),
                                source.content_sha256
                            );
                        }
                    }
                }
                if with_version {
                    print_session_version(&snapshot.version);
                    println!("protocol_version: {}", snapshot.epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        snapshot.epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Job {
                phase,
                job,
                message,
                epoch,
            } => {
                println!("walk");
                println!("{}", "-".repeat(40));
                println!("status: job");
                println!("phase: {phase} - {}", phase.detail());
                println!("job_id: {}", job.job_id);
                println!("operation_id: {}", job.operation_id);
                println!("job_command: {}", job.command);
                println!("job_status: {:?}", job.status);
                if let Some(target) = job.target_phase {
                    println!("job_target: {target}");
                }
                if let Some(watch) = job.watch {
                    println!("job_watch: {watch}");
                }
                if let Some(allowed) = job.allow_live_api {
                    println!("allow_live_api: {allowed}");
                }
                if let Some(allowed) = job.allow_git_changes {
                    println!("allow_git_changes: {allowed}");
                }
                if let Some(source) = job.llm_source {
                    println!("llm_source: {source:?}");
                }
                if let Some(allowed) = job.allow_workspace_mutation {
                    println!("allow_workspace_mutation: {allowed}");
                }
                if let Some(allowed) = job.allow_provenance_record {
                    println!("allow_provenance_record: {allowed}");
                }
                if let Some(receipt) = &job.receipt {
                    let edges = receipt
                        .edges
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("receipt_phase_before: {}", receipt.phase_before);
                    println!("receipt_phase_after: {}", receipt.phase_after);
                    println!("receipt_edges: {edges}");
                    println!(
                        "receipt_journal_revision: {}",
                        receipt.version.journal_revision()
                    );
                    println!("receipt_event_projection: {:?}", receipt.event_projection);
                }
                if let Some(resolution) = &job.resolution {
                    println!("resolution: {:?}", resolution.kind);
                    println!(
                        "resolution_journal_revision: {}",
                        resolution.observed.journal_revision()
                    );
                    println!("resolved_at: {}", resolution.resolved_at);
                }
                print_multiline("message", message);
                if let Some(job_message) = &job.message {
                    print_multiline("job_message", job_message);
                }
                if with_version {
                    print_session_version(&job.expected);
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Status {
                message,
                snapshot,
                epoch,
            } => {
                println!("walk");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                let phase = snapshot.phase();
                println!("phase: {phase} - {}", phase.detail());
                println!("phase_source: {}", snapshot.position.source_label());
                println!("controller_attached: {}", snapshot.controller_attached);
                println!("mutation_authority: {:?}", snapshot.authority);
                if let Some(blocker) = &snapshot.blocker {
                    println!("blocker_code: {:?}", blocker.code);
                    print_multiline("blocker", &blocker.detail);
                }
                if let Some(job) = &snapshot.job {
                    println!("job_id: {}", job.job_id);
                    println!("operation_id: {}", job.operation_id);
                    println!("job_command: {}", job.command);
                    println!("job_status: {:?}", job.status);
                } else {
                    println!("job_status: idle");
                }
                println!("actions:");
                for action in &snapshot.actions {
                    println!(
                        "  {:?} enabled={} edge={} target={} live={} git={} blocker={}",
                        action.kind,
                        action.enabled,
                        action.edge.map_or("-", |edge| edge.id()),
                        action.target.map_or("-", |target| target.as_str()),
                        action.requires_live_api,
                        action.requires_git_changes,
                        action
                            .blocker
                            .map_or_else(|| "-".to_string(), |code| format!("{code:?}"))
                    );
                }
                print_multiline("message", message);
                if with_version {
                    print_session_version(&snapshot.version());
                    println!("protocol_version: {}", epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::History { history } => {
                println!("walk session history");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!(
                    "journal_path: {}",
                    history
                        .journal_path
                        .as_ref()
                        .map_or_else(|| "-".to_string(), |path| path.display().to_string())
                );
                print_session_version(&history.version);
                println!("origin: {:?}", history.origin);
                if let Some(profile) = &history.profile {
                    println!("profile_path: {}", profile.profile_path.display());
                    println!("profile_sha256: {}", profile.sha256);
                    println!("profile_admitted_at: {}", profile.admitted_at);
                } else {
                    println!("profile_path: -");
                }
                println!("damage: {:?}", history.damage);
                if let Some(abandonment) = &history.abandonment {
                    print_multiline("abandonment", &abandonment.detail);
                } else {
                    println!("abandonment: -");
                }
                println!("events: {}", history.events.len());
                for event in &history.events {
                    println!(
                        "event[{}]: recorded_at_ms={} kind={:?}",
                        event.revision, event.recorded_at_ms, event.kind
                    );
                }
                if with_version {
                    println!("protocol_version: {}", history.epoch.protocol_version);
                    println!(
                        "transition_graph_version: {}",
                        history.epoch.transition_graph_version
                    );
                }
            }
            WalkResponse::Delta {
                phase,
                report,
                snapshot,
                epoch,
            } => {
                println!("walk transition delta");
                println!("{}", "-".repeat(40));
                println!("status: ok");
                println!("phase: {phase} - {}", phase.detail());
                match &snapshot.state {
                    WalkDeltaState::NotRecorded => println!("delta: not_recorded"),
                    WalkDeltaState::Recorded { from, edges } => {
                        println!("from: {from}");
                        println!("to: {}", snapshot.version.phase());
                        println!("edges: {}", edges.len());
                        for (index, edge) in edges.iter().enumerate() {
                            println!(
                                "edge[{index}]: {} {} -> {}",
                                edge.edge,
                                edge.from(),
                                edge.to()
                            );
                            for axis in &edge.axes {
                                println!(
                                    "edge[{index}].{}: {} -> {}",
                                    axis.axis.as_str(),
                                    axis.from,
                                    axis.to
                                );
                            }
                        }
                        if with_version {
                            print_session_version(&snapshot.version);
                        }
                    }
                }
                print_multiline("report", report);
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
                version,
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
                    if let Some(version) = version {
                        print_session_version(version);
                    }
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

pub(crate) fn render_response_json(response: &WalkResponse) -> Result<String, PrepareError> {
    if let WalkResponse::Status {
        message,
        snapshot,
        epoch,
    } = response
        && let WalkPosition::Legacy { phase, version } = &snapshot.position
    {
        if epoch.protocol_version >= WALK_PROTOCOL_VERSION {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk protocol {} status cannot emit legacy position authority",
                    epoch.protocol_version
                ),
            });
        }
        let wire = serde_json::json!({
            "type": "status",
            "message": message,
            "snapshot": {
                "phase": phase,
                "version": version,
                "controller_attached": snapshot.controller_attached,
                "authority": snapshot.authority,
                "job": &snapshot.job,
                "blocker": &snapshot.blocker,
                "actions": &snapshot.actions,
            },
            "epoch": epoch,
        });
        return serde_json::to_string_pretty(&wire).map_err(PrepareError::Serialize);
    }
    serde_json::to_string_pretty(response).map_err(PrepareError::Serialize)
}

fn print_session_version(version: &SessionVersion) {
    println!(
        "session_id: {}",
        version
            .session_id
            .map(|session| session.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("session_journal_revision: {}", version.journal_revision);
    if let Some(cursor) = &version.cursor {
        println!("session_cursor: {}", cursor.phase);
        println!("session_evidence: {}", cursor.evidence);
    } else {
        println!("session_cursor: -");
        println!("session_evidence: -");
    }
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
    if let WalkResponse::Ok { result, .. } = response {
        println!("{}", result.text());
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
    match response {
        WalkResponse::Error { code, detail, .. } => Err(PrepareError::InvalidBatchSelection {
            detail: format!("walk request failed ({code}): {detail}"),
        }),
        WalkResponse::Job { job, .. }
            if matches!(
                job.status,
                WalkJobStatus::Failed | WalkJobStatus::Cancelled | WalkJobStatus::Indeterminate
            ) =>
        {
            Err(PrepareError::InvalidBatchSelection {
                detail: job.message.unwrap_or_else(|| {
                    format!("walk job {} ended as {:?}", job.job_id, job.status)
                }),
            })
        }
        _ => Ok(()),
    }
}

fn recovery_result(response: WalkResponse) -> Result<(), PrepareError> {
    match &response {
        WalkResponse::Error { code, detail, .. } => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!("walk recovery failed ({code}): {detail}"),
            });
        }
        WalkResponse::Job { job, .. }
            if matches!(
                job.status,
                WalkJobStatus::Failed | WalkJobStatus::Cancelled | WalkJobStatus::Indeterminate
            ) =>
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: job.message.clone().unwrap_or_else(|| {
                    format!("walk recovery job {} ended as {:?}", job.job_id, job.status)
                }),
            });
        }
        _ => {}
    }
    response_result(response)
}
