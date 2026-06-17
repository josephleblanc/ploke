//! Long-running local typestate walk server.
//!
//! The server binds one Unix socket, owns one `WalkController`, and handles one
//! framed request at a time. Mutating requests pass through the epoch guard so
//! stale binaries do not continue stepping after the checkout changes.

use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, info, warn};

use crate::{cli::Prototype1StateWalkServeCommand, spec::PrepareError};

use super::{
    controller::WalkController,
    epoch::ServerEpoch,
    ipc, paths,
    protocol::{WalkRequest, WalkRequestBody, WalkResponse},
};

/// Runtime state owned by one server process.
struct WalkServer {
    epoch: ServerEpoch,
    controller: WalkController,
}

/// Run the server until it receives `Stop` or the listener fails.
pub(crate) async fn serve(command: Prototype1StateWalkServeCommand) -> Result<(), PrepareError> {
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
        "prototype1-state walk server listening"
    );
    let mut server = WalkServer {
        epoch,
        controller: WalkController::new(),
    };
    let result = accept_loop(&mut server, listener).await;
    if let Err(error) = paths::remove_socket_file(&socket_path) {
        warn!(error = ?error, socket = %socket_path.display(), "failed to remove walk socket after server exit");
    }
    result
}

/// Accept client connections serially and dispatch each request.
async fn accept_loop(server: &mut WalkServer, listener: UnixListener) -> Result<(), PrepareError> {
    loop {
        let (stream, _) =
            listener
                .accept()
                .await
                .map_err(|source| PrepareError::DatabaseSetup {
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
    let (response, stop) = server.handle(request);
    ipc::send(&mut stream, &response).await?;
    Ok(stop)
}

impl WalkServer {
    /// Dispatch one decoded request against the in-memory controller.
    fn handle(&mut self, request: WalkRequest) -> (WalkResponse, bool) {
        let phase = self.controller.phase();
        let stop_requested = matches!(request.body, WalkRequestBody::Stop);
        let result = match request.body {
            WalkRequestBody::Health => Ok(WalkResponse::ok(
                phase,
                self.controller.describe(),
                self.epoch.clone(),
            )),
            WalkRequestBody::Show => Ok(WalkResponse::ok(
                phase,
                self.controller.describe(),
                self.epoch.clone(),
            )),
            WalkRequestBody::Stop => Ok(WalkResponse::ok(
                phase,
                "prototype1-state walk server stopping",
                self.epoch.clone(),
            )),
            WalkRequestBody::Start { config, until } => {
                self.with_epoch_guard(request.client_epoch.as_ref(), |controller| {
                    let phase = controller.start(config, until)?;
                    Ok(format!("started prototype1-state walk at {phase}"))
                })
            }
            WalkRequestBody::Step { until } => {
                self.with_epoch_guard(request.client_epoch.as_ref(), |controller| {
                    let phase = controller.step(until)?;
                    Ok(format!("advanced prototype1-state walk to {phase}"))
                })
            }
        };
        match result {
            Ok(response) if stop_requested => {
                debug!(phase = ?response.phase(), stop = true, "handled prototype1-state walk request");
                (response, true)
            }
            Ok(response) => {
                debug!(phase = ?response.phase(), stop = false, "handled prototype1-state walk request");
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

    /// Run a mutating controller operation after client/server freshness checks.
    fn with_epoch_guard<F>(
        &mut self,
        client_epoch: Option<&ServerEpoch>,
        f: F,
    ) -> Result<WalkResponse, PrepareError>
    where
        F: FnOnce(&mut WalkController) -> Result<String, PrepareError>,
    {
        self.epoch.ensure_compatible_request(client_epoch)?;
        self.epoch.ensure_not_stale_now()?;
        let message = f(&mut self.controller)?;
        Ok(WalkResponse::ok(
            self.controller.phase(),
            message,
            self.epoch.clone(),
        ))
    }
}
