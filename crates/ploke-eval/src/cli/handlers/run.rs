use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;

use ploke_llm::{ModelId, ProviderKey};
use serde::Serialize;

use crate::cli::prototype1_state;
use crate::cli::provider::{
    headless_model_selection, load_parent_patcher_model_selection, parse_provider_key,
};
use crate::cli::{
    FetchMsbRepoCommand, InspectOutputFormat, PrepareMsbBatchCommand, PrepareMsbSingleCommand,
    PrepareSingleCommand, ReplayInspectCommand, ReplayMsbBatchCommand, ReplaySelfEditLiveCommand,
    ReplayTurnLiveCommand, RunListCommand, RunMsbAgentBatchCommand, RunMsbAgentSingleCommand,
    RunMsbBatchCommand, RunMsbSingleCommand, has_legacy_instance_root_artifacts,
    legacy_instance_root_warning, list_attempt_registrations, sanitize_batch_component,
    selection_with_resolution, truncate_for_table, truncate_middle,
};
use crate::layout::{batches_dir, instances_dir, repos_dir, workspace_root_for_key};
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
use crate::projection::OperatorProjectionRead;
use crate::registry::builtin_dataset_registry_entry;
use crate::runner::{
    ReplayMsbBatchRequest, RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest,
    RunMsbSingleRequest,
};
use crate::selection::{load_active_selection, render_selection_warnings};
use crate::spec::{EvalBudget, IssueInput, PrepareError, PrepareSingleRunRequest, PrepareWrite};

impl PrepareSingleCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = PrepareSingleRunRequest {
            task_id: self.task_id,
            repo_root: self.repo,
            issue: IssueInput {
                title: self.issue_title,
                body: self.issue_body,
                body_path: self.issue_file,
            },
            output_dir: self.out_dir,
            base_sha: self.base_sha,
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        };

        let prepared = request.prepare()?;
        let write = if self.stdout {
            PrepareWrite::Stdout
        } else {
            PrepareWrite::File(prepared.manifest_path())
        };
        prepared.write_manifest(self.output_mode, write)
    }
}

impl PrepareMsbSingleCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let prepared = PrepareMsbSingleRunRequest {
            dataset_file: self.dataset,
            dataset_key: self.dataset_key,
            instance_id: self.instance,
            repo_cache: self.repo_cache.unwrap_or(repos_dir()?),
            instances_root: self.instances_root.unwrap_or(instances_dir()?),
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        }
        .prepare()?;

        let write = if self.stdout {
            PrepareWrite::Stdout
        } else {
            PrepareWrite::File(prepared.manifest_path())
        };
        prepared.write_manifest(self.output_mode, write)
    }
}

impl PrepareMsbBatchCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let batch_id = self.batch_id.unwrap_or_else(|| {
            default_batch_id(
                self.dataset_key.as_deref(),
                self.dataset.as_ref(),
                self.all,
                &self.instance,
                &self.specific,
            )
        });
        let prepared = PrepareMsbBatchRequest {
            dataset_file: self.dataset,
            dataset_key: self.dataset_key,
            batch_id,
            select_all: self.all,
            instance_ids: self.instance,
            specifics: self.specific,
            limit: self.limit,
            repo_cache: self.repo_cache.unwrap_or(repos_dir()?),
            instances_root: self.instances_root.unwrap_or(instances_dir()?),
            batches_root: self.batches_root.unwrap_or(batches_dir()?),
            budget: EvalBudget {
                max_turns: self.max_turns,
                max_tool_calls: self.max_tool_calls,
                wall_clock_secs: self.wall_clock_secs,
            },
        }
        .prepare()?;

        for run in &prepared.runs {
            run.write_manifest(self.output_mode, PrepareWrite::File(run.manifest_path()))?;
        }
        prepared.batch.write_manifest(self.output_mode)?;
        println!("{}", prepared.batch.manifest_path().display());
        Ok(())
    }
}
impl RunMsbSingleCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbSingleRequest {
            run_manifest,
            batch_id: None,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
        }
        .run()
        .await?;
        println!("{}", artifacts.execution_log.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl RunMsbBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let batch_manifest = resolve_batch_manifest(self.batch, self.batch_id)?;
        let artifacts = RunMsbBatchRequest {
            batch_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
            stop_on_error: self.stop_on_error,
        }
        .run()
        .await?;
        println!("{}", artifacts.summary.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}

impl RunMsbAgentSingleCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        let artifacts = RunMsbAgentSingleRequest {
            run_manifest,
            batch_id: None,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
            embedding_model_id: self.embedding_model_id,
            embedding_provider: parse_provider_key(self.embedding_provider)?,
        }
        .run()
        .await?;
        #[cfg(not(feature = "demo"))]
        {
            println!("{}", artifacts.base.execution_log.display());
            println!("{}", artifacts.turn_summary.display());
            if let Some(path) = artifacts.base.full_response_trace {
                println!("{}", path.display());
            }
            if let Some(path) = artifacts.base.msb_submission {
                println!("{}", path.display());
            }
        }
        #[cfg(feature = "demo")]
        let _ = artifacts;
        Ok(())
    }
}

