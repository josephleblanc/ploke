//! Headless `ploke-tui` executor bridge for broad edit attempts.

use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use ploke_llm::{
    manager::{RecordedResponse, RecordedResponseTape},
    router_only::RouterVariants,
};
use ploke_records::llm_response::RawFullResponseRecord;
use ploke_tui::app::commands::harness::TestAppAccessor;
use serde::Deserialize;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::cli::prototype1_state::edit_surface::tui_adapter::harness;

use super::super::harness_request::{
    EvidenceRoot, EvidenceRootKind, EvidenceRootLocation, contract,
};
use super::super::surface_policy::SurfacePolicy;
use super::{
    Attempt, Budget, Capture, Error, LIVE_TRACE_ENV, ModelSelection,
    harness_io::{
        AppliedEdit, CargoValidationObservation, Event, Feedback, HeadlessAttempt,
        HeadlessAttemptResult, HeadlessRun, HeadlessTerminal, Outcome, Reject, join_paths,
        latest_failed_cargo_validation_feedback, observe_cargo_validation, push_changed_paths,
        truncate_chars,
    },
};

pub(crate) async fn run_headless(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    surface: &SurfacePolicy,
    evidence_roots: &[EvidenceRoot],
) -> Result<HeadlessRun, Error> {
    run_headless_with_model(
        workspace_path,
        prompt,
        budget,
        surface,
        evidence_roots,
        None,
    )
    .await
}

pub(crate) async fn run_headless_with_model(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    surface: &SurfacePolicy,
    evidence_roots: &[EvidenceRoot],
    model: Option<ModelSelection>,
) -> Result<HeadlessRun, Error> {
    Attempt {
        workspace: workspace_path.to_path_buf(),
        prompt: prompt.to_string(),
        budget,
        surface: surface.clone(),
        evidence: evidence_roots.to_vec(),
        validation: Vec::new(),
        model,
        capture: Capture::Off,
        policy_suffix: None,
    }
    .run()
    .await
    .map(super::attempt::Attempt::into_headless_run)
}

pub(crate) async fn run_headless_with_model_capture_responses(
    workspace_path: &Path,
    prompt: &str,
    budget: Budget,
    surface: &SurfacePolicy,
    evidence_roots: &[EvidenceRoot],
    validation_commands: &[contract::Command],
    model: Option<ModelSelection>,
) -> Result<HeadlessRun, Error> {
    Attempt {
        workspace: workspace_path.to_path_buf(),
        prompt: prompt.to_string(),
        budget,
        surface: surface.clone(),
        evidence: evidence_roots.to_vec(),
        validation: validation_commands.to_vec(),
        model,
        capture: Capture::Responses,
        policy_suffix: None,
    }
    .run()
    .await
    .map(super::attempt::Attempt::into_headless_run)
}

pub(crate) enum LlmDebugStepSource {
    Recorded(RawFullResponseRecord),
    Live,
}

pub(crate) struct LlmDebugStepRun {
    pub(crate) session_id: Uuid,
    pub(crate) outcome: String,
    pub(crate) attempts: u32,
    pub(crate) final_messages: usize,
}

pub(crate) async fn run_llm_debug_step(
    workspace_path: &Path,
    messages: Vec<ploke_tui::llm::RequestMessage>,
    budget: Budget,
    surface: &SurfacePolicy,
    evidence_roots: &[EvidenceRoot],
    model: Option<ModelSelection>,
    source: LlmDebugStepSource,
) -> Result<LlmDebugStepRun, Error> {
    let timeouts = harness::Timeouts::default();
    let extra_read_roots = evidence_read_roots(evidence_roots);
    let mut runtime = start_debug_runtime(
        workspace_path,
        &extra_read_roots,
        surface,
        model.as_ref(),
        budget,
        &timeouts,
    )
    .await?;
    let _debug_guard =
        super::tool_loop_debug::install_for_attempt(workspace_path, model.as_ref(), evidence_roots);
    match source {
        LlmDebugStepSource::Recorded(record) => {
            ploke_tui::llm::install_recorded_response_tape(RecordedResponseTape::new(vec![
                record.into_recorded_response(),
            ]));
        }
        LlmDebugStepSource::Live => {
            ploke_tui::llm::install_recorded_response_prefix_then_live_steps(
                RecordedResponseTape::new(Vec::new()),
                1,
            );
        }
    }
    let _recorded_guard = RecordedResponseGuard;
    let report = ploke_tui::llm::run_chat_debug_messages(ploke_tui::llm::ChatDebugRunArgs {
        state: Arc::clone(&runtime.state),
        client: reqwest::Client::new(),
        messages,
        event_bus: Arc::clone(&runtime.event_bus),
        assistant_message_id: Uuid::new_v4(),
        parent_id: Uuid::new_v4(),
        cmd_tx: runtime.app.state_cmd_tx(),
    })
    .await;
    runtime.app.pump_pending_events().await;
    Ok(LlmDebugStepRun {
        session_id: report.session_id,
        outcome: report.outcome,
        attempts: report.attempts,
        final_messages: report.final_messages.len(),
    })
}

struct RecordedResponseGuard;

impl Drop for RecordedResponseGuard {
    fn drop(&mut self) {
        ploke_tui::llm::clear_recorded_response_tape();
    }
}

async fn start_debug_runtime(
    workspace_path: &Path,
    extra_read_roots: &[PathBuf],
    surface: &SurfacePolicy,
    model: Option<&ModelSelection>,
    budget: Budget,
    timeouts: &harness::Timeouts,
) -> Result<crate::runner::WorkspaceTuiRuntime, Error> {
    let runtime = crate::runner::setup_workspace_tui_runtime_with_read_roots(
        workspace_path,
        extra_read_roots,
    )
    .await
    .map_err(Error::from_headless_start)?;

    let write_scope = surface.write_scope();
    runtime
        .state
        .with_system_txn(|txn| txn.set_write_scope(Some(write_scope)))
        .await;

    let cmd_tx = runtime.app.state_cmd_tx();
    send_state(
        &cmd_tx,
        ploke_tui::app_state::StateCommand::SetEditingAutoConfirm { enabled: false },
    )
    .await?;
    {
        let mut cfg = runtime.state.config.write().await;
        cfg.context_management.mode = ploke_tui::user_config::CtxMode::Off;
        cfg.tooling.cargo_check_timeout_secs = timeouts.validation_cargo_check_secs;
        cfg.tooling.cargo_test_timeout_secs = timeouts.validation_cargo_test_secs;
        cfg.chat_policy.tool_call_timeout_secs = budget_tool_timeout_secs(budget, timeouts);
        if let Some(model) = model {
            cfg.active_model = model.model_id.clone();
            cfg.active_router = model.router();
            if !matches!(model.router(), RouterVariants::Google(_)) || model.provider.is_some() {
                cfg.model_registry
                    .select_model_provider(&model.model_id, model.provider.as_ref());
            }
        }
    }
    Ok(runtime)
}

fn budget_tool_timeout_secs(budget: Budget, timeouts: &harness::Timeouts) -> u64 {
    budget
        .timeout_secs()
        .max(timeouts.validation_cargo_check_secs)
        .max(timeouts.validation_cargo_test_secs)
}

