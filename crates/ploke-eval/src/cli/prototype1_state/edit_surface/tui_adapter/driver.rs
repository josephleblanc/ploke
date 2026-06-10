use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::Duration,
};

use ploke_llm::manager::RecordedResponse;
use uuid::Uuid;

use super::super::harness_request::contract;
use super::super::surface_policy::SurfacePolicy;
use super::harness::{SessionSpec, Timeouts, TuiHarness};
use super::harness_io::observed_headless_error;
use super::tui_bridge::{
    AttemptEnd, LiveObserver, attempt_prompt, evidence_read_roots, run_attempt,
    start_attempt_runtime, timeout_terminal_for_run,
};
use super::{
    AttemptOutcome, Budget, Error, Feedback, HeadlessRun, HeadlessTerminal, ModelSelection, NoEdit,
    Outcome, Reject, Step, Terminal,
};

pub(crate) struct AttemptDriver {
    workspace: std::path::PathBuf,
    prompt: String,
    budget: Budget,
    surface: SurfacePolicy,
    evidence: Vec<super::super::harness_request::EvidenceRoot>,
    validation: Vec<contract::Command>,
    model: Option<ModelSelection>,
    response_rx: Option<Arc<Mutex<Receiver<RecordedResponse>>>>,
}

impl AttemptDriver {
    pub(crate) fn new(
        attempt: super::attempt::Attempt,
        response_rx: Option<Arc<Mutex<Receiver<RecordedResponse>>>>,
    ) -> Self {
        Self {
            workspace: attempt.workspace,
            prompt: attempt.prompt,
            budget: attempt.budget,
            surface: attempt.surface,
            evidence: attempt.evidence,
            validation: attempt.validation,
            model: attempt.model,
            response_rx,
        }
    }

    pub(crate) async fn run(self) -> Result<AttemptOutcome, Error> {
        let mut run = HeadlessRun::new();
        run.model_route = self.model.as_ref().map(ModelSelection::model_route_record);
        let mut turn = 1_u32;
        let extra_read_roots = evidence_read_roots(&self.evidence);
        let surface = self.surface.clone();
        let mut next_prompt = attempt_prompt(
            &self.workspace,
            &surface,
            &self.evidence,
            &self.prompt,
            None,
        );
        let observer = LiveObserver::from_env();
        observer.emit(format!(
            "start workspace={} max_attempts={} timeout_secs={} evidence_read_roots={}",
            self.workspace.display(),
            self.budget.max_attempts(),
            self.budget.timeout_secs(),
            extra_read_roots.len()
        ));
        observer.emit_workspace_size("workspace_start", &self.workspace);

        let timeouts = Timeouts::from_budget(self.budget);
        let retry = self.budget.retry();
        let outcome = tokio::time::timeout(Duration::from_secs(timeouts.attempt_secs), async {
            loop {
                observer.emit(format!("attempt {turn} start"));
                let (runtime, parent_id) = start_attempt_runtime(
                    &self.workspace,
                    &extra_read_roots,
                    next_prompt.clone(),
                    &surface,
                    self.model.as_ref(),
                    &timeouts,
                )
                .await?;
                let (end, _runtime) = run_attempt(
                    runtime,
                    parent_id,
                    &self.workspace,
                    &surface,
                    turn,
                    &mut run,
                    &observer,
                    &self.validation,
                    self.response_rx.as_ref().map(Arc::clone),
                    timeouts,
                )
                .await?;

                if let AttemptEnd::Terminal(terminal) = end {
                    observer.emit(format!("terminal {}", terminal.live_summary()));
                    return Ok::<HeadlessTerminal, Error>(terminal);
                }

                let retry_outcome = retry_outcome_from_end(&end)?;
                match retry.decide_outcome(turn, &retry_outcome) {
                    Step::Retry {
                        next_attempt,
                        feedback,
                    } => {
                        observer.emit(format!(
                            "retry attempt={next_attempt} feedback={}",
                            feedback.message()
                        ));
                        turn = next_attempt;
                        next_prompt = attempt_prompt(
                            &self.workspace,
                            &surface,
                            &self.evidence,
                            &self.prompt,
                            Some(feedback.message()),
                        );
                    }
                    Step::Terminal(Terminal::Exhausted { attempts, last }) => {
                        let terminal = exhausted_terminal(end, attempts, &last);
                        observer.emit(format!("terminal {}", terminal.live_summary()));
                        return Ok(terminal);
                    }
                    Step::Terminal(Terminal::Accepted(_))
                    | Step::Terminal(Terminal::Invalid { .. }) => {
                        return Err(Error::HeadlessEvent(
                            "retry policy returned a terminal step for a retryable attempt end"
                                .to_string(),
                        ));
                    }
                }
            }
        })
        .await;

        let terminal = match outcome {
            Ok(Ok(terminal)) => terminal,
            Ok(Err(source)) => {
                if !run.has_observed_activity() {
                    return Err(source);
                }
                HeadlessTerminal::ToolFailed {
                    error: observed_headless_error(source),
                }
            }
            Err(_) => timeout_terminal_for_run(&run, self.budget.timeout_secs()),
        };
        observer.emit(format!("done {}", terminal.live_summary()));
        observer.emit_workspace_size("workspace_done", &self.workspace);
        run.terminal = Some(terminal.clone());
        Ok(AttemptOutcome { run, terminal })
    }
}