impl RunMsbAgentBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let batch_manifest = resolve_batch_manifest(self.batch, self.batch_id)?;
        let artifacts = RunMsbAgentBatchRequest {
            batch_manifest,
            index_debug_snapshots: self.index_debug_snapshots,
            use_default_model: self.use_default_model,
            model_id: self.model_id,
            provider: parse_provider_key(self.provider)?,
            embedding_model_id: None,
            embedding_provider: None,
            stop_on_error: self.stop_on_error,
        }
        .run()
        .await?;
        println!("{}", artifacts.summary.display());
        if let Some(path) = artifacts.msb_submission {
            println!("{}", path.display());
        }
        Ok(())
    }
}
impl ReplayMsbBatchCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let run_manifest = match (self.run, self.instance) {
            (Some(path), None) => path,
            (None, Some(instance)) => instances_dir()?.join(instance).join("run.json"),
            _ => {
                return Err(PrepareError::MissingRunManifest(
                    instances_dir()?.join("<instance>/run.json"),
                ));
            }
        };

        ReplayMsbBatchRequest {
            run_manifest,
            batch_number: self.batch,
        }
        .run()
        .await
        .map(|batch_file| {
            println!("{}", batch_file.display());
        })
    }
}

impl ReplayTurnLiveCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        // CLI responsibility stops at flag parsing, model selection, and
        // rendering. Cursor resolution, tape installation, and headless TUI
        // execution live in `replay::turn` / `replay::probe` so future selector
        // commands can reuse the same probe body without copying semantics.
        let tail: crate::replay::turn::ReplayTail = self.tail.into();
        let model = match (tail, self.model_id, self.provider) {
            (crate::replay::turn::ReplayTail::Stop, None, None) => None,
            (_, model_id, provider) => {
                Some(resolve_replay_probe_model_selection(model_id, provider)?)
            }
        };
        let cursor = ploke_tree::TurnCursor {
            artifact_kind: self.artifact_kind.into(),
            artifact_path: self.artifact_path,
            event_index: self.event_index,
        };
        let prefix_selector = if let Some(response_index) = self.through_response_index {
            crate::replay::turn::ReplayPrefixSelector::ThroughResponseIndex { response_index }
        } else if self.through_event {
            crate::replay::turn::ReplayPrefixSelector::ThroughEvent {
                cursor: cursor.clone(),
            }
        } else {
            crate::replay::turn::ReplayPrefixSelector::FullTape
        };
        let probe = crate::replay::probe::ProbeRequest {
            run_dir: self.run_dir,
            workspace: self.workspace,
            cursor,
            prefix_selector,
            tail,
            budget: crate::replay::probe::ProbeBudget::new(self.max_attempts, self.timeout_secs),
            model,
            branch: self
                .branch_in
                .as_deref()
                .map(crate::replay::probe::ReplayBranchTape::load)
                .transpose()?,
            branch_out: self.branch_out,
        }
        .run()
        .await?;
        print_replay_probe(&probe, self.format)
    }
}

impl ReplaySelfEditLiveCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let tail: crate::replay::turn::ReplayTail = self.tail.into();
        let model = match (tail, self.model_id, self.provider) {
            (crate::replay::turn::ReplayTail::Stop, None, None) => None,
            (_, model_id, provider) => {
                Some(resolve_replay_probe_model_selection(model_id, provider)?)
            }
        };
        let budget = prototype1_state::edit_surface::tui_adapter::Budget::new(
            self.max_attempts,
            self.timeout_secs,
        )
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "self_edit_replay_budget",
            detail: source.to_string(),
        })?;
        let probe = crate::replay::self_edit::SelfEditProbeRequest {
            request_path: self.request,
            source: match (self.result, self.raw_full_response) {
                (Some(path), None) => crate::replay::self_edit::Source::HeadlessResult { path },
                (None, Some(path)) => crate::replay::self_edit::Source::RawFullResponse { path },
                _ => {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: "provide exactly one of --result or --raw-full-response"
                            .to_string(),
                    });
                }
            },
            workspace: self.workspace,
            event_index: self.event_index.unwrap_or(0),
            through_event: self.through_event,
            through_response_index: self
                .through_response_index
                .map(ploke_llm::manager::ResponseIndex::new),
            tail,
            budget,
            model,
        }
        .run()
        .await?;
        print_self_edit_probe(&probe, self.format)
    }
}

impl ReplayInspectCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let limit = self.limit;
        let tool_events_only = self.tool_events_only;
        let inspection = crate::replay::inspect::ReplayInspectRequest {
            run_dir: self.run_dir,
            workspace: self.workspace,
        }
        .inspect()?;
        print_replay_inspection(&inspection, self.format, limit, tool_events_only)
    }
}
fn resolve_replay_probe_model_selection(
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<prototype1_state::edit_surface::tui_adapter::ModelSelection, PrepareError> {
    match (model_id, provider) {
        (Some(model_id), provider) => {
            let model_id =
                ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
                    phase: "replay_probe_model_id",
                    detail: format!("invalid model id '{model_id}': {err}"),
                })?;
            let provider = provider
                .map(|provider| {
                    ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
                        phase: "replay_probe_provider",
                        detail: format!("invalid provider slug '{provider}': {err}"),
                    })
                })
                .transpose()?;
            headless_model_selection(model_id, provider)
        }
        (None, Some(provider)) => Err(PrepareError::InvalidBatchSelection {
            detail: format!("replay turn-live provider '{provider}' requires --model-id"),
        }),
        (None, None) => load_parent_patcher_model_selection(),
    }
}

fn print_replay_inspection(
    inspection: &crate::replay::inspect::ReplayInspection,
    format: InspectOutputFormat,
    limit: Option<usize>,
    tool_events_only: bool,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(inspection).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("replay inspection");
            println!("{}", "-".repeat(40));
            println!("run_dir: {}", inspection.run_dir.display());
            match &inspection.workspace {
                Some(workspace) => println!(
                    "workspace: {} (exists: {})",
                    workspace.display(),
                    inspection.workspace_exists.unwrap_or(false)
                ),
                None => println!("workspace: (not provided)"),
            }
            println!("steps: {}", inspection.step_count());
            println!("quality_signals: {}", inspection.signal_count());
            if tool_events_only {
                println!("filter: tool events only");
            }
            let prompt_paths = replay_prompt_path_summary(inspection);
            if !prompt_paths.is_empty() {
                println!("prompt_paths:");
                for path in prompt_paths.iter().take(4) {
                    println!("  {path}");
                }
                if prompt_paths.len() > 4 {
                    println!("  ... {} more path(s)", prompt_paths.len() - 4);
                }
            }
            println!();
            println!(
                "{:<36} {:<16} {:<22} {:<14} {:<18} {:<24} signals",
                "cursor", "event", "tool", "call_id", "assistant", "tape"
            );
            let rows = inspection
                .steps
                .iter()
                .filter(|step| !tool_events_only || is_replay_tool_event(step.event_kind))
                .collect::<Vec<_>>();
            for step in rows.iter().take(limit.unwrap_or(usize::MAX)) {
                println!(
                    "{:<36} {:<16} {:<22} {:<14} {:<18} {:<24} {}",
                    format_replay_cursor(&step.cursor),
                    format!("{:?}", step.event_kind),
                    step.tool_name.as_deref().unwrap_or("-"),
                    step.call_id.as_deref().unwrap_or("-"),
                    step.assistant_message_id
                        .as_deref()
                        .map(short_id)
                        .unwrap_or_else(|| "-".to_owned()),
                    format_tape_cell(step.tape.as_ref()),
                    format_signal_cell(&step.quality_signals),
                );
            }
            if let Some(limit) = limit {
                if rows.len() > limit {
                    println!("... {} more step(s) omitted by --limit", rows.len() - limit);
                }
            }
        }
    }
    Ok(())
}

