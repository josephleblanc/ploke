//! Long-running local typestate walk server.
//!
//! The server binds one Unix socket, owns one `WalkController`, and handles one
//! framed request at a time. Mutating requests pass through the epoch guard so
//! stale binaries do not continue stepping after the checkout changes.

use std::time::Duration;

use tokio::{
    net::{UnixListener, UnixStream},
    time,
};
use tracing::{debug, info, warn};

use crate::{cli::Prototype1StateWalkServeCommand, spec::PrepareError};

use super::{
    controller::{DeltaRenderStyle, WalkController},
    epoch::ServerEpoch,
    ipc, paths,
    phase::WalkPhase,
    protocol::{WalkRequest, WalkRequestBody, WalkResponse},
};

/// Runtime state owned by one server process.
struct WalkServer {
    epoch: ServerEpoch,
    controller: WalkController,
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
    let mut server = WalkServer {
        epoch,
        controller: WalkController::new(repo_root.clone()),
    };
    let result = accept_loop(&mut server, listener, idle_ttl).await;
    if let Err(error) = paths::remove_socket_file(&socket_path) {
        warn!(error = ?error, socket = %socket_path.display(), "failed to remove walk socket after server exit");
    }
    result
}

/// Accept client connections serially and dispatch each request.
async fn accept_loop(
    server: &mut WalkServer,
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
        let stop = handle_stream(server, stream).await?;
        if stop {
            return Ok(());
        }
    }
}

/// Read one request from a connected socket and write one response.
async fn handle_stream(
    server: &mut WalkServer,
    mut stream: UnixStream,
) -> Result<bool, PrepareError> {
    let request: WalkRequest = match ipc::recv(&mut stream).await {
        Ok(request) => request,
        Err(error) => {
            let response = WalkResponse::error(
                "bad_request",
                error.to_string(),
                Some(server.controller.phase()),
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
    async fn handle(&mut self, request: WalkRequest) -> (WalkResponse, bool) {
        let phase = self.controller.phase();
        let stop_requested = matches!(request.body, WalkRequestBody::Stop);
        let result = match request.body {
            WalkRequestBody::Health => {
                Ok(WalkResponse::ok(phase, self.describe(), self.epoch.clone()))
            }
            WalkRequestBody::Show => self.controller.refresh_from_disk().map(|_| {
                WalkResponse::ok(self.controller.phase(), self.describe(), self.epoch.clone())
            }),
            WalkRequestBody::ShowDelta { verbose, color } => Ok(WalkResponse::ok(
                phase,
                self.controller
                    .delta_report(DeltaRenderStyle { verbose, color }),
                self.epoch.clone(),
            )),
            WalkRequestBody::LlmLanes { verbose } => self
                .controller
                .llm_lanes_report(verbose)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::LlmFocus { lane } => self
                .controller
                .llm_focus(lane)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::LlmShow {
                session_id,
                lane,
                head,
                step,
            } => self
                .controller
                .llm_report(session_id.as_deref(), lane.as_deref(), head, step)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::LlmTimeline { session_id, lane } => self
                .controller
                .llm_timeline(session_id.as_deref(), lane.as_deref())
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
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
                Ok(()) => self
                    .controller
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
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
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
                Ok(()) => self
                    .controller
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
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
                Err(error) => Err(error),
            },
            WalkRequestBody::LlmBack { lane, steps } => self
                .controller
                .llm_move(lane.as_deref(), steps, super::controller::LlmMove::Back)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::LlmForward { lane, steps } => self
                .controller
                .llm_move(lane.as_deref(), steps, super::controller::LlmMove::Forward)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::LlmHead { lane } => self
                .controller
                .llm_head(lane.as_deref())
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::Replay { index, tail } => self
                .controller
                .replay_report(index, tail)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::ReplayBack { steps, tail } => self
                .controller
                .replay_back(steps, tail)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::ReplayForward { steps, tail } => self
                .controller
                .replay_forward(steps, tail)
                .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
            WalkRequestBody::BranchLive {
                reason,
                allow_provenance_record,
            } => match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                Ok(()) if allow_provenance_record => self
                    .controller
                    .record_replay_branch(reason)
                    .map(|message| WalkResponse::ok(phase, message, self.epoch.clone())),
                Ok(()) => Err(PrepareError::InvalidBatchSelection {
                    detail: "walk branch-live writes a provenance record; rerun with `--allow provenance-record`"
                        .to_string(),
                }),
                Err(error) => Err(error),
            },
            WalkRequestBody::Stop => Ok(WalkResponse::ok(
                phase,
                "walk server stopping",
                self.epoch.clone(),
            )),
            WalkRequestBody::Start { config, until } => {
                match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                    Ok(()) => match self.controller.start(config, until).await {
                        Ok(phase) => Ok(WalkResponse::ok(
                            self.controller.phase(),
                            format!("started walk at {phase} - {}", phase.detail()),
                            self.epoch.clone(),
                        )),
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(error),
                }
            }
            WalkRequestBody::Step {
                until,
                watch,
                allow_git_changes,
            } => match self.ensure_epoch_guard(request.client_epoch.as_ref()) {
                Ok(()) => match self.controller.step(until, watch, allow_git_changes).await {
                    Ok(report) => Ok(WalkResponse::ok(
                        self.controller.phase(),
                        report.render(),
                        self.epoch.clone(),
                    )),
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
            },
            WalkRequestBody::Reset => {
                self.with_epoch_guard(request.client_epoch.as_ref(), |controller| {
                    let phase = controller.reset();
                    Ok(format!("reset walk to {phase} - {}", phase.detail()))
                })
            }
            WalkRequestBody::Files => Ok(WalkResponse::ok(
                phase,
                self.controller.files_report(),
                self.epoch.clone(),
            )),
        };
        match result {
            Ok(response) if stop_requested => {
                debug!(phase = ?response.phase(), stop = true, "handled walk request");
                (response, true)
            }
            Ok(response) if response.is_ok() && response.phase() == Some(WalkPhase::R14b) => {
                debug!(
                    phase = ?response.phase(),
                    stop = true,
                    "handled walk request; stopping parent walk server after final handoff report"
                );
                (response, true)
            }
            Ok(response) => {
                debug!(phase = ?response.phase(), stop = false, "handled walk request");
                (response, false)
            }
            Err(error) => (
                WalkResponse::error(
                    "request_failed",
                    error.to_string(),
                    Some(self.controller.phase()),
                    self.epoch.clone(),
                ),
                false,
            ),
        }
    }

    fn describe(&self) -> String {
        format!(
            "server_pid={}\n{}",
            std::process::id(),
            self.controller.describe()
        )
    }

    fn ensure_epoch_guard(
        &mut self,
        client_epoch: Option<&ServerEpoch>,
    ) -> Result<(), PrepareError> {
        self.epoch.ensure_compatible_request(client_epoch)?;
        self.epoch.ensure_not_stale_now()
    }

    /// Run a mutating controller operation after client/server freshness checks.
    fn with_epoch_guard<F>(
        &mut self,
        client_epoch: Option<&ServerEpoch>,
        f: F,
    ) -> Result<WalkResponse, PrepareError>
    where
        F: FnOnce(&mut WalkController) -> Result<String, PrepareError>,
    {
        self.ensure_epoch_guard(client_epoch)?;
        let message = f(&mut self.controller)?;
        Ok(WalkResponse::ok(
            self.controller.phase(),
            message,
            self.epoch.clone(),
        ))
    }
}
