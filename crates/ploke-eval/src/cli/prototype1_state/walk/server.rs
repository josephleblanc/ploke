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
    collections::BTreeSet, fs, path::PathBuf, process::Command as ProcessCommand, sync::Arc,
    time::Duration,
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
    cli::{
        Prototype1StateWalkServeCommand,
        prototype1_state::{
            identity,
            journal::{JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
        },
    },
    layout::campaigns_dir,
    spec::PrepareError,
};

use super::{
    controller::{DeltaRenderStyle, WalkController},
    epoch::ServerEpoch,
    ipc, paths,
    phase::WalkPhase,
    protocol::{
        WalkJobSnapshot, WalkJobStatus, WalkRequest, WalkRequestBody, WalkResponse, WalkStartConfig,
    },
};

/// Runtime state owned by one server process.
struct WalkServer {
    epoch: ServerEpoch,
    controller: Arc<Mutex<WalkController>>,
    jobs: Arc<Mutex<JobRegistry>>,
}

#[derive(Default)]
struct JobRegistry {
    next_id: u64,
    active: Option<ActiveWalkJob>,
}

struct ActiveWalkJob {
    snapshot: WalkJobSnapshot,
    handle: Option<JoinHandle<()>>,
}

struct WalkEventInput {
    command: &'static str,
    phase_before: Option<WalkPhase>,
    phase_after: WalkPhase,
    target_phase: Option<WalkPhase>,
    watch: Option<bool>,
    allow_git_changes: Option<bool>,
    transitions: Vec<String>,
}

