use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::Duration,
};

use ploke_llm::manager::RecordedResponse;

use super::super::harness_request::contract;
use super::super::surface_policy::SurfacePolicy;
use super::harness::Timeouts;
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
        let mut protected_recovery_used = false;
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
                        if !protected_recovery_used
                            && let Some(feedback) = protected_write_denial_feedback(&last)
                        {
                            match workspace_is_clean(&self.workspace) {
                                Ok(true) => {
                                    protected_recovery_used = true;
                                    turn = attempts.saturating_add(1);
                                    let recovery = protected_write_recovery_feedback(
                                        &self.workspace,
                                        &surface,
                                        &feedback,
                                    );
                                    observer.emit(format!(
                                        "protected_write_recovery attempt={} feedback={}",
                                        turn,
                                        truncate_inline(&feedback, 240)
                                    ));
                                    next_prompt = attempt_prompt(
                                        &self.workspace,
                                        &surface,
                                        &self.evidence,
                                        &self.prompt,
                                        Some(&recovery),
                                    );
                                    continue;
                                }
                                Ok(false) => observer.emit(
                                    "protected_write_recovery skipped: workspace is not clean"
                                        .to_string(),
                                ),
                                Err(error) => observer.emit(format!(
                                    "protected_write_recovery skipped: workspace cleanliness check failed: {error}"
                                )),
                            }
                        }
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

fn protected_write_denial_feedback(outcome: &Outcome) -> Option<String> {
    let feedback = Feedback::from_outcome(outcome).message().to_string();
    if feedback.contains("Write path denied before executing")
        && (feedback.contains("protected") || feedback.contains("out-of-scope"))
    {
        Some(feedback)
    } else {
        None
    }
}

fn protected_write_recovery_feedback(
    workspace: &Path,
    surface: &SurfacePolicy,
    feedback: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push("Protected write recovery:".to_string());
    lines.push(format!("- Previous protected write denial: {feedback}"));
    lines.push(
        "- Do not retry protected manifests/configs such as Cargo.toml, Cargo.lock, rust-toolchain.toml, .cargo/, .ploke/, or crates/ploke-eval/."
            .to_string(),
    );
    lines.push(
        "- Manifest/config changes are not valid candidate patches in this run. Find a source-code change inside the writable surface instead."
            .to_string(),
    );
    lines.push(
        "- Use request_code_context, read_file, list_dir, or code lookup tools to locate an allowed Rust/source-file edit before calling write tools again."
            .to_string(),
    );
    let roots = writable_crate_roots(workspace, surface);
    if roots.is_empty() {
        lines.push(
            "- Writable surface hint: no crate roots could be listed, but protected manifests/configs remain forbidden."
                .to_string(),
        );
    } else {
        lines.push(format!(
            "- Writable crate roots you may inspect/edit include: {}.",
            roots.join(", ")
        ));
    }
    lines.push(
        "- If no mutable-surface solution exists, say that explicitly; otherwise stage exactly one allowed source edit."
            .to_string(),
    );
    lines.join("\n")
}

fn writable_crate_roots(workspace: &Path, surface: &SurfacePolicy) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .arg("ls-files")
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut roots = BTreeSet::new();
    for line in stdout.lines() {
        let rel = PathBuf::from(line);
        if surface
            .classify_paths(workspace, std::slice::from_ref(&rel))
            .is_some()
        {
            continue;
        }
        if let Some(root) = crate_root_label(&rel) {
            roots.insert(root);
        }
    }
    roots.into_iter().take(16).collect()
}

fn crate_root_label(path: &Path) -> Option<String> {
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(first)), Some(Component::Normal(second)))
            if first == "crates" || first == "proc_macros" =>
        {
            Some(format!(
                "{}/{}",
                first.to_string_lossy(),
                second.to_string_lossy()
            ))
        }
        _ => None,
    }
}

fn workspace_is_clean(workspace: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .arg("status")
        .arg("--porcelain")
        .arg("--untracked-files=all")
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(output.stdout.is_empty())
}

fn truncate_inline(value: &str, max: usize) -> String {
    let mut out = value.replace('\n', " ");
    if out.len() > max {
        out.truncate(max);
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::super::harness::{Decision, Harness, Progress};
    use super::super::harness_io::HeadlessRun;
    use super::super::{Budget, Error};
    use super::*;

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

    #[test]
    fn protected_write_denial_is_recoverable_feedback() {
        let message = "Previous attempt failed: non_semantic_patch: Write path denied before executing `non_semantic_patch`: Cargo.toml. Reason: path 'Cargo.toml' is protected";
        let outcome = Outcome::NoEdit(NoEdit::new(message));

        let observed = protected_write_denial_feedback(&outcome).expect("protected denial");

        assert!(observed.contains("Cargo.toml"));
        assert!(observed.contains("Write path denied"));
    }

    #[test]
    fn recovery_feedback_lists_writable_crates_not_protected_crates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        std::fs::create_dir_all(workspace.join("crates/ploke-core/src")).expect("core dir");
        std::fs::create_dir_all(workspace.join("crates/ploke-eval/src")).expect("eval dir");
        std::fs::write(
            workspace.join("crates/ploke-core/src/lib.rs"),
            "pub fn ok() {}\n",
        )
        .expect("write core");
        std::fs::write(
            workspace.join("crates/ploke-eval/src/lib.rs"),
            "pub fn no() {}\n",
        )
        .expect("write eval");
        std::fs::write(workspace.join("Cargo.toml"), "[workspace]\n").expect("write manifest");
        Command::new("git")
            .arg("init")
            .current_dir(workspace)
            .output()
            .expect("git init");
        Command::new("git")
            .args(["add", "."])
            .current_dir(workspace)
            .output()
            .expect("git add");

        let feedback = protected_write_recovery_feedback(
            workspace,
            &SurfacePolicy::workspace_except_core(),
            "Write path denied before executing `non_semantic_patch`: Cargo.toml. Reason: path 'Cargo.toml' is protected",
        );

        let roots_line = feedback
            .lines()
            .find(|line| line.starts_with("- Writable crate roots"))
            .expect("writable roots hint");
        assert!(roots_line.contains("crates/ploke-core"));
        assert!(!roots_line.contains("crates/ploke-eval"));
        assert!(feedback.contains("Cargo.toml"));
        assert!(feedback.contains("request_code_context"));
    }

    #[test]
    fn workspace_cleanliness_guard_detects_untracked_changes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        Command::new("git")
            .arg("init")
            .current_dir(workspace)
            .output()
            .expect("git init");

        assert!(workspace_is_clean(workspace).expect("clean workspace check"));

        std::fs::write(workspace.join("scratch.txt"), "dirty\n").expect("write scratch");

        assert!(!workspace_is_clean(workspace).expect("dirty workspace check"));
    }
}