fn print_replay_probe(
    probe: &crate::replay::probe::ProbeRun,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    // Render only the library projection. Do not derive replay semantics here;
    // `ProbeRun::tail_reached` is the authority for whether the live provider
    // boundary was crossed after the recorded prefix.
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(probe).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            for line in crate::replay::probe_text::render_table(probe) {
                println!("{line}");
            }
        }
    }
    Ok(())
}

fn print_self_edit_probe(
    probe: &crate::replay::self_edit::SelfEditProbeRun,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(probe).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!("self_edit_replay");
            println!("  request: {}", probe.request_path.display());
            println!(
                "  source: {} {}",
                probe.source_kind,
                probe.source_path.display()
            );
            println!("  workspace: {}", probe.workspace.display());
            println!("  selected_events: {}", probe.selected_events);
            if let Some(records) = probe.selected_response_records {
                println!("  selected_response_records: {records}");
            }
            println!("  selected_tool_requests: {}", probe.selected_tool_requests);
            println!("  installed_records: {}", probe.installed_records);
            println!("  terminal: {}", probe.terminal);
            if !probe.historical_failures.is_empty() {
                println!("historical_failures_before_breakpoint:");
                for failure in &probe.historical_failures {
                    println!(
                        "  event={} call={} {}",
                        failure.event_index,
                        failure.call_id,
                        truncate_middle(&failure.preview, 180)
                    );
                }
            }
            println!("observed_tool_events:");
            let mut count = 0_usize;
            for event in &probe.observed.events {
                match event {
                    prototype1_state::edit_surface::tui_adapter::evidence::Event::ToolRequest {
                        call_id,
                        tool,
                        arguments,
                        ..
                    } => {
                        count += 1;
                        println!(
                            "  request call={} tool={} args={}",
                            call_id,
                            tool,
                            truncate_middle(&arguments.preview, 180)
                        );
                    }
                    prototype1_state::edit_surface::tui_adapter::evidence::Event::ToolCompleted {
                        call_id,
                        content,
                    } => {
                        count += 1;
                        println!(
                            "  completed call={} result={}",
                            call_id,
                            truncate_middle(&content.preview, 220)
                        );
                    }
                    prototype1_state::edit_surface::tui_adapter::evidence::Event::ToolFailed {
                        call_id,
                        error,
                    } => {
                        count += 1;
                        println!(
                            "  failed call={} error={}",
                            call_id,
                            truncate_middle(&error.preview, 220)
                        );
                    }
                    _ => {}
                }
            }
            if count == 0 {
                println!("  (none)");
            }
        }
    }
    Ok(())
}

fn format_replay_cursor(cursor: &ploke_tree::TurnCursor) -> String {
    format!(
        "{:?}:{}:{}",
        cursor.artifact_kind, cursor.artifact_path, cursor.event_index
    )
}

fn is_replay_tool_event(kind: ploke_tree::TurnEventKind) -> bool {
    matches!(
        kind,
        ploke_tree::TurnEventKind::ToolRequested
            | ploke_tree::TurnEventKind::ToolCompleted
            | ploke_tree::TurnEventKind::ToolFailed
    )
}

fn replay_prompt_path_summary(
    inspection: &crate::replay::inspect::ReplayInspection,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for path in inspection
        .steps
        .iter()
        .flat_map(|step| step.prompt_paths.iter())
    {
        let exists = if path.exists { "exists" } else { "missing" };
        let target = if path.target_workspace {
            "target"
        } else {
            "not-target"
        };
        paths.insert(format!("{} ({exists}, {target})", path.path.display()));
    }
    paths.into_iter().collect()
}

fn format_tape_cell(tape: Option<&crate::replay::inspect::TapeContinuity>) -> String {
    match tape {
        None => "-".to_owned(),
        Some(crate::replay::inspect::TapeContinuity::Present {
            response_indices,
            missing_response_indices,
            ..
        }) if missing_response_indices.is_empty() => {
            format!(
                "{} rec {}",
                response_indices.len(),
                format_response_indices(response_indices)
            )
        }
        Some(crate::replay::inspect::TapeContinuity::Present {
            response_indices,
            missing_response_indices,
            ..
        }) => format!(
            "{} rec missing {}",
            response_indices.len(),
            format_response_indices(missing_response_indices)
        ),
        Some(crate::replay::inspect::TapeContinuity::Unavailable { .. }) => {
            "unavailable".to_owned()
        }
    }
}