fn retry_outcome_from_end(end: &AttemptEnd) -> Result<Outcome, Error> {
    match end {
        AttemptEnd::RetryFailure(feedback) => Ok(Outcome::Rejected(Reject::Invalid {
            reason: super::tui_bridge::retry_feedback(feedback),
        })),
        AttemptEnd::RetryNoEdit { feedback, .. } => Ok(Outcome::NoEdit(NoEdit::new(
            super::tui_bridge::retry_feedback(feedback),
        ))),
        AttemptEnd::Terminal(_) => Err(Error::HeadlessEvent(
            "terminal attempt end is not retryable".to_string(),
        )),
    }
}

fn exhausted_terminal(end: AttemptEnd, attempts: u32, last: &Outcome) -> HeadlessTerminal {
    if let AttemptEnd::RetryNoEdit {
        outcome, summary, ..
    } = end
    {
        return HeadlessTerminal::CompletedWithoutEdit { outcome, summary };
    }
    HeadlessTerminal::Exhausted {
        attempts,
        last: Feedback::from_outcome(last).message().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use uuid::Uuid;

    use super::super::harness::{
        Batch, Decision, DenyItem, Harness, Progress, SessionSpec, Staged, StagedKind, TurnStop,
    };
    use super::super::harness_io::{HeadlessRun, PromptDiagnostic};
    use super::super::{Budget, Error};
    use super::*;
    use crate::cli::prototype1_state::edit_surface::surface_policy::SurfacePolicy;

    struct FixtureHarness {
        run: HeadlessRun,
        stops: std::collections::VecDeque<AttemptEnd>,
    }

    impl FixtureHarness {
        fn retry_no_edit() -> Self {
            Self {
                run: HeadlessRun::new(),
                stops: std::collections::VecDeque::from([AttemptEnd::RetryNoEdit {
                    feedback: "no edit produced".to_string(),
                    outcome: "no_edit".to_string(),
                    summary: "model completed without staging an edit".to_string(),
                }]),
            }
        }
    }

    impl Harness for FixtureHarness {
        async fn next(&mut self, _deadline: Instant) -> Result<Progress, Error> {
            Err(Error::HeadlessEvent(
                "fixture harness does not stream events".to_string(),
            ))
        }

        async fn decide(&mut self, _decision: Decision) -> Result<bool, Error> {
            Ok(false)
        }

        async fn settle(
            &mut self,
            _deadline: Instant,
        ) -> Result<super::super::harness::Settled, Error> {
            Ok(super::super::harness::Settled {
                applied: Vec::new(),
                changed_paths: Vec::new(),
            })
        }

        fn run(&self) -> &HeadlessRun {
            &self.run
        }

        fn run_mut(&mut self) -> &mut HeadlessRun {
            &mut self.run
        }
    }

    impl FixtureHarness {
        fn pop_end(&mut self) -> AttemptEnd {
            self.stops
                .pop_front()
                .unwrap_or_else(|| AttemptEnd::Terminal(HeadlessTerminal::NoEdit))
        }
    }

    #[test]
    fn retry_decide_exhausts_no_edit_into_completed_without_edit() {
        let end = AttemptEnd::RetryNoEdit {
            feedback: "no edit".to_string(),
            outcome: "no_edit".to_string(),
            summary: "nothing staged".to_string(),
        };
        let outcome = retry_outcome_from_end(&end).expect("retry outcome");
        let step = Budget::new(1, 30)
            .expect("budget")
            .retry()
            .decide_outcome(1, &outcome);
        let Step::Terminal(Terminal::Exhausted { .. }) = step else {
            panic!("expected exhausted terminal step, got {step:?}");
        };
        let terminal = exhausted_terminal(end, 1, &outcome);
        assert!(matches!(
            terminal,
            HeadlessTerminal::CompletedWithoutEdit { .. }
        ));
    }

    #[test]
    fn fixture_harness_is_constructible_without_workspace_runtime() {
        let mut harness = FixtureHarness::retry_no_edit();
        assert!(harness.stops.len() == 1);
        let _ = harness.pop_end();
    }

    // W1 regression: a no-edit turn that still retries must carry the curated
    // `feedback` (e.g. policy-rejection detail) into the next prompt, not the
    // generic turn `summary`. The old hand-rolled loop used
    // `retry_feedback(&feedback)`; the driver must preserve that contract.
    #[test]
    fn retry_no_edit_uses_curated_feedback_not_summary_for_next_prompt() {
        let curated = "Rejected protected paths: crates/ploke-eval/src/lib.rs";
        let summary = "the model completed the turn without staging an edit";
        let end = AttemptEnd::RetryNoEdit {
            feedback: curated.to_string(),
            outcome: "no_edit".to_string(),
            summary: summary.to_string(),
        };
        let outcome = retry_outcome_from_end(&end).expect("retry outcome");
        // max_attempts >= 2 so the no-edit turn retries instead of exhausting.
        let step = Budget::new(2, 30)
            .expect("budget")
            .retry()
            .decide_outcome(1, &outcome);
        let Step::Retry { feedback, .. } = step else {
            panic!("expected retry step for non-exhausted no-edit turn, got {step:?}");
        };
        let expected = super::super::tui_bridge::retry_feedback(curated);
        assert_eq!(
            feedback.message(),
            expected,
            "retry prompt must use the curated feedback, not the turn summary"
        );
        assert_ne!(
            feedback.message(),
            summary,
            "retry prompt must not regress to the generic turn summary"
        );
    }
}
