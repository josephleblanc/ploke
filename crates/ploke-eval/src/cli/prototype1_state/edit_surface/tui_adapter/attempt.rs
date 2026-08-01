use std::path::PathBuf;

use super::super::harness_request::{EvidenceRoot, contract};
use super::super::surface_policy::SurfacePolicy;
use super::{AttemptDriver, AttemptOutcome, Budget, Error, HeadlessRun, ModelSelection};
use crate::replay::tool_loop::OuterAttemptLink;

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
    /// Optional prompt-side bias string, appended to every chat-step prompt
    /// in this attempt. **Read-side / prompt-side only.** The anti-attractor
    /// policy uses this to nudge the LLM away from recently-touched edit
    /// surfaces; the value never influences selection, admission, replay,
    /// oracle, or protocol authority. `None` means no bias is applied.
    ///
    /// See [`crate::cli::prototype1_state::profile::prompt_suffix_for`] for
    /// the only sanctioned builder; callers should not hand-construct
    /// arbitrary suffixes.
    pub policy_suffix: Option<String>,
}

impl Attempt {
    pub(crate) async fn run(self) -> Result<AttemptOutcome, Error> {
        self.run_with_outer_attempt(OuterAttemptLink::Unlinked)
            .await
    }

    pub(crate) async fn run_with_outer_attempt(
        self,
        outer_attempt: OuterAttemptLink,
    ) -> Result<AttemptOutcome, Error> {
        let (response_rx, session_capture) = match self.capture {
            Capture::Off => (None, ploke_tui::llm::SessionCapture::default()),
            Capture::Responses => {
                let (response_tx, response_rx) = std::sync::mpsc::channel();
                let response_rx = std::sync::Arc::new(std::sync::Mutex::new(response_rx));
                let debug_sink = super::tool_loop_debug::sink_for_attempt(
                    &self.workspace,
                    self.model.as_ref(),
                    &self.evidence,
                    outer_attempt,
                );
                let capture = ploke_tui::llm::SessionCapture::new(Some(response_tx), debug_sink);
                (Some(response_rx), capture)
            }
        };
        AttemptDriver::new(self, response_rx, session_capture)
            .run()
            .await
    }
}

impl Attempt {
    pub(crate) fn into_headless_run(outcome: AttemptOutcome) -> HeadlessRun {
        outcome.run
    }
}