pub(super) async fn start_attempt_runtime(
    workspace_path: &Path,
    extra_read_roots: &[PathBuf],
    prompt: String,
    surface: &SurfacePolicy,
    model: Option<&ModelSelection>,
    timeouts: &harness::Timeouts,
) -> Result<(crate::runner::WorkspaceTuiRuntime, Uuid), Error> {
    let runtime = crate::runner::setup_workspace_tui_runtime_with_read_roots(
        workspace_path,
        extra_read_roots,
    )
    .await
    .map_err(Error::from_headless_start)?;

    let write_scope = surface.write_scope();
    runtime
        .state
        .with_system_txn(|txn| txn.set_write_scope(Some(write_scope)))
        .await;

    let cmd_tx = runtime.app.state_cmd_tx();
    send_state(
        &cmd_tx,
        ploke_tui::app_state::StateCommand::SetEditingAutoConfirm { enabled: false },
    )
    .await?;
    {
        let mut cfg = runtime.state.config.write().await;
        cfg.context_management.mode = ploke_tui::user_config::CtxMode::Off;
        cfg.tooling.cargo_check_timeout_secs = timeouts.validation_cargo_check_secs;
        cfg.tooling.cargo_test_timeout_secs = timeouts.validation_cargo_test_secs;
        if let Some(model) = model {
            cfg.active_model = model.model_id.clone();
            cfg.active_router = model.router();
            if !matches!(model.router(), RouterVariants::Google(_)) || model.provider.is_some() {
                cfg.model_registry
                    .select_model_provider(&model.model_id, model.provider.as_ref());
            }
        }
    }
    let parent_id = submit_prompt(&runtime.app, prompt).await?;
    Ok((runtime, parent_id))
}

#[derive(Debug)]
pub(super) enum AttemptEnd {
    Terminal(HeadlessTerminal),
    RetryFailure(String),
    RetryNoEdit {
        feedback: String,
        outcome: String,
        summary: String,
    },
}

const MAX_POLICY_REPAIR_TURNS: u32 = 2;