/// Run the server until it receives `Stop`, the listener fails, or idle TTL expires.
pub(crate) async fn serve(command: Prototype1StateWalkServeCommand) -> Result<(), PrepareError> {
    let idle_ttl = command.idle_ttl()?;
    let repo_root = paths::resolve_repo_root(command.repo_root.as_deref())?;
    let socket_path = paths::socket_path(&repo_root, command.socket.as_deref())?;
    paths::ensure_socket_parent(&socket_path)?;
    paths::remove_socket_file(&socket_path)?;
    let listener =
        UnixListener::bind(&socket_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_bind",
            detail: format!(
                "failed to bind walk socket '{}': {source}",
                socket_path.display()
            ),
        })?;
    let epoch = ServerEpoch::capture(&repo_root)?;
    info!(
        socket = %socket_path.display(),
        repo_root = %repo_root.display(),
        idle_ttl_secs = ?idle_ttl.map(|ttl| ttl.as_secs()),
        "walk server listening"
    );
    let server = WalkServer {
        epoch,
        controller: Arc::new(Mutex::new(WalkController::new(repo_root.clone()))),
        jobs: Arc::new(Mutex::new(JobRegistry::default())),
    };
    let result = accept_loop(server, listener, idle_ttl).await;
    if let Err(error) = paths::remove_socket_file(&socket_path) {
        warn!(error = ?error, socket = %socket_path.display(), "failed to remove walk socket after server exit");
    }
    result
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
        let result = match request.body {
            WalkRequestBody::Health => Ok(self.status_response("walk server online").await),
            WalkRequestBody::Show => {
                let mut controller = self.controller.lock().await;
                controller.refresh_from_disk().map(|_| {
                    WalkResponse::ok(
                        controller.phase(),
                        self.describe_locked(&controller),
                        self.epoch.clone(),
                    )
                })
            }
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
            WalkRequestBody::LlmStep {
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
            } => match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                Ok(()) => {
                    let mut controller = self.controller.lock().await;
                    let phase = controller.phase();
                    controller
                        .llm_step(
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
                        .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
                }
                Err(error) => Err(error),
            },
            WalkRequestBody::LlmFinish {
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
            } => match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                Ok(()) => {
                    let mut controller = self.controller.lock().await;
                    let phase = controller.phase();
                    controller
                        .llm_finish(
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
                        .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
                }
                Err(error) => Err(error),
            },
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
                    .llm_move(
                        lane.as_deref(),
                        steps,
                        super::controller::LlmMove::Forward,
                    )
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
            WalkRequestBody::BranchLive {
                reason,
                allow_provenance_record,
            } => match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                Ok(()) if allow_provenance_record => {
                    let mut controller = self.controller.lock().await;
                    let phase = controller.phase();
                    controller
                        .record_replay_branch(reason)
                        .map(|message| WalkResponse::ok(phase, message, self.epoch.clone()))
                }
                Ok(()) => Err(PrepareError::InvalidBatchSelection {
                    detail: "walk branch-live writes a provenance record; rerun with `--allow provenance-record`"
                        .to_string(),
                }),
                Err(error) => Err(error),
            },
            WalkRequestBody::Stop => Ok(self.stop_active_job().await),
            WalkRequestBody::Start { config, until } => {
                self.submit_start(request.client_epoch.as_ref(), config, until)
                    .await
            }
            WalkRequestBody::Step {
                until,
                watch,
                allow_git_changes,
            } => {
                self.submit_step(
                    request.client_epoch.as_ref(),
                    until,
                    watch,
                    allow_git_changes,
                )
                .await
            }
            WalkRequestBody::Reset => {
                self.with_epoch_guard(request.client_epoch.as_ref(), "reset", |controller| {
                    let phase = controller.reset();
                    Ok(format!("reset walk to {phase} - {}", phase.detail()))
                })
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
                        "request_failed",
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
        config: WalkStartConfig,
        until: WalkPhase,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        if let Some(job) = self.active_job().await {
            return Ok(WalkResponse::job(
                job_phase(&job),
                job,
                "walk job already running; no new start submitted",
                self.epoch.clone(),
            ));
        }
        let phase_before = self.phase_for_response().await;
        let job = self
            .register_job("start", phase_before, Some(until), None, None)
            .await;
        let controller = Arc::clone(&self.controller);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let job_id = job.job_id;
        let handle = tokio::spawn(run_start_job(
            controller, jobs, epoch, job_id, config, until,
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
        until: Option<WalkPhase>,
        watch: bool,
        allow_git_changes: bool,
    ) -> Result<WalkResponse, PrepareError> {
        self.ensure_epoch_guard(client_epoch)?;
        if let Some(job) = self.active_job().await {
            return Ok(WalkResponse::job(
                job_phase(&job),
                job,
                "walk job already running; no new step submitted",
                self.epoch.clone(),
            ));
        }
        let phase_before = self.phase_for_response().await;
        let job = self
            .register_job(
                "step",
                phase_before,
                until,
                Some(watch),
                Some(allow_git_changes),
            )
            .await;
        let controller = Arc::clone(&self.controller);
        let jobs = Arc::clone(&self.jobs);
        let epoch = self.epoch.clone();
        let job_id = job.job_id;
        let handle = tokio::spawn(run_step_job(
            controller,
            jobs,
            epoch,
            job_id,
            until,
            watch,
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

    async fn register_job(
        &self,
        command: &'static str,
        phase_before: WalkPhase,
        target_phase: Option<WalkPhase>,
        watch: Option<bool>,
        allow_git_changes: Option<bool>,
    ) -> WalkJobSnapshot {
        let mut jobs = self.jobs.lock().await;
        jobs.next_id += 1;
        let now = now_rfc3339();
        let snapshot = WalkJobSnapshot {
            job_id: jobs.next_id,
            command: command.to_string(),
            status: WalkJobStatus::Running,
            phase_before,
            phase_after: None,
            target_phase,
            watch,
            allow_git_changes,
            started_at: now.clone(),
            updated_at: now,
            finished_at: None,
            message: None,
        };
        jobs.active = Some(ActiveWalkJob {
            snapshot: snapshot.clone(),
            handle: None,
        });
        snapshot
    }

    async fn attach_job_handle(&self, job_id: u64, handle: JoinHandle<()>) {
        let mut jobs = self.jobs.lock().await;
        if let Some(active) = jobs
            .active
            .as_mut()
            .filter(|active| active.snapshot.job_id == job_id)
        {
            active.handle = Some(handle);
        } else {
            handle.abort();
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

    async fn status_response(&self, heading: &str) -> WalkResponse {
        let job = self.latest_job().await;
        let (phase, controller_summary) = match job.as_ref() {
            Some(job) if job.status.is_active() => {
                let summary = self
                    .controller
                    .try_lock()
                    .ok()
                    .map(|controller| controller.describe());
                (job_phase(job), summary)
            }
            _ => {
                let controller = self.controller.lock().await;
                (controller.phase(), Some(controller.describe()))
            }
        };
        let message = self.render_status_message(heading, job.as_ref(), controller_summary);
        WalkResponse::status(phase, message, job, self.epoch.clone())
    }

    async fn stop_active_job(&self) -> WalkResponse {
        let cancelled = {
            let mut jobs = self.jobs.lock().await;
            if let Some(active) = jobs
                .active
                .as_mut()
                .filter(|active| active.snapshot.status.is_active())
            {
                active.snapshot.status = WalkJobStatus::CancelRequested;
                active.snapshot.updated_at = now_rfc3339();
                if let Some(handle) = active.handle.take() {
                    handle.abort();
                }
                active.snapshot.status = WalkJobStatus::Cancelled;
                active.snapshot.updated_at = now_rfc3339();
                active.snapshot.finished_at = Some(now_rfc3339());
                active.snapshot.message = Some("cancelled by walk stop".to_string());
                Some(active.snapshot.clone())
            } else {
                None
            }
        };

        let cleanup = terminate_recorded_processes(&self.epoch).unwrap_or_else(|error| {
            vec![format!(
                "process cleanup skipped after journal read failure: {error}"
            )]
        });
        let phase = match cancelled.as_ref() {
            Some(job) => job_phase(job),
            None => self.controller.lock().await.phase(),
        };
        let mut message =
            self.render_status_message("walk server stopping", cancelled.as_ref(), None);
        for line in cleanup {
            message.push('\n');
            message.push_str(&line);
        }
        WalkResponse::status(phase, message, cancelled, self.epoch.clone())
    }

    fn render_status_message(
        &self,
        heading: &str,
        job: Option<&WalkJobSnapshot>,
        controller_summary: Option<String>,
    ) -> String {
        let mut lines = vec![
            format!("server_pid={}", std::process::id()),
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

    /// Run a mutating controller operation after client/server freshness checks.
    async fn with_epoch_guard<F>(
        &self,
        client_epoch: Option<&ServerEpoch>,
        command: &'static str,
        f: F,
    ) -> Result<WalkResponse, PrepareError>
    where
        F: FnOnce(&mut WalkController) -> Result<String, PrepareError>,
    {
        self.ensure_epoch_guard(client_epoch)?;
        if let Some(job) = self.active_job().await {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk {command} refused because job {} ({}) is still {:?}",
                    job.job_id, job.command, job.status
                ),
            });
        }
        let mut controller = self.controller.lock().await;
        let message = f(&mut controller)?;
        Ok(WalkResponse::ok(
            controller.phase(),
            message,
            self.epoch.clone(),
        ))
    }
}

async fn run_start_job(
    controller: Arc<Mutex<WalkController>>,
    jobs: Arc<Mutex<JobRegistry>>,
    epoch: ServerEpoch,
    job_id: u64,
    config: WalkStartConfig,
    until: WalkPhase,
) {
    let result = {
        let mut controller = controller.lock().await;
        match controller.start(config, until).await {
            Ok(report) => {
                let phase = controller.phase();
                let event = WalkEventInput {
                    command: "start",
                    phase_before: Some(report.from()),
                    phase_after: report.to(),
                    target_phase: Some(until),
                    watch: None,
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
    until: Option<WalkPhase>,
    client_watch: bool,
    allow_git_changes: bool,
) {
    let result = {
        let mut controller = controller.lock().await;
        // `WalkController::step` still uses this boolean as live-edge
        // admission. The socket-level `--watch` flag is now client follow
        // behavior, so server-submitted jobs must admit the long edge here and
        // let the client decide whether to wait for the job status to settle.
        match controller.step(until, true, allow_git_changes).await {
            Ok(report) => {
                let phase = controller.phase();
                let message = report.render();
                let event = WalkEventInput {
                    command: "step",
                    phase_before: Some(report.from()),
                    phase_after: report.to(),
                    target_phase: until,
                    watch: Some(client_watch),
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
    active.snapshot.status = status;
    active.snapshot.phase_after = phase_after;
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
    pid: u32,
    binary_path: PathBuf,
    invocation_path: Option<PathBuf>,
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
        if !recorded_process_still_matches(&process) {
            messages.push(format!(
                "process_cleanup: skipped pid {} ({}) because /proc did not match recorded invocation",
                process.pid, process.kind
            ));
            continue;
        }
        match terminate_process_group(process.pid) {
            Ok(status) if status.success() => messages.push(format!(
                "process_cleanup: sent SIGTERM to process group {} ({})",
                process.pid, process.kind
            )),
            Ok(status) => messages.push(format!(
                "process_cleanup: kill -TERM -{} ({}) exited with {}",
                process.pid, process.kind, status
            )),
            Err(error) => messages.push(format!(
                "process_cleanup: failed to signal process group {} ({}): {}",
                process.pid, process.kind, error
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
                if let Some(pid) = entry.child_pid {
                    let key = ("child", pid);
                    if seen.insert(key) {
                        processes.push(RecordedProcess {
                            kind: "child",
                            pid,
                            binary_path: entry.paths.binary_path,
                            invocation_path: invocation_path_from_argv(&entry.argv),
                        });
                    }
                }
            }
            JournalEntry::SuccessorHandoff(entry) => {
                let key = ("successor", entry.pid);
                if seen.insert(key) {
                    processes.push(RecordedProcess {
                        kind: "successor",
                        pid: entry.pid,
                        binary_path: entry.binary_path,
                        invocation_path: Some(entry.invocation_path),
                    });
                }
            }
            JournalEntry::ParentStarted(_)
            | JournalEntry::Resource(_)
            | JournalEntry::ChildArtifactCommitted(_)
            | JournalEntry::ActiveCheckoutAdvanced(_)
            | JournalEntry::Successor(_)
            | JournalEntry::MaterializeBranch(_)
            | JournalEntry::BuildChild(_)
            | JournalEntry::Child(_)
            | JournalEntry::ChildReady(_)
            | JournalEntry::ObserveChild(_) => {}
        }
    }
    processes
}

fn invocation_path_from_argv(argv: &[String]) -> Option<PathBuf> {
    argv.windows(2)
        .find(|window| window[0] == "--invocation")
        .map(|window| PathBuf::from(&window[1]))
}

fn recorded_process_still_matches(process: &RecordedProcess) -> bool {
    let Some(cmdline) = process_cmdline(process.pid) else {
        return false;
    };
    let binary_path = process.binary_path.display().to_string();
    let invocation_matches = process
        .invocation_path
        .as_ref()
        .map(|path| cmdline.contains(&path.display().to_string()))
        .unwrap_or(false);
    cmdline.contains(&binary_path) || invocation_matches
}

fn process_cmdline(pid: u32) -> Option<String> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(
        bytes
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(unix)]
fn terminate_process_group(pid: u32) -> std::io::Result<std::process::ExitStatus> {
    ProcessCommand::new("kill")
        .arg("-TERM")
        .arg(format!("-{pid}"))
        .status()
}

#[cfg(not(unix))]
fn terminate_process_group(pid: u32) -> std::io::Result<std::process::ExitStatus> {
    ProcessCommand::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status()
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
    if let Some(message) = &job.message {
        lines.push(format!("job_message={message}"));
    }
    lines.join("\n")
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