fn format_response_indices(indices: &[usize]) -> String {
    match indices {
        [] => "[]".to_owned(),
        [single] => single.to_string(),
        [first, .., last] if is_contiguous_usize(indices) => format!("{first}..{last}"),
        _ if indices.len() <= 6 => format!("{indices:?}"),
        _ => {
            let first = indices[0];
            let second = indices[1];
            let third = indices[2];
            let last = indices[indices.len() - 1];
            format!("[{first}, {second}, {third}, .., {last}]")
        }
    }
}

fn is_contiguous_usize(indices: &[usize]) -> bool {
    indices.windows(2).all(|window| window[1] == window[0] + 1)
}

fn format_signal_cell(signals: &[crate::replay::inspect::ReplayQualitySignal]) -> String {
    if signals.is_empty() {
        return "-".to_owned();
    }
    signals
        .iter()
        .take(3)
        .map(|signal| replay_signal_label(signal.kind))
        .collect::<Vec<_>>()
        .join(",")
}

fn replay_signal_label(kind: crate::replay::inspect::ReplayQualitySignalKind) -> &'static str {
    match kind {
        crate::replay::inspect::ReplayQualitySignalKind::TapeUnavailable => "tape_unavailable",
        crate::replay::inspect::ReplayQualitySignalKind::TapeGap => "tape_gap",
        crate::replay::inspect::ReplayQualitySignalKind::PromptPathUnreadable => {
            "prompt_path_unreadable"
        }
        crate::replay::inspect::ReplayQualitySignalKind::PromptTargetMismatch => {
            "prompt_target_mismatch"
        }
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
impl RunListCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let selection = load_active_selection(OperatorProjectionRead::cli_operator())?;
        let instance = self
            .instance
            .clone()
            .or_else(|| selection.instance.clone())
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "run_list",
                detail:
                    "instance is required (pass --instance or `ploke-eval select instance <id>`)"
                        .to_string(),
            })?;
        let registrations = list_attempt_registrations(&instances_dir()?, &instance)?;
        let mut warnings = render_selection_warnings(&selection_with_resolution(
            &selection,
            Some(&instance),
            None,
        ));
        if !registrations.is_empty()
            && has_legacy_instance_root_artifacts(&instances_dir()?, &instance)
        {
            warnings.push(legacy_instance_root_warning(&instance));
        }
        let rows: Vec<_> = registrations
            .iter()
            .enumerate()
            .map(|(index, registration)| RunListRow {
                attempt: (index + 1) as u32,
                latest: index + 1 == registrations.len(),
                execution_status: registration.lifecycle.execution_status,
                submission_status: registration.lifecycle.submission_status,
                run_arm_id: registration.frozen_spec.run_arm_id.clone(),
                model_id: registration.frozen_spec.model_id.clone(),
                provider_slug: registration.frozen_spec.provider_slug.clone(),
                started_at: registration.lifecycle.started_at.clone(),
                finished_at: registration.lifecycle.finished_at.clone(),
                run_root: registration.artifacts.run_root.clone(),
            })
            .collect();

        match self.format {
            InspectOutputFormat::Table => {
                if rows.is_empty() {
                    println!("instance {} has no registered attempts.", instance);
                } else {
                    println!(
                        "{:<7} {:<6} {:<11} {:<13} {:<28} {:<12} {:<10} {}",
                        "Attempt",
                        "Latest",
                        "Execution",
                        "Submission",
                        "Arm",
                        "Provider",
                        "Model",
                        "Finished"
                    );
                    println!("{}", "-".repeat(120));
                    for row in &rows {
                        println!(
                            "{:<7} {:<6} {:<11} {:<13} {:<28} {:<12} {:<10} {}",
                            row.attempt,
                            if row.latest { "yes" } else { "" },
                            execution_status_label(row.execution_status),
                            submission_status_label(row.submission_status),
                            truncate_for_table(&row.run_arm_id, 26),
                            truncate_for_table(row.provider_slug.as_deref().unwrap_or("-"), 10),
                            truncate_for_table(row.model_id.as_deref().unwrap_or("-"), 8),
                            row.finished_at.as_deref().unwrap_or("-"),
                        );
                    }
                    println!("\ninstance: {}", instance);
                    println!(
                        "latest attempt: {}",
                        rows.last().map(|row| row.attempt).unwrap_or(0)
                    );
                }
                for warning in warnings {
                    println!("warning: {warning}");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "instance": instance,
                    "selection": selection,
                    "warnings": warnings,
                    "runs": rows,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize)]
struct RunListRow {
    attempt: u32,
    latest: bool,
    execution_status: crate::run_registry::RunExecutionStatus,
    submission_status: crate::run_registry::RunSubmissionStatus,
    run_arm_id: String,
    model_id: Option<String>,
    provider_slug: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    run_root: PathBuf,
}
fn execution_status_label(status: crate::run_registry::RunExecutionStatus) -> &'static str {
    match status {
        crate::run_registry::RunExecutionStatus::Registered => "registered",
        crate::run_registry::RunExecutionStatus::Running => "running",
        crate::run_registry::RunExecutionStatus::Completed => "completed",
        crate::run_registry::RunExecutionStatus::Failed => "failed",
    }
}

