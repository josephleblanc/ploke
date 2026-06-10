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
    pub(crate) async fn run(mut self) -> Result<AttemptOutcome, Error> {
        // The response tap is process-global; its RAII guard must outlive the
        // whole attempt run. Binding it in this function scope (not inside the
        // match arm) keeps it installed across the `.await` below.
        let (response_rx, _response_tap_guard) = match self.capture {
            Capture::Off => (None, None),
            Capture::Responses => {
                let (response_tx, response_rx) = std::sync::mpsc::channel();
                let response_rx = std::sync::Arc::new(std::sync::Mutex::new(response_rx));
                let guard = ploke_tui::llm::install_response_tap(response_tx);
                (Some(response_rx), Some(guard))
            }
        };
        // Capture the suffix via `mem::take` so we can still move the rest of
        // `self` into `AttemptDriver::new`. The default of `None` after take
        // does not matter because `self` is consumed.
        let policy_suffix = self.policy_suffix.take();
        let driver = AttemptDriver::new(self, response_rx, policy_suffix.as_deref());
        driver.run().await
    }
}

impl Attempt {
    pub(crate) fn into_headless_run(outcome: AttemptOutcome) -> HeadlessRun {
        outcome.run
    }
}