/// Headless broad-harness slots run request-declared cargo validation against a
/// cold per-slot `target/` dir. The default 60s `cargo check` budget is too
/// small for a cold compile of `ploke-eval`, so harness validation was being
/// killed before it could pass. Give the slot's declared validation a budget
/// that can absorb a cold compile while still fitting inside the slot
/// wall-clock timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppliedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl AppliedItem {
    pub(super) fn id(self) -> Uuid {
        match self {
            Self::Edit(id) | Self::Create(id) => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum StagedItem {
    Edit(Uuid),
    Create(Uuid),
}

impl StagedItem {
    pub(super) fn id(self) -> Uuid {
        match self {
            Self::Edit(id) | Self::Create(id) => id,
        }
    }

    pub(super) fn applied(self) -> AppliedItem {
        match self {
            Self::Edit(id) => AppliedItem::Edit(id),
            Self::Create(id) => AppliedItem::Create(id),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ToolBatch {
    expected: Vec<ploke_core::ArcStr>,
    completed: Vec<ploke_core::ArcStr>,
    staged: Vec<StagedItem>,
}

impl ToolBatch {
    pub(super) fn request(&mut self, call_id: ploke_core::ArcStr) {
        if !self.expected.contains(&call_id) {
            self.expected.push(call_id);
        }
    }

    fn complete(&mut self, call_id: ploke_core::ArcStr, staged: Option<StagedItem>) {
        if !self.completed.contains(&call_id) {
            self.completed.push(call_id);
        }
        if let Some(staged) = staged {
            if !self.staged.contains(&staged) {
                self.staged.push(staged);
            }
        }
    }

    fn is_complete(&self) -> bool {
        !self.expected.is_empty()
            && self
                .expected
                .iter()
                .all(|call_id| self.completed.contains(call_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) item: StagedItem,
    pub(super) proposed_at_ms: i64,
    pub(super) paths: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub(super) struct BatchOutcome {
    pub(super) applied: Vec<AppliedItem>,
    pub(super) changed_paths: Vec<PathBuf>,
    pub(super) mutated: bool,
    pub(super) feedbacks: Vec<String>,
    pub(super) retry: Option<String>,
}

// TODO: I think this actually wants to be a method of `HeadlessRun`
pub(super) async fn run_attempt(
    runtime: crate::runner::WorkspaceTuiRuntime,
    active_parent_id: Uuid,
    workspace_path: &Path,
    surface: &SurfacePolicy,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    validation_commands: &[contract::Command],
    response_rx: Option<Arc<Mutex<Receiver<RecordedResponse>>>>,
    timeouts: harness::Timeouts,
) -> Result<(AttemptEnd, crate::runner::WorkspaceTuiRuntime), Error> {
    let spec = harness::SessionSpec {
        workspace_path: workspace_path.to_path_buf(),
        timeouts,
    };
    let owned_run = std::mem::replace(run, HeadlessRun::new());
    let mut harness = harness::TuiHarness::attach(
        runtime,
        owned_run,
        spec,
        active_parent_id,
        turn,
        validation_commands.to_vec(),
        response_rx,
        *observer,
    );
    let end = harness.drive_to_attempt_end(surface).await;
    harness.finalize().await;
    let (restored_run, runtime) = harness.into_parts();
    *run = restored_run;
    end.map(|end| (end, runtime))
}

pub(super) fn terminal_ids(applied: &[AppliedItem]) -> Option<(Uuid, Vec<Uuid>)> {
    let proposal_ids = applied.iter().map(|item| item.id()).collect::<Vec<_>>();
    proposal_ids
        .last()
        .copied()
        .map(|primary| (primary, proposal_ids))
}

pub(super) fn applied_edit_from_terminal_items(
    applied: &[AppliedItem],
    changed_paths: &[PathBuf],
) -> Option<AppliedEdit> {
    let (proposal_id, proposal_ids) = terminal_ids(applied)?;
    Some(AppliedEdit {
        proposal_id,
        proposal_ids,
        changed_paths: changed_paths.to_vec(),
    })
}

/// Run request-declared validation against the candidate as soon as a tool
/// batch settles with at least one allowed applied edit, then classify the
/// applied candidate.
///
/// The harness runs the declared validation itself, so admission does not
/// depend on the model issuing the exact declared cargo commands; those harness
/// observations are the most recent for each command, so
/// `classify_applied_terminal` consults the harness run rather than any earlier
/// model-issued cargo call.
///
/// This returns the classified terminal but does not decide whether to stop.
/// The caller finalizes the attempt only when validation is satisfied
/// (`HeadlessTerminal::Applied`). When validation is not yet satisfied the
/// caller keeps the attempt running so the model can repair the candidate
/// across later turns, which preserves multi-turn repair flows while still
/// letting a passing candidate stop immediately instead of burning the slot
/// wall-clock on further tool calls.
pub(super) async fn validate_applied_batch(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    active_parent_id: Uuid,
    request_id: Uuid,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    validation_commands: &[contract::Command],
    applied: &[AppliedItem],
    changed_paths: &[PathBuf],
) -> Option<HeadlessTerminal> {
    let applied_edit = applied_edit_from_terminal_items(applied, changed_paths)?;
    observer.emit(format!(
        "attempt {turn} validate_applied_batch proposals={} changed_paths={}",
        applied_edit.proposal_ids().len(),
        changed_paths.len()
    ));
    if !validation_commands.is_empty() {
        run_contract_validations(
            runtime,
            active_parent_id,
            request_id,
            turn,
            run,
            observer,
            validation_commands,
        )
        .await;
    }
    Some(classify_applied_terminal(
        run,
        validation_commands,
        request_id,
        applied_edit,
    ))
}

pub(super) fn timeout_terminal_for_run(run: &HeadlessRun, secs: u64) -> HeadlessTerminal {
    if let Some(applied) = run.applied_edit() {
        return HeadlessTerminal::AppliedTimedOut { secs, applied };
    }
    HeadlessTerminal::TimedOut { secs }
}

pub(super) fn turn_aborted_after_apply_terminal(
    applied: AppliedEdit,
    outcome: String,
    summary: String,
) -> HeadlessTerminal {
    HeadlessTerminal::AppliedTurnAborted {
        applied,
        outcome,
        summary,
    }
}

pub(super) fn classify_applied_terminal(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
    request_id: Uuid,
    applied: AppliedEdit,
) -> HeadlessTerminal {
    if validation_commands.is_empty() {
        if let Some(feedback) = latest_failed_cargo_validation_feedback(run) {
            return HeadlessTerminal::AppliedValidationFailed { applied, feedback };
        }
        return applied_terminal(request_id, applied);
    }

    if let Some(feedback) = failed_requested_validation_feedback(run, validation_commands) {
        return HeadlessTerminal::AppliedValidationFailed { applied, feedback };
    }

    let missing = missing_requested_validation_commands(run, validation_commands);
    if !missing.is_empty() {
        return HeadlessTerminal::AppliedValidationMissing { applied, missing };
    }

    applied_terminal(request_id, applied)
}

fn applied_terminal(request_id: Uuid, applied: AppliedEdit) -> HeadlessTerminal {
    HeadlessTerminal::Applied {
        proposal_id: applied.proposal_id(),
        applied_proposal_ids: applied.proposal_ids().to_vec(),
        request_id,
        changed_paths: applied.changed_paths().to_vec(),
    }
}

fn failed_requested_validation_feedback(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
) -> Option<String> {
    for command in validation_commands {
        let required = validation_command_display(command);
        let Some(observation) = latest_observation_for_command(run, &required) else {
            continue;
        };
        if let Some(feedback) = observation.failure_feedback() {
            return Some(format!(
                "Requested validation `{required}` failed: {feedback}"
            ));
        }
    }
    None
}

fn missing_requested_validation_commands(
    run: &HeadlessRun,
    validation_commands: &[contract::Command],
) -> Vec<String> {
    validation_commands
        .iter()
        .map(validation_command_display)
        .filter(|required| latest_observation_for_command(run, required).is_none())
        .collect()
}

fn latest_observation_for_command<'a>(
    run: &'a HeadlessRun,
    required: &str,
) -> Option<&'a CargoValidationObservation> {
    run.validations()
        .iter()
        .rev()
        .find(|observation| command_display_matches(required, &observation.display_command))
}

pub(super) fn command_display_matches(required: &str, observed: &str) -> bool {
    observed == required || observed.replace(" -- ", " ") == required
}

pub(super) fn validation_command_display(command: &contract::Command) -> String {
    std::iter::once(command.program.as_str())
        .chain(command.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) async fn run_contract_validations(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    parent_id: Uuid,
    request_id: Uuid,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    validation_commands: &[contract::Command],
) {
    for (idx, command) in validation_commands.iter().enumerate() {
        let required = validation_command_display(command);
        let call_id = format!("declared_validation_{turn}_{idx}");
        let args = match contract_cargo_args(command) {
            Ok(args) => args,
            Err(error) => {
                observer.emit(format!(
                    "attempt {turn} declared_validation_unsupported command={} error={}",
                    required,
                    truncate_chars(&error, 240)
                ));
                continue;
            }
        };
        observer.emit(format!(
            "attempt {turn} declared_validation_start call_id={} command={}",
            call_id, required
        ));

        let ctx = ploke_tui::tools::Ctx {
            state: Arc::clone(&runtime.state),
            event_bus: Arc::new(ploke_tui::EventBus::new(ploke_tui::EventBusCaps::default())),
            request_id,
            parent_id,
            call_id: ploke_core::ArcStr::from(call_id.clone()),
        };
        let params =
            match <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::deserialize_params(
                &args,
            ) {
                Ok(params) => params,
                Err(error) => {
                    let detail =
                        <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::adapt_error(
                            error,
                        )
                        .to_wire_string();
                    observer.emit(format!(
                        "attempt {turn} declared_validation_deserialize_failed call_id={} command={} error={}",
                        call_id,
                        required,
                        truncate_chars(&detail, 240)
                    ));
                    record_validation_failure(run, &call_id, command);
                    continue;
                }
            };
        match <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::execute(params, ctx)
            .await
        {
            Ok(result) => {
                if let Some(validation) =
                    observe_cargo_validation(run, &call_id, &args, &result.content)
                {
                    observer.emit(format!(
                        "attempt {turn} declared_validation_done call_id={} ok={} status={} command={}",
                        call_id,
                        validation.ok,
                        validation.status_reason,
                        validation.display_command
                    ));
                }
            }
            Err(error) => {
                let detail =
                    <ploke_tui::tools::cargo::CargoTool as ploke_tui::tools::Tool>::adapt_error(
                        ploke_tui::tools::ToolInvocationError::Exec(error),
                    )
                    .to_wire_string();
                observer.emit(format!(
                    "attempt {turn} declared_validation_failed call_id={} command={} error={}",
                    call_id,
                    required,
                    truncate_chars(&detail, 240)
                ));
                record_validation_failure(run, &call_id, command);
            }
        }
    }
}

pub(super) fn contract_cargo_args(command: &contract::Command) -> Result<String, String> {
    if command.program != "cargo" {
        return Err(format!(
            "unsupported validation program `{}`",
            command.program
        ));
    }
    let Some((subcommand, rest)) = command.args.split_first() else {
        return Err("cargo validation command is missing subcommand".to_string());
    };
    if !matches!(subcommand.as_str(), "check" | "test") {
        return Err(format!("unsupported cargo subcommand `{subcommand}`"));
    }

    let mut args = serde_json::Map::new();
    args.insert(
        "command".to_string(),
        serde_json::Value::String(subcommand.clone()),
    );
    let mut test_args = Vec::<String>::new();
    let mut rest = rest.iter();
    let mut passthrough = false;

    while let Some(arg) = rest.next() {
        if passthrough {
            test_args.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--" => passthrough = true,
            "-p" | "--package" => insert_next(&mut args, &mut rest, "package", arg)?,
            "--features" => insert_features(&mut args, rest.next(), arg)?,
            "--target" => insert_next(&mut args, &mut rest, "target", arg)?,
            "--profile" => insert_next(&mut args, &mut rest, "profile", arg)?,
            "--all-features" => insert_bool(&mut args, "all_features"),
            "--no-default-features" => insert_bool(&mut args, "no_default_features"),
            "--release" => insert_bool(&mut args, "release"),
            "--lib" => insert_bool(&mut args, "lib"),
            "--tests" => insert_bool(&mut args, "tests"),
            "--bins" => insert_bool(&mut args, "bins"),
            "--examples" => insert_bool(&mut args, "examples"),
            "--benches" => insert_bool(&mut args, "benches"),
            _ if arg.starts_with("--package=") => {
                insert_str(&mut args, "package", &arg["--package=".len()..])?
            }
            _ if arg.starts_with("--features=") => {
                insert_features_value(&mut args, &arg["--features=".len()..])?
            }
            _ if arg.starts_with("--target=") => {
                insert_str(&mut args, "target", &arg["--target=".len()..])?
            }
            _ if arg.starts_with("--profile=") => {
                insert_str(&mut args, "profile", &arg["--profile=".len()..])?
            }
            _ if subcommand == "test" => test_args.push(arg.clone()),
            _ => {
                return Err(format!(
                    "unsupported cargo validation argument `{arg}` in `{}`",
                    validation_command_display(command)
                ));
            }
        }
    }

    if !test_args.is_empty() {
        args.insert(
            "test_args".to_string(),
            serde_json::Value::Array(
                test_args
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    serde_json::to_string(&serde_json::Value::Object(args))
        .map_err(|error| format!("failed to serialize cargo validation args: {error}"))
}

fn insert_next<'a>(
    args: &mut serde_json::Map<String, serde_json::Value>,
    rest: &mut impl Iterator<Item = &'a String>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = rest.next() else {
        return Err(format!("missing value for `{flag}`"));
    };
    insert_str(args, key, value)
}

fn insert_str(
    args: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("empty value for `{key}`"));
    }
    args.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
    Ok(())
}

fn insert_bool(args: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    args.insert(key.to_string(), serde_json::Value::Bool(true));
}

fn insert_features<'a>(
    args: &mut serde_json::Map<String, serde_json::Value>,
    value: Option<&'a String>,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = value else {
        return Err(format!("missing value for `{flag}`"));
    };
    insert_features_value(args, value)
}

fn insert_features_value(
    args: &mut serde_json::Map<String, serde_json::Value>,
    value: &str,
) -> Result<(), String> {
    let features = value
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(|feature| serde_json::Value::String(feature.to_string()))
        .collect::<Vec<_>>();
    if features.is_empty() {
        return Err("empty value for `features`".to_string());
    }
    args.insert("features".to_string(), serde_json::Value::Array(features));
    Ok(())
}

fn record_validation_failure(run: &mut HeadlessRun, call_id: &str, command: &contract::Command) {
    let command_name = command.args.first().cloned().unwrap_or_default();
    run.validations.push(CargoValidationObservation {
        call_id: call_id.to_string(),
        command: command_name,
        display_command: validation_command_display(command),
        ok: false,
        status_reason: "cargo_failed_or_invalid_args".to_string(),
        exit_code: None,
        manifest_path: String::new(),
        errors: 1,
        warnings: 0,
    });
}

pub(super) fn drain_response_records(
    run: &mut HeadlessRun,
    assistant_message_id: Uuid,
    response_rx: Option<&Mutex<Receiver<RecordedResponse>>>,
) {
    let Some(response_rx) = response_rx else {
        return;
    };
    let Ok(response_rx) = response_rx.lock() else {
        return;
    };
    for recorded_response in response_rx.try_iter() {
        let response_index = run.next_response_index;
        run.next_response_index = run.next_response_index.saturating_add(1);
        run.full_response_records.push(RawFullResponseRecord {
            assistant_message_id,
            recorded_response: RecordedResponse::new(response_index, recorded_response.response),
        });
    }
}

pub(super) fn record_batch_terminal(
    batches: &mut HashMap<Uuid, ToolBatch>,
    request_id: Uuid,
    call_id: ploke_core::ArcStr,
    staged: Option<StagedItem>,
) -> Option<Vec<StagedItem>> {
    let batch = batches.entry(request_id).or_default();
    batch.complete(call_id, staged);
    if batch.is_complete() {
        batches.remove(&request_id).map(|batch| batch.staged)
    } else {
        None
    }
}

pub(super) async fn observe_staged_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    ui_payload: Option<&ploke_tui::tools::ToolUiPayload>,
    request_id: Uuid,
    applied: &[AppliedItem],
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
) -> Option<StagedItem> {
    let Some(payload) = ui_payload else {
        return None;
    };
    if let Some(proposal_id) = payload.proposal_id {
        if applied.contains(&AppliedItem::Edit(proposal_id)) {
            observer.emit(format!(
                "attempt {turn} proposal_already_applied id={proposal_id}"
            ));
            return None;
        }
        let Some(proposal) = runtime
            .state
            .proposals
            .read()
            .await
            .get(&proposal_id)
            .cloned()
        else {
            return None;
        };
        let paths = proposal_paths(&proposal);
        run.events.push(Event::Proposal {
            id: proposal_id.to_string(),
            edit_count: proposal.edits.len() + proposal.edits_ns.len(),
            paths: paths.clone(),
        });
        observer.emit(format!(
            "attempt {turn} proposal id={} edit_count={} paths={}",
            proposal_id,
            proposal.edits.len() + proposal.edits_ns.len(),
            join_paths(&paths)
        ));
        return Some(StagedItem::Edit(proposal_id));
    }

    if payload.tool == ploke_tui::tools::ToolName::CreateFile {
        if applied.contains(&AppliedItem::Create(request_id)) {
            observer.emit(format!(
                "attempt {turn} creation_already_applied id={request_id}"
            ));
            return None;
        }
        let Some(proposal) = runtime
            .state
            .create_proposals
            .read()
            .await
            .get(&request_id)
            .cloned()
        else {
            return None;
        };
        let paths = proposal.files.clone();
        run.events.push(Event::Proposal {
            id: request_id.to_string(),
            edit_count: proposal.creates.len(),
            paths: paths.clone(),
        });
        observer.emit(format!(
            "attempt {turn} creation id={} edit_count={} paths={}",
            request_id,
            proposal.creates.len(),
            join_paths(&paths)
        ));
        return Some(StagedItem::Create(request_id));
    }

    None
}

async fn settle_staged_batch(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    workspace_path: &Path,
    surface: &SurfacePolicy,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    staged: Vec<StagedItem>,
    applied: &[AppliedItem],
) -> Result<BatchOutcome, Error> {
    let mut outcome = BatchOutcome::default();
    let mut candidates = Vec::new();

    for item in staged {
        if applied.contains(&item.applied()) {
            continue;
        }
        let Some(candidate) = candidate_for_item(runtime, item).await else {
            let reason = format!("staged proposal {} disappeared before admission", item.id());
            reject_item(runtime, item, turn, run, observer, reason.clone()).await?;
            outcome.retry = Some(reason);
            continue;
        };
        if candidate.paths.is_empty() {
            let reason = match item {
                StagedItem::Edit(_) => "No material edit was staged; make a concrete bounded edit.",
                StagedItem::Create(_) => {
                    "No material file creation was staged; make a concrete bounded edit."
                }
            }
            .to_string();
            reject_item(runtime, item, turn, run, observer, reason.clone()).await?;
            outcome.feedbacks.push(repair_prompt_feedback(&reason));
            continue;
        }
        if let Some(rejection) = classify_paths(workspace_path, &surface, &candidate.paths) {
            let feedback = Feedback::from_outcome(&Outcome::Rejected(rejection));
            reject_item(
                runtime,
                item,
                turn,
                run,
                observer,
                feedback.message().to_string(),
            )
            .await?;
            outcome
                .feedbacks
                .push(repair_prompt_feedback(feedback.message()));
            continue;
        }
        candidates.push(candidate);
    }

    let (selected, rejected) = select_disjoint(candidates, workspace_path);
    for candidate in rejected {
        let reason =
            "Staged edit overlaps a newer valid proposal from the same tool batch".to_string();
        reject_item(runtime, candidate.item, turn, run, observer, reason.clone()).await?;
        outcome.feedbacks.push(repair_prompt_feedback(&reason));
    }

    if selected.is_empty() {
        return Ok(outcome);
    }

    let refresh_paths = selected
        .iter()
        .flat_map(|candidate| candidate.paths.iter().cloned())
        .collect::<Vec<_>>();
    let timeouts = harness::Timeouts::default();
    approve_selected(runtime, turn, observer, &selected).await?;
    let applied_outcome =
        match wait_for_selected(runtime, turn, run, observer, &selected, &timeouts).await {
            Ok(outcome) => outcome,
            Err(error) => {
                record_post_approval_indeterminate(
                    run,
                    turn,
                    observer,
                    &selected,
                    &error.to_string(),
                );
                return Err(error);
            }
        };
    outcome.applied.extend(applied_outcome.applied);
    outcome.changed_paths.extend(applied_outcome.changed_paths);
    outcome.mutated |= applied_outcome.mutated;
    if applied_outcome.retry.is_some() {
        outcome.retry = applied_outcome.retry;
    }
    if !outcome.applied.is_empty() || outcome.mutated {
        let refresh_deadline = Instant::now() + timeouts.post_apply_index_duration();
        if let Err(error) = wait_for_refresh(
            runtime,
            pending_events,
            turn,
            observer,
            refresh_deadline,
            &timeouts,
            &refresh_paths,
        )
        .await
        {
            record_post_approval_indeterminate(run, turn, observer, &selected, &error.to_string());
            return Err(error);
        }
    }
    Ok(outcome)
}

pub(super) fn record_post_approval_indeterminate(
    run: &mut HeadlessRun,
    turn: u32,
    observer: &LiveObserver,
    selected: &[Candidate],
    error: &str,
) {
    for candidate in selected {
        if has_recorded_apply_outcome(run, candidate.item) {
            continue;
        }
        run.attempts.push(HeadlessAttempt {
            turn,
            proposal_id: Some(candidate.item.id()),
            result: HeadlessAttemptResult::PostApprovalIndeterminate {
                paths: candidate.paths.clone(),
                error: error.to_string(),
            },
        });
        observer.emit(format!(
            "attempt {turn} proposal_post_approval_indeterminate id={} error={}",
            candidate.item.id(),
            truncate_chars(error, 240)
        ));
    }
}

fn has_recorded_apply_outcome(run: &HeadlessRun, item: StagedItem) -> bool {
    run.attempts.iter().any(|attempt| {
        attempt.proposal_id == Some(item.id())
            && matches!(
                attempt.result,
                HeadlessAttemptResult::Applied { .. }
                    | HeadlessAttemptResult::Rejected { .. }
                    | HeadlessAttemptResult::PostApprovalIndeterminate { .. }
            )
    })
}

pub(super) async fn candidate_for_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
) -> Option<Candidate> {
    match item {
        StagedItem::Edit(proposal_id) => {
            let proposal = runtime
                .state
                .proposals
                .read()
                .await
                .get(&proposal_id)
                .cloned()?;
            Some(Candidate {
                item,
                proposed_at_ms: proposal.proposed_at_ms,
                paths: proposal_paths(&proposal),
            })
        }
        StagedItem::Create(request_id) => {
            let proposal = runtime
                .state
                .create_proposals
                .read()
                .await
                .get(&request_id)
                .cloned()?;
            Some(Candidate {
                item,
                proposed_at_ms: proposal.proposed_at_ms,
                paths: proposal.files,
            })
        }
    }
}

pub(super) fn select_disjoint(
    mut candidates: Vec<Candidate>,
    workspace_path: &Path,
) -> (Vec<Candidate>, Vec<Candidate>) {
    candidates.sort_by(|a, b| {
        b.proposed_at_ms
            .cmp(&a.proposed_at_ms)
            .then(b.item.id().cmp(&a.item.id()))
    });

    let mut occupied = Vec::<PathBuf>::new();
    let mut selected = Vec::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        let keys = candidate
            .paths
            .iter()
            .map(|path| path_key(workspace_path, path))
            .collect::<Vec<_>>();
        if keys.iter().any(|key| occupied.contains(key)) {
            rejected.push(candidate);
        } else {
            occupied.extend(keys);
            selected.push(candidate);
        }
    }
    (selected, rejected)
}

fn path_key(workspace_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.strip_prefix(workspace_path)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

pub(super) async fn reject_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    reason: String,
) -> Result<(), Error> {
    deny_item(runtime, item, Some(reason.clone())).await?;
    run.attempts.push(HeadlessAttempt {
        turn,
        proposal_id: Some(item.id()),
        result: HeadlessAttemptResult::Rejected {
            reason: reason.clone(),
        },
    });
    observer.emit(format!(
        "attempt {turn} proposal_rejected id={} reason={}",
        item.id(),
        truncate_chars(&reason, 240)
    ));
    Ok(())
}

async fn deny_item(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
    reason: Option<String>,
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    let cmd_tx = runtime.app.state_cmd_tx();
    match item {
        StagedItem::Edit(proposal_id) => {
            send_state(
                &cmd_tx,
                StateCommand::DenyEdits {
                    proposal_id,
                    reason,
                },
            )
            .await
        }
        StagedItem::Create(request_id) => {
            send_state(&cmd_tx, StateCommand::DenyCreations { request_id }).await
        }
    }
}

pub(super) async fn approve_selected(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    turn: u32,
    observer: &LiveObserver,
    selected: &[Candidate],
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    let cmd_tx = runtime.app.state_cmd_tx();
    for candidate in selected {
        match candidate.item {
            StagedItem::Edit(proposal_id) => {
                observer.emit(format!("attempt {turn} proposal_approve id={proposal_id}"));
                send_state(&cmd_tx, StateCommand::ApproveEdits { proposal_id }).await?;
            }
            StagedItem::Create(request_id) => {
                observer.emit(format!("attempt {turn} creation_approve id={request_id}"));
                send_state(&cmd_tx, StateCommand::ApproveCreations { request_id }).await?;
            }
        }
    }
    Ok(())
}

pub(super) async fn wait_for_selected(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    turn: u32,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    selected: &[Candidate],
    timeouts: &harness::Timeouts,
) -> Result<BatchOutcome, Error> {
    use ploke_tui::app_state::core::EditProposalStatus;

    let status_timeout = timeouts.post_apply_status_duration();
    let deadline = Instant::now() + status_timeout;
    let mut pending = selected
        .iter()
        .map(|candidate| candidate.item)
        .collect::<Vec<_>>();
    let mut outcome = BatchOutcome::default();

    while !pending.is_empty() {
        if Instant::now() >= deadline {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for proposal batch apply after {}s",
                status_timeout.as_secs()
            )));
        }

        runtime.app.pump_pending_events().await;
        drain_debug_observed(&mut runtime.debug_rx, run, observer, turn);

        let mut still_pending = Vec::new();
        for item in pending {
            let Some((status, paths)) = item_status(runtime, item).await else {
                let reason = format!("staged proposal {} disappeared before apply", item.id());
                run.attempts.push(HeadlessAttempt {
                    turn,
                    proposal_id: Some(item.id()),
                    result: HeadlessAttemptResult::Rejected {
                        reason: reason.clone(),
                    },
                });
                outcome.retry = Some(reason);
                continue;
            };
            match status {
                EditProposalStatus::Applied => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Applied {
                            paths: paths.clone(),
                        },
                    });
                    observer.emit(format!("attempt {turn} proposal_applied id={}", item.id()));
                    outcome.applied.push(item.applied());
                    push_changed_paths(&mut outcome.changed_paths, paths);
                }
                EditProposalStatus::PartiallyApplied(reason) => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!(
                        "attempt {turn} proposal_partially_applied id={} reason={}",
                        item.id(),
                        truncate_chars(&reason, 240)
                    ));
                    outcome.mutated = true;
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Failed(reason) | EditProposalStatus::Stale(reason) => {
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!(
                        "attempt {turn} proposal_apply_failed id={} reason={}",
                        item.id(),
                        truncate_chars(&reason, 240)
                    ));
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Denied => {
                    let reason = "proposal was denied before apply".to_string();
                    run.attempts.push(HeadlessAttempt {
                        turn,
                        proposal_id: Some(item.id()),
                        result: HeadlessAttemptResult::Rejected {
                            reason: reason.clone(),
                        },
                    });
                    observer.emit(format!("attempt {turn} proposal_denied id={}", item.id()));
                    outcome.retry = Some(reason);
                }
                EditProposalStatus::Pending | EditProposalStatus::Approved => {
                    still_pending.push(item);
                }
            }
        }
        pending = still_pending;
        if !pending.is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    Ok(outcome)
}