fn submission_status_label(status: crate::run_registry::RunSubmissionStatus) -> &'static str {
    match status {
        crate::run_registry::RunSubmissionStatus::Missing => "missing",
        crate::run_registry::RunSubmissionStatus::EmptyPatch => "empty_patch",
        crate::run_registry::RunSubmissionStatus::NonemptyPatch => "nonempty_patch",
    }
}

pub(crate) fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(crate) fn resolve_batch_manifest(
    batch: Option<PathBuf>,
    batch_id: Option<String>,
) -> Result<PathBuf, PrepareError> {
    match (batch, batch_id) {
        (Some(path), None) => Ok(path),
        (None, Some(batch_id)) => Ok(batches_dir()?.join(batch_id).join("batch.json")),
        _ => Err(PrepareError::MissingBatchManifest(
            batches_dir()?.join("<batch-id>/batch.json"),
        )),
    }
}

pub(crate) fn default_batch_id(
    dataset_key: Option<&str>,
    dataset: Option<&PathBuf>,
    select_all: bool,
    instances: &[String],
    specifics: &[String],
) -> String {
    let dataset_stem = dataset_key
        .map(str::to_string)
        .or_else(|| {
            dataset
                .and_then(|path| path.file_stem())
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "msb".to_string());
    let selector = if select_all {
        "all".to_string()
    } else if instances.len() == 1 && specifics.is_empty() {
        sanitize_batch_component(&instances[0])
    } else if specifics.len() == 1 && instances.is_empty() {
        sanitize_batch_component(&specifics[0])
    } else {
        format!("selection-{}", instances.len() + specifics.len())
    };
    format!("{}-{}", sanitize_batch_component(&dataset_stem), selector)
}
impl FetchMsbRepoCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let entry = builtin_dataset_registry_entry(&self.dataset_key)
            .ok_or_else(|| PrepareError::UnknownDatasetKey(self.dataset_key.clone()))?;

        let repo_root = workspace_root_for_key(&self.dataset_key)?;
        let parent = repo_root
            .parent()
            .expect("repo root built from repos_dir/org/repo always has a parent");
        std::fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;

        if repo_root.join(".git").exists() {
            run_git(
                &[
                    "-C",
                    repo_root.to_string_lossy().as_ref(),
                    "fetch",
                    "--all",
                    "--tags",
                    "--prune",
                ],
                format!("git -C {} fetch --all --tags --prune", repo_root.display()),
            )?;
        } else {
            run_git(
                &[
                    "clone",
                    entry.clone_url().as_str(),
                    repo_root.to_string_lossy().as_ref(),
                ],
                format!("git clone {} {}", entry.clone_url(), repo_root.display()),
            )?;
        }

        println!("{}", repo_root.display());
        Ok(())
    }
}

fn run_git(args: &[&str], command_label: String) -> Result<(), PrepareError> {
    let status = std::process::Command::new("git")
        .args(args)
        .status()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: status.code().unwrap_or(-1),
        })
    }
}
