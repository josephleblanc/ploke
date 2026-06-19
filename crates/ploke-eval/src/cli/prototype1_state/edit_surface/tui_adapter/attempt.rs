use std::path::PathBuf;

use super::super::harness_request::{EvidenceRoot, contract};
use super::super::surface_policy::SurfacePolicy;
use super::{AttemptDriver, AttemptOutcome, Budget, Error, HeadlessRun, ModelSelection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capture {
    Off,
    Responses,
}

pub(crate) struct Attempt {
    pub workspace: PathBuf,
    pub prompt: String,
    pub budget: Budget,
    pub surface: SurfacePolicy,
    pub evidence: Vec<EvidenceRoot>,
    pub validation: Vec<contract::Command>,
    pub model: Option<ModelSelection>,
    pub capture: Capture,
}

impl Attempt {
    pub(crate) async fn run(self) -> Result<AttemptOutcome, Error> {
        // The response tap is process-global; its RAII guard must outlive the
        // whole attempt run. Binding it in this function scope (not inside the
        // match arm) keeps it installed across the `.await` below.
        let (response_rx, _response_tap_guard, _debug_guard) = match self.capture {
            Capture::Off => (None, None, None),
            Capture::Responses => {
                let (response_tx, response_rx) = std::sync::mpsc::channel();
                let response_rx = std::sync::Arc::new(std::sync::Mutex::new(response_rx));
                let guard = ploke_tui::llm::install_response_tap(response_tx);
                let debug_guard = super::tool_loop_debug::install_for_attempt(
                    &self.workspace,
                    self.model.as_ref(),
                    &self.evidence,
                );
                (Some(response_rx), Some(guard), debug_guard)
            }
        };
        AttemptDriver::new(self, response_rx).run().await
    }
}

impl Attempt {
    pub(crate) fn into_headless_run(outcome: AttemptOutcome) -> HeadlessRun {
        outcome.run
    }
}