async fn item_status(
    runtime: &crate::runner::WorkspaceTuiRuntime,
    item: StagedItem,
) -> Option<(ploke_tui::app_state::core::EditProposalStatus, Vec<PathBuf>)> {
    match item {
        StagedItem::Edit(proposal_id) => {
            let proposal = runtime
                .state
                .proposals
                .read()
                .await
                .get(&proposal_id)
                .cloned()?;
            let paths = proposal_paths(&proposal);
            Some((proposal.status, paths))
        }
        StagedItem::Create(request_id) => {
            let proposal = runtime
                .state
                .create_proposals
                .read()
                .await
                .get(&request_id)
                .cloned()?;
            Some((proposal.status, proposal.files))
        }
    }
}

pub(super) async fn wait_for_refresh(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    turn: u32,
    observer: &LiveObserver,
    deadline: Instant,
    timeouts: &harness::Timeouts,
    changed_paths: &[PathBuf],
) -> Result<(), Error> {
    use ploke_tui::app_state::StateCommand;

    if changed_paths.is_empty() {
        return Err(Error::HeadlessEvent(
            "post-apply refresh requires at least one changed path".to_string(),
        ));
    }

    let (scan_tx, scan_rx) = oneshot::channel();
    send_state(
        &runtime.app.state_cmd_tx(),
        StateCommand::ScanPathsForChange {
            paths: changed_paths.to_vec(),
            scan_tx,
        },
    )
    .await?;
    let changed = scan_rx
        .await
        .map_err(|source| Error::HeadlessEvent(format!("scan barrier failed: {source}")))?;
    runtime.app.pump_pending_events().await;
    observer.emit(format!(
        "attempt {turn} scan_barrier changed={}",
        changed
            .as_ref()
            .map(|paths| join_paths(paths))
            .unwrap_or_else(|| "none".to_string())
    ));
    if wait_for_sparse_search_refresh(
        runtime,
        changed.is_some(),
        turn,
        observer,
        deadline,
        timeouts,
    )
    .await?
    {
        return Ok(());
    }
    wait_for_index_output(
        runtime,
        pending_events,
        changed.is_some(),
        turn,
        observer,
        deadline,
        timeouts,
    )
    .await
}

async fn wait_for_sparse_search_refresh(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    changed: bool,
    turn: u32,
    observer: &LiveObserver,
    deadline: Instant,
    timeouts: &harness::Timeouts,
) -> Result<bool, Error> {
    use ploke_db::bm25_index::bm25_service::Bm25Status;

    let sparse_refresh = {
        let cfg = runtime.state.config.read().await;
        sparse_search_refresh_enabled(&cfg.rag.strategy, cfg.rag.strict_bm25_by_default)
    };
    if !sparse_refresh {
        return Ok(false);
    }

    let Some(rag) = runtime.state.rag.as_ref().cloned() else {
        return Err(Error::HeadlessEvent(
            "sparse post-apply refresh requires a RAG service, but none is configured".to_string(),
        ));
    };

    if changed {
        observer.emit(format!("attempt {turn} sparse_refresh bm25_rebuild"));
        rag.bm25_rebuild()
            .await
            .map_err(|source| Error::HeadlessEvent(format!("BM25 rebuild failed: {source}")))?;
    } else {
        observer.emit(format!("attempt {turn} sparse_refresh bm25_status"));
    }

    loop {
        runtime.app.pump_pending_events().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for BM25 readiness after applying proposal batch after {}s",
                timeouts.post_apply_index_secs
            )));
        }

        let status = rag
            .bm25_status_with_timeout(remaining)
            .await
            .map_err(|source| Error::HeadlessEvent(format!("BM25 status failed: {source}")))?;
        match status {
            Bm25Status::Ready { docs } => {
                if docs > 0 {
                    runtime.app.pump_pending_events().await;
                    observer.emit(format!(
                        "attempt {turn} sparse_refresh bm25_ready docs={docs}"
                    ));
                    return Ok(true);
                }
                return Err(Error::HeadlessEvent(
                    "BM25 index is empty after applying proposal batch".to_string(),
                ));
            }
            Bm25Status::Empty => {
                return Err(Error::HeadlessEvent(
                    "BM25 index is empty after applying proposal batch".to_string(),
                ));
            }
            Bm25Status::Error(detail) => {
                return Err(Error::HeadlessEvent(format!(
                    "BM25 index failed after applying proposal batch: {detail}"
                )));
            }
            Bm25Status::Uninitialized | Bm25Status::Building => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

pub(super) fn sparse_search_refresh_enabled(
    strategy: &ploke_tui::user_config::RetrievalStrategyUser,
    strict_bm25_by_default: bool,
) -> bool {
    match strategy {
        ploke_tui::user_config::RetrievalStrategyUser::Sparse { strict } => {
            *strict || strict_bm25_by_default
        }
        ploke_tui::user_config::RetrievalStrategyUser::Dense
        | ploke_tui::user_config::RetrievalStrategyUser::Hybrid { .. } => false,
    }
}

async fn wait_for_index_output(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    pending_events: &mut VecDeque<ploke_tui::AppEvent>,
    require_index: bool,
    turn: u32,
    observer: &LiveObserver,
    deadline: Instant,
    timeouts: &harness::Timeouts,
) -> Result<(), Error> {
    use ploke_tui::{AppEvent, app_state::events::SystemEvent};

    let full_deadline = deadline;
    let start_grace = Instant::now() + timeouts.post_apply_index_start_grace();
    let mut saw_index = require_index;

    loop {
        runtime.app.pump_pending_events().await;
        let now = Instant::now();
        if now >= full_deadline {
            return Err(Error::HeadlessEvent(format!(
                "timed out waiting for indexing completion after {}s",
                timeouts.post_apply_index_secs
            )));
        }
        if !saw_index && now >= start_grace {
            observer.emit(format!("attempt {turn} index_barrier no_index_output"));
            return Ok(());
        }

        let deadline = if saw_index {
            full_deadline
        } else {
            start_grace
        };
        let wait_for = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(250));
        let event = match tokio::time::timeout(wait_for, next_event(runtime)).await {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => return Err(err),
            Err(_) => continue,
        };

        match event {
            AppEvent::System(SystemEvent::ReIndex { target }) => {
                saw_index = true;
                observer.emit(format!(
                    "attempt {turn} reindex_scheduled target={target:?}"
                ));
                runtime.app.pump_pending_events().await;
            }
            AppEvent::IndexingStarted | AppEvent::IndexingProgress(_) => {
                saw_index = true;
            }
            AppEvent::IndexingCompleted => {
                runtime.app.pump_pending_events().await;
                observer.emit(format!("attempt {turn} index_barrier completed"));
                return Ok(());
            }
            AppEvent::IndexingFailed => {
                return Err(Error::HeadlessEvent(
                    "indexing failed after applying proposal batch".to_string(),
                ));
            }
            AppEvent::Error(error) if error.message.contains("Indexing failed") => {
                return Err(Error::HeadlessEvent(error.message));
            }
            other => pending_events.push_back(other),
        }
    }
}

pub(super) fn provider_unavailable_reason(content: &str) -> Option<String> {
    let normalized = content.to_ascii_lowercase();
    if normalized.contains("api error")
        && (normalized.contains("status 401")
            || normalized.contains("status 403")
            || normalized.contains("status 429")
            || normalized.contains("key limit exceeded")
            || normalized.contains("rate limit"))
    {
        return Some(content.to_string());
    }
    if normalized.contains("failed to resolve bearer token")
        || normalized.contains("application default credentials")
        || normalized.contains("reauthentication failed")
        || normalized.contains("auth tokens")
    {
        return Some(content.to_string());
    }
    None
}

pub(super) fn provider_failure_from_message(
    kind: ploke_tui::chat_history::MessageKind,
    status: &ploke_tui::chat_history::MessageStatus,
    content: &str,
) -> Option<String> {
    let ploke_tui::chat_history::MessageStatus::Error { description } = status else {
        return None;
    };
    if !matches!(
        kind,
        ploke_tui::chat_history::MessageKind::Assistant
            | ploke_tui::chat_history::MessageKind::System
            | ploke_tui::chat_history::MessageKind::SysInfo
    ) {
        return None;
    }
    provider_unavailable_reason(content).or_else(|| provider_unavailable_reason(description))
}

pub(super) async fn provider_failure_from_chat(
    runtime: &crate::runner::WorkspaceTuiRuntime,
) -> Option<String> {
    let chat = runtime.state.chat.0.read().await;
    chat.messages.values().find_map(|message| {
        provider_failure_from_message(message.kind, &message.status, &message.content)
    })
}

pub(super) fn attempt_prompt(
    _workspace_path: &Path,
    _surface: &SurfacePolicy,
    _evidence_roots: &[EvidenceRoot],
    request_prompt: &str,
    feedback: Option<&str>,
    policy_suffix: Option<&str>,
) -> String {
    let mut prompt = request_prompt.trim_end().to_string();
    if let Some(feedback) = feedback {
        prompt.push_str("\n\nPrevious attempt result:\n");
        prompt.push_str(feedback);
        push_policy_suffix(&mut prompt, policy_suffix);
        prompt.push('\n');
    } else {
        push_policy_suffix(&mut prompt, policy_suffix);
    }
    prompt
}

/// Append the optional anti-attractor policy suffix to the chat-step prompt.
/// Authority boundary: the suffix is plain text only; the function never
/// returns a value that influences selection, admission, or replay. When the
/// suffix is `None` or empty, the prompt is unchanged.
fn push_policy_suffix(prompt: &mut String, policy_suffix: Option<&str>) {
    if let Some(suffix) = policy_suffix {
        if !suffix.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(suffix);
        }
    }
}

pub(super) fn evidence_read_roots(evidence_roots: &[EvidenceRoot]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for root in evidence_roots {
        if root.kind == EvidenceRootKind::SubmittedResultOutput {
            continue;
        }
        if let Some(path) = prototype_navigation_root(root) {
            roots.push(path);
        }
        match &root.location {
            EvidenceRootLocation::Directory { path } => {
                roots.push(path.clone());
            }
            EvidenceRootLocation::File { path } => {
                if let Some(parent) = path.parent() {
                    roots.push(parent.to_path_buf());
                }
            }
            EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. } => {
                roots.push(nodes_root.clone());
            }
            EvidenceRootLocation::AttachedReport { .. } => {}
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn prototype_navigation_root(root: &EvidenceRoot) -> Option<PathBuf> {
    match (&root.kind, &root.location) {
        (
            EvidenceRootKind::Evaluations | EvidenceRootKind::Nodes,
            EvidenceRootLocation::Directory { path },
        ) => path.parent().map(Path::to_path_buf),
        (EvidenceRootKind::HistoryBlocks, EvidenceRootLocation::Directory { path }) => {
            path.parent().and_then(Path::parent).map(Path::to_path_buf)
        }
        (
            EvidenceRootKind::ProtocolArtifacts,
            EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. },
        ) => nodes_root.parent().map(Path::to_path_buf),
        _ => None,
    }
}

pub(super) fn retry_feedback(feedback: &str) -> String {
    #[derive(Deserialize)]
    struct ToolFailure {
        user: String,
    }

    if let Ok(failure) = serde_json::from_str::<ToolFailure>(feedback) {
        return format!("Previous attempt failed: {}", failure.user);
    }

    if feedback.contains("[aborted]") {
        return "Previous attempt aborted before staging an edit.".to_string();
    }

    feedback.to_string()
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LiveObserver {
    enabled: bool,
    resources: bool,
    started: Instant,
}

impl LiveObserver {
    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn from_env() -> Self {
        Self {
            enabled: std::env::var_os(LIVE_TRACE_ENV)
                .and_then(|value| value.into_string().ok())
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
                })
                .unwrap_or(false),
            resources: std::env::var_os("PLOKE_EVAL_HEADLESS_TUI_LIVE_RESOURCES")
                .and_then(|value| value.into_string().ok())
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
                })
                .unwrap_or(false),
            started: Instant::now(),
        }
    }

    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            enabled: false,
            resources: false,
            started: Instant::now(),
        }
    }

    pub(super) fn emit(&self, message: impl AsRef<str>) {
        if self.enabled {
            if self.resources {
                match current_rss_kb() {
                    Some(rss_kb) => eprintln!(
                        "[headless-tui elapsed_ms={} rss_kb={rss_kb}] {}",
                        self.started.elapsed().as_millis(),
                        message.as_ref()
                    ),
                    None => eprintln!(
                        "[headless-tui elapsed_ms={} rss_kb=unknown] {}",
                        self.started.elapsed().as_millis(),
                        message.as_ref()
                    ),
                }
            } else {
                eprintln!(
                    "[headless-tui elapsed_ms={}] {}",
                    self.started.elapsed().as_millis(),
                    message.as_ref()
                );
            }
        }
    }

    pub(in crate::cli::prototype1_state::edit_surface::tui_adapter) fn emit_workspace_size(
        &self,
        label: &str,
        workspace_path: &Path,
    ) {
        if self.enabled && self.resources {
            let workspace = dir_size_limited(workspace_path, 50_000);
            let target = dir_size_limited(&workspace_path.join("target"), 50_000);
            match current_rss_kb() {
                Some(rss_kb) => eprintln!(
                    "[headless-tui elapsed_ms={} rss_kb={rss_kb} {label} workspace_bytes={} workspace_entries={} workspace_truncated={} target_bytes={} target_entries={} target_truncated={}]",
                    self.started.elapsed().as_millis(),
                    workspace.bytes,
                    workspace.entries,
                    workspace.truncated,
                    target.bytes,
                    target.entries,
                    target.truncated,
                ),
                None => eprintln!(
                    "[headless-tui elapsed_ms={} rss_kb=unknown {label} workspace_bytes={} workspace_entries={} workspace_truncated={} target_bytes={} target_entries={} target_truncated={}]",
                    self.started.elapsed().as_millis(),
                    workspace.bytes,
                    workspace.entries,
                    workspace.truncated,
                    target.bytes,
                    target.entries,
                    target.truncated,
                ),
            }
        }
    }
}

#[derive(Debug, Default)]
struct DirSize {
    bytes: u64,
    entries: usize,
    truncated: bool,
}

fn current_rss_kb() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.split_whitespace().next()?.parse::<u64>().ok()
    })
}

fn dir_size_limited(path: &Path, max_entries: usize) -> DirSize {
    let mut size = DirSize::default();
    let mut stack = vec![path.to_path_buf()];
    while let Some(path) = stack.pop() {
        if size.entries >= max_entries {
            size.truncated = true;
            break;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        size.entries += 1;
        if metadata.is_file() {
            size.bytes = size.bytes.saturating_add(metadata.len());
            continue;
        }
        if !metadata.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            stack.push(entry.path());
        }
    }
    size
}

pub(super) async fn submit_prompt(
    app: &ploke_tui::app::App,
    content: String,
) -> Result<Uuid, Error> {
    let new_msg_id = Uuid::new_v4();
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();
    let cmd_tx = app.state_cmd_tx();
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::AddUserMessage {
            content,
            new_user_msg_id: new_msg_id,
            completion_tx,
        },
    )
    .await?;
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::ScanForChange { scan_tx },
    )
    .await?;
    send_state(
        &cmd_tx,
        ploke_tui::app_state::commands::StateCommand::EmbedMessage {
            new_msg_id,
            completion_rx,
            scan_rx,
        },
    )
    .await?;
    Ok(new_msg_id)
}

async fn send_state(
    cmd_tx: &tokio::sync::mpsc::Sender<ploke_tui::app_state::commands::StateCommand>,
    cmd: ploke_tui::app_state::commands::StateCommand,
) -> Result<(), Error> {
    cmd_tx
        .send(cmd)
        .await
        .map_err(|source| Error::HeadlessEvent(format!("state command send failed: {source}")))
}

pub(super) async fn next_event_with_deadline(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
    deadline: Instant,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    turn: u32,
) -> Result<ploke_tui::AppEvent, Error> {
    use tokio::sync::broadcast::error::RecvError;

    loop {
        if Instant::now() >= deadline {
            return Err(Error::HeadlessEvent(
                "event wait deadline exceeded".to_string(),
            ));
        }
        let sleep = tokio::time::sleep_until(deadline.into());
        tokio::pin!(sleep);
        tokio::select! {
            realtime = runtime.realtime_rx.recv() => {
                match realtime {
                    Ok(event) => return Ok(event),
                    Err(RecvError::Lagged(dropped)) => {
                        record_event_lag(run, observer, turn, "realtime", dropped);
                        continue;
                    }
                    Err(source) => return Err(Error::HeadlessEvent(source.to_string())),
                }
            }
            background = runtime.background_rx.recv() => {
                match background {
                    Ok(event) => return Ok(event),
                    Err(RecvError::Lagged(dropped)) => {
                        record_event_lag(run, observer, turn, "background", dropped);
                        continue;
                    }
                    Err(source) => return Err(Error::HeadlessEvent(source.to_string())),
                }
            }
            _ = &mut sleep => {
                return Err(Error::HeadlessEvent(
                    "event wait deadline exceeded".to_string(),
                ));
            }
        }
    }
}

/// Record a broadcast lag so a lag-induced hang-until-deadline is diagnosable
/// after the fact, on both the live trace and the persisted debug relay.
fn record_event_lag(
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    turn: u32,
    channel: &str,
    dropped: u64,
) {
    let message = format!("attempt {turn} event_lag channel={channel} dropped={dropped}");
    observer.emit(&message);
    run.debug_relay.push(&message);
}

pub(super) async fn next_event(
    runtime: &mut crate::runner::WorkspaceTuiRuntime,
) -> Result<ploke_tui::AppEvent, Error> {
    tokio::select! {
        realtime = runtime.realtime_rx.recv() => {
            realtime.map_err(|source| Error::HeadlessEvent(source.to_string()))
        }
        background = runtime.background_rx.recv() => {
            background.map_err(|source| Error::HeadlessEvent(source.to_string()))
        }
    }
}

fn drain_debug(
    debug_rx: &mut tokio::sync::mpsc::Receiver<
        ploke_tui::app::commands::harness::DebugStateCommand,
    >,
    run: &mut HeadlessRun,
) {
    loop {
        match debug_rx.try_recv() {
            Ok(debug) => run.debug_relay.push(debug.as_str()),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}

pub(super) fn drain_debug_observed(
    debug_rx: &mut tokio::sync::mpsc::Receiver<
        ploke_tui::app::commands::harness::DebugStateCommand,
    >,
    run: &mut HeadlessRun,
    observer: &LiveObserver,
    turn: u32,
) {
    loop {
        match debug_rx.try_recv() {
            Ok(debug) => {
                let text = debug.as_str();
                if text.contains("Provider emitted invalid arguments")
                    || text.contains("Repeated repair attempts")
                {
                    observer.emit(format!(
                        "attempt {turn} repair_event {}",
                        truncate_chars(text, 240)
                    ));
                }
                run.debug_relay.push(text);
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}

fn proposal_paths(proposal: &ploke_tui::app_state::core::EditProposal) -> Vec<PathBuf> {
    if !proposal.files.is_empty() {
        return proposal.files.clone();
    }
    let mut paths = proposal
        .edits
        .iter()
        .map(|edit| edit.file_path.clone())
        .collect::<Vec<_>>();
    paths.extend(proposal.edits_ns.iter().map(|edit| edit.file_path.clone()));
    paths
}

pub(super) fn classify_paths(
    workspace_path: &Path,
    surface: &SurfacePolicy,
    paths: &[PathBuf],
) -> Option<Reject> {
    surface.classify_paths(workspace_path, paths)
}

pub(super) fn repair_prompt_feedback(feedback: &str) -> String {
    format!("The headless harness rejected a staged edit before applying it: {feedback}.")
}

pub(super) fn policy_repair_prompt(feedback: &str, has_applied_edits: bool) -> String {
    let mut prompt = String::new();
    prompt.push_str("Previous attempt result:\n");
    prompt.push_str(feedback);
    prompt.push_str("\n\n");
    prompt.push_str(
        "Protected core: see `crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs::EVAL_CORE_SURFACE_ROOT` and `WORKSPACE_EXCEPT_AUTHORITY_*`. Ordinary edits touching that surface will be rejected.\n",
    );
    if has_applied_edits {
        prompt
            .push_str("\nThe workspace already contains allowed edits from earlier tool calls.\n");
    } else {
        prompt.push_str("\nNo allowed source edit has been applied yet.\n");
    }
    prompt
}

#[cfg(test)]
mod attempt_prompt_tests {
    //! Unit tests for the optional anti-attractor policy suffix in
    //! [`super::attempt_prompt`]. These exercise the read-side / prompt-side
    //! contract only: the suffix is plain text, appended at the end of the
    //! chat-step prompt, and never affects selection, admission, or replay.

    use super::attempt_prompt;
    use crate::cli::prototype1_state::edit_surface::surface_policy::SurfacePolicy;

    fn dummy_surface() -> SurfacePolicy {
        SurfacePolicy::workspace_except_core()
    }

    #[test]
    fn no_policy_suffix_leaves_prompt_unchanged() {
        let prompt = "the request body";
        let result = attempt_prompt(
            std::path::Path::new("."),
            &dummy_surface(),
            &[],
            prompt,
            None,
            None,
        );
        assert_eq!(result, prompt);
    }

    #[test]
    fn policy_suffix_is_appended_with_blank_line_separator() {
        let prompt = "the request body";
        let suffix = "Use a different region of the codebase.";
        let result = attempt_prompt(
            std::path::Path::new("."),
            &dummy_surface(),
            &[],
            prompt,
            None,
            Some(suffix),
        );
        assert!(
            result.ends_with(suffix),
            "suffix must be present at the end of the prompt, got:\n{result}"
        );
        assert!(
            result.contains(&format!("\n\n{suffix}")),
            "expected double-newline separator before suffix, got:\n{result}"
        );
    }

    #[test]
    fn empty_policy_suffix_is_a_no_op() {
        let prompt = "the request body";
        let result = attempt_prompt(
            std::path::Path::new("."),
            &dummy_surface(),
            &[],
            prompt,
            None,
            Some(""),
        );
        assert_eq!(result, prompt);
    }

    #[test]
    fn policy_suffix_coexists_with_feedback() {
        let prompt = "the request body";
        let feedback = "applied edit to file X";
        let suffix = "Try a different file next.";
        let result = attempt_prompt(
            std::path::Path::new("."),
            &dummy_surface(),
            &[],
            prompt,
            Some(feedback),
            Some(suffix),
        );
        assert!(result.contains(feedback));
        assert!(result.contains(suffix));
        // Suffix must still come at the end so the LLM sees the bias last.
        let suffix_pos = result.find(suffix).expect("suffix present");
        let feedback_pos = result.find(feedback).expect("feedback present");
        assert!(
            suffix_pos > feedback_pos,
            "suffix must come after feedback in the prompt"
        );
    }
}
