use std::path::Path;
use std::sync::Arc;

use ploke_protocol::procedure::{
    ProcedureDebugEvent, ProcedureDebugEventKind, ProcedureDebugSink, set_procedure_debug_sink,
};
use ploke_protocol::tool_calls::segment;
use ploke_protocol::{JsonAdjudicator, Procedure, ProtocolReasoningPolicy};
use ploke_records::protocol::InterventionIssueDetectionArtifact;

use crate::campaign::default_protocol_max_tokens;
use crate::cli::ProtocolNextStep;
use crate::cli::{
    InspectOutputFormat, PROTOCOL_HTTP_MAX_ATTEMPTS, ProtocolCommand,
    ProtocolIssueDetectionCommand, ProtocolRunCommand, ProtocolStatusCommand, ProtocolSubcommand,
    ProtocolToolCallIntentSegmentsCommand, ProtocolToolCallReviewCommand,
    ProtocolToolCallSegmentReviewCommand, TOOL_CALL_REVIEW_TIMEOUT_SECS,
    build_segment_review_subject, build_tool_call_review_subject, build_tool_call_sequence_subject,
    execute_protocol_intent_segments_quiet, execute_protocol_tool_call_review_quiet,
    execute_protocol_tool_call_segment_review_quiet, join_indices, print_protocol_state_table,
    print_record_resolution_footer, protocol_llm_config, protocol_next_command,
    protocol_state_for_run, resolve_record_path, run_tool_call_review_with_json_retries,
    run_tool_call_segment_review_with_json_retries, serde_name,
    tool_call_intent_segmentation_error_to_prepare,
};
use crate::intervention::{
    INTERVENTION_ISSUE_DETECTION_PROCEDURE, IssueCase, IssueDetectionInput, IssueDetectionOutput,
    detect_issue_cases, issue_detection_artifact_input, select_primary_issue,
};
use crate::protocol::protocol_aggregate::load_protocol_aggregate;
use crate::protocol_artifacts::write_protocol_artifact;
use crate::record::read_compressed_record;
use crate::spec::PrepareError;

pub(crate) fn persist_issue_detection_for_record(
    record_path: &Path,
) -> Result<IssueDetectionOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let protocol_aggregate = load_protocol_aggregate(record_path).ok();
    let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
    let persisted_input = issue_detection_artifact_input(&detection_input);
    let output = detect_issue_cases(&detection_input);
    let artifact = build_issue_detection_artifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_ISSUE_DETECTION_PROCEDURE,
        &subject_id,
        None,
        None,
        &persisted_input,
        &output,
        &artifact,
    )?;
    Ok(output)
}

impl ProtocolCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            ProtocolSubcommand::Status(cmd) => cmd.run().await,
            ProtocolSubcommand::Run(cmd) => cmd.run().await,
            ProtocolSubcommand::IssueDetection(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallReview(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallIntentSegments(cmd) => cmd.run().await,
            ProtocolSubcommand::ToolCallSegmentReview(cmd) => cmd.run().await,
        }
    }
}

impl ProtocolToolCallReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_review_subject(&record, self.index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            TOOL_CALL_REVIEW_TIMEOUT_SECS,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let reviewed =
            run_tool_call_review_with_json_retries(subject, &cfg, &client, self.index).await?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                if let Some(focal_index) = review.packet.focal_call_index {
                    println!("Focal call index: {}", focal_index);
                }
                println!("Calls:");
                for call in &review.packet.calls {
                    let marker = if Some(call.index) == review.packet.focal_call_index {
                        "focal"
                    } else {
                        "scope"
                    };
                    println!(
                        "  {:<6} [{}] {} | {}",
                        marker, call.index, call.tool_name, call.summary
                    );
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!(
                    "Similar searches: {}",
                    review.signals.similar_search_neighbors
                );
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut state = protocol_state_for_run(&instance_id, &record_path)?;
        state.next_command =
            protocol_next_command(&instance_id, &record_path, &state.next_step, true, false);

        match self.format {
            InspectOutputFormat::Table => {
                print_protocol_state_table(&state);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolRunCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;
        let instance_id = record.metadata.benchmark.instance_id.clone();
        let mut before = protocol_state_for_run(&instance_id, &record_path)?;
        before.next_command =
            protocol_next_command(&instance_id, &record_path, &before.next_step, true, false);

        let progress_guard = match self.format {
            InspectOutputFormat::Table => Some(install_protocol_progress_printer()),
            InspectOutputFormat::Json => None,
        };

        let executed = match before.next_step.clone() {
            ProtocolNextStep::Ineligible => None,
            ProtocolNextStep::IntentSegmentation => {
                execute_protocol_intent_segments_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some("tool_call_intent_segmentation".to_string())
            }
            ProtocolNextStep::ToolCallReview { index } => {
                execute_protocol_tool_call_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    index,
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some(format!("tool_call_review[{index}]"))
            }
            ProtocolNextStep::ToolCallSegmentReview { segment_index } => {
                execute_protocol_tool_call_segment_review_quiet(
                    &record_path,
                    self.model_id.clone(),
                    self.route_source,
                    self.provider.clone(),
                    segment_index,
                    default_protocol_max_tokens(),
                    ProtocolReasoningPolicy::default(),
                )
                .await?;
                Some(format!("tool_call_segment_review[{segment_index}]"))
            }
            ProtocolNextStep::Complete | ProtocolNextStep::Blocked => None,
        };

        let mut after = protocol_state_for_run(&instance_id, &record_path)?;
        after.next_command =
            protocol_next_command(&instance_id, &record_path, &after.next_step, true, false);

        drop(progress_guard);

        match self.format {
            InspectOutputFormat::Table => {
                if let Some(executed) = &executed {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: {}", executed);
                    println!();
                } else {
                    println!("protocol run");
                    println!("{}", "-".repeat(40));
                    println!("executed: (none)");
                    println!();
                }
                print_protocol_state_table(&after);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "executed": executed,
                    "before": before,
                    "after": after,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolIssueDetectionCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject_id = record.metadata.benchmark.instance_id.clone();
        let protocol_aggregate = load_protocol_aggregate(&record_path).ok();
        let detection_input = IssueDetectionInput::from_record(record, protocol_aggregate);
        let persisted_input = issue_detection_artifact_input(&detection_input);
        let output = detect_issue_cases(&detection_input);
        let artifact = build_issue_detection_artifact(&output);
        let persisted_path = write_protocol_artifact(
            &record_path,
            INTERVENTION_ISSUE_DETECTION_PROCEDURE,
            &subject_id,
            None,
            None,
            &persisted_input,
            &output,
            &artifact,
        )?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", INTERVENTION_ISSUE_DETECTION_PROCEDURE);
                println!("{}", "-".repeat(40));
                println!("Artifact: {}", persisted_path.display());
                println!("Cases: {}", output.cases.len());
                if let Some(primary) = select_primary_issue(&output) {
                    print_issue_case_block("Primary issue", &primary);
                } else {
                    println!("Primary issue: (none)");
                }
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": INTERVENTION_ISSUE_DETECTION_PROCEDURE,
                    "persisted_artifact_path": persisted_path,
                    "output": output,
                    "artifact": artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

struct ProtocolProgressGuard;

impl Drop for ProtocolProgressGuard {
    fn drop(&mut self) {
        set_procedure_debug_sink(None);
    }
}

fn install_protocol_progress_printer() -> ProtocolProgressGuard {
    let sink: ProcedureDebugSink = Arc::new(|event: &ProcedureDebugEvent| match event.event {
        ProcedureDebugEventKind::ProcedureStarted => {
            if event.request_label.is_none() {
                eprintln!("protocol progress: {} started", event.procedure_name);
            }
        }
        ProcedureDebugEventKind::ProcedureFinished => {
            if event.request_label.is_none() {
                if let Some(elapsed_ms) = event.elapsed_ms {
                    eprintln!(
                        "protocol progress: {} finished ({} ms)",
                        event.procedure_name, elapsed_ms
                    );
                } else {
                    eprintln!("protocol progress: {} finished", event.procedure_name);
                }
            }
        }
        ProcedureDebugEventKind::ProcedureFailed => {
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            if let Some(elapsed_ms) = event.elapsed_ms {
                eprintln!(
                    "protocol progress: {} failed after {} ms: {}",
                    event.procedure_name, elapsed_ms, detail
                );
            } else {
                eprintln!(
                    "protocol progress: {} failed: {}",
                    event.procedure_name, detail
                );
            }
        }
        ProcedureDebugEventKind::SubrequestStarted => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total) {
                (Some(index), Some(total)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) sent",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) sent", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFinished => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received ({} ms)",
                        index, total, label, elapsed_ms
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) received",
                        index, total, label
                    );
                }
                _ => eprintln!("protocol progress: model request ({}) received", label),
            }
        }
        ProcedureDebugEventKind::SubrequestFailed => {
            let label = event.request_label.as_deref().unwrap_or("model_request");
            let detail = event.detail.as_deref().unwrap_or("unknown error");
            match (event.request_index, event.request_total, event.elapsed_ms) {
                (Some(index), Some(total), Some(elapsed_ms)) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed after {} ms: {}",
                        index, total, label, elapsed_ms, detail
                    );
                }
                (Some(index), Some(total), None) => {
                    eprintln!(
                        "protocol progress: model request {}/{} ({}) failed: {}",
                        index, total, label, detail
                    );
                }
                _ => eprintln!(
                    "protocol progress: model request ({}) failed: {}",
                    label, detail
                ),
            }
        }
    });

    set_procedure_debug_sink(Some(sink));
    ProtocolProgressGuard
}

fn build_issue_detection_artifact(
    output: &IssueDetectionOutput,
) -> InterventionIssueDetectionArtifact<IssueCase> {
    InterventionIssueDetectionArtifact {
        case_count: output.cases.len(),
        primary_issue: select_primary_issue(output),
    }
}

pub(crate) fn print_issue_case_block(label: &str, issue: &IssueCase) {
    println!("{}:", label);
    println!("  selection_basis: {}", serde_name(&issue.selection_basis));
    println!("  target_tool: {}", issue.target_tool.as_str());
    println!(
        "  target_file: {}",
        issue.target_tool.description_artifact_relpath()
    );
    println!(
        "  evidence: reviewed_calls={} reviewed_issue_calls={}",
        issue.evidence.reviewed_call_count, issue.evidence.reviewed_issue_call_count
    );
    let protocol = &issue.evidence.protocol;
    if !protocol.reviewed_call_indices.is_empty() {
        println!("  reviewed_calls: {:?}", protocol.reviewed_call_indices);
    }
    if !protocol.reviewed_segment_indices.is_empty() {
        println!(
            "  reviewed_segments: {:?}",
            protocol.reviewed_segment_indices
        );
    }
    if !protocol.nearby_segment_labels.is_empty() {
        println!(
            "  nearby_segment_labels: {:?}",
            protocol.nearby_segment_labels
        );
    }
    if !protocol.candidate_concerns.is_empty() {
        println!("  candidate_concerns:");
        for concern in &protocol.candidate_concerns {
            println!("    - {}", concern);
        }
    }
}

impl ProtocolToolCallSegmentReviewCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let sequence_subject = build_tool_call_sequence_subject(&record)?;
        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            120,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let adjudicator = JsonAdjudicator::new(client.clone(), cfg.clone());
        let segmentation = segment::ToolCallIntentSegmentation::new(adjudicator.clone())
            .run(sequence_subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;

        let subject = build_segment_review_subject(&segmentation.output, self.segment_index)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();
        let reviewed = run_tool_call_segment_review_with_json_retries(
            subject,
            &cfg,
            &client,
            self.segment_index,
        )
        .await?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &reviewed.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &reviewed.output,
            &reviewed.artifact,
        )?;
        let review = &reviewed.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", reviewed.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!(
                    "Target: {:?} {}",
                    review.packet.target_kind, review.packet.target_id
                );
                println!("Scope: {}", review.packet.scope_summary);
                println!("Turns: {}", join_turns(&review.packet.turn_span));
                println!("Calls in scope: {}", review.packet.total_calls_in_scope);
                println!("Calls:");
                for call in &review.packet.calls {
                    println!("  [{}] {} | {}", call.index, call.tool_name, call.summary);
                }
                println!();
                println!("Signals");
                println!("{}", "-".repeat(40));
                println!(
                    "Repeated tool calls: {}",
                    review.signals.repeated_tool_name_count
                );
                println!("Distinct tools: {}", review.signals.distinct_tool_count);
                println!("Directory pivots: {}", review.signals.directory_pivots);
                println!("Search calls: {}", review.signals.search_calls_in_scope);
                println!("Read calls: {}", review.signals.read_calls_in_scope);
                println!("Browse calls: {}", review.signals.browse_calls_in_scope);
                println!(
                    "Source labeled segments: {}",
                    review.signals.labeled_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source ambiguous segments: {}",
                    review.signals.ambiguous_segments_in_source.unwrap_or(0)
                );
                println!(
                    "Source uncovered calls: {}",
                    review.signals.uncovered_calls_in_source.unwrap_or(0)
                );
                println!(
                    "Candidate concerns: {:?}",
                    review.signals.candidate_concerns
                );
                println!();
                println!("Assessments");
                println!("{}", "-".repeat(40));
                println!(
                    "Usefulness: {:?} ({:?})",
                    review.usefulness.verdict, review.usefulness.confidence
                );
                println!("  {}", review.usefulness.rationale);
                println!(
                    "Redundancy: {:?} ({:?})",
                    review.redundancy.verdict, review.redundancy.confidence
                );
                println!("  {}", review.redundancy.rationale);
                println!(
                    "Recoverability: {:?} ({:?})",
                    review.recoverability.verdict, review.recoverability.confidence
                );
                println!("  {}", review.recoverability.rationale);
                println!();
                println!(
                    "Overall: {:?} ({:?})",
                    review.overall, review.overall_confidence
                );
                println!("Synthesis: {}", review.synthesis_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "procedure": reviewed.procedure_name,
                    "persisted_artifact_path": persisted_path,
                    "output": reviewed.output,
                    "artifact": reviewed.artifact,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

impl ProtocolToolCallIntentSegmentsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolution = resolve_record_path(self.record, self.instance, None)?;
        let record_path = resolution.record_path.clone();
        let record =
            read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
                path: record_path.clone(),
                source,
            })?;

        let subject = build_tool_call_sequence_subject(&record)?;
        let subject_id = subject.subject_id.clone();
        let persisted_input = subject.clone();

        let client = reqwest::Client::new();
        let cfg = protocol_llm_config(
            self.model_id,
            self.route_source,
            self.provider,
            120,
            PROTOCOL_HTTP_MAX_ATTEMPTS,
            default_protocol_max_tokens(),
            ProtocolReasoningPolicy::default(),
        )?;
        let protocol =
            segment::ToolCallIntentSegmentation::new(JsonAdjudicator::new(client, cfg.clone()));
        let segmented = protocol
            .run(subject)
            .await
            .map_err(tool_call_intent_segmentation_error_to_prepare)?;
        let persisted_path = write_protocol_artifact(
            &record_path,
            &segmented.procedure_name,
            &subject_id,
            Some(cfg.model_id.as_str()),
            cfg.provider_slug.as_deref(),
            &persisted_input,
            &segmented.output,
            &segmented.artifact,
        )?;
        let output = &segmented.output;

        match self.format {
            InspectOutputFormat::Table => {
                println!("Protocol: {}", segmented.procedure_name);
                println!("{}", "-".repeat(40));
                println!("Model: {}", cfg.model_id);
                println!("Provider: {}", cfg.provider_display());
                println!("Artifact: {}", persisted_path.display());
                println!("Turns: {}", output.sequence.total_turns);
                println!("Tool calls: {}", output.sequence.total_calls_in_run);
                println!("Segments: {}", output.segments.len());
                println!("Labeled segments: {}", output.coverage.labeled_segments);
                println!("Ambiguous segments: {}", output.coverage.ambiguous_segments);
                println!("Labeled calls: {}", output.coverage.labeled_calls);
                println!("Ambiguous calls: {}", output.coverage.ambiguous_calls);
                println!("Uncovered calls: {}", output.coverage.uncovered_calls);
                if !output.uncovered_call_indices.is_empty() {
                    println!(
                        "Uncovered indices: {}",
                        join_indices(&output.uncovered_call_indices)
                    );
                }
                if !output.uncovered_spans.is_empty() {
                    println!(
                        "Uncovered spans: {}",
                        output
                            .uncovered_spans
                            .iter()
                            .map(|span| format!("{}..={}", span.start_index, span.end_index))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                println!();
                println!("Sequence Signals");
                println!("{}", "-".repeat(40));
                println!("Search calls: {}", output.signals.search_calls);
                println!("Read calls: {}", output.signals.read_calls);
                println!("Browse calls: {}", output.signals.browse_calls);
                println!("Edit calls: {}", output.signals.edit_calls);
                println!("Failed calls: {}", output.signals.failed_calls);
                println!(
                    "Repeated search runs: {}",
                    output.signals.repeated_search_runs
                );
                println!("Directory pivots: {}", output.signals.directory_pivots);
                println!();
                println!("Intent Segments");
                println!("{}", "-".repeat(40));
                for segment in &output.segments {
                    println!(
                        "[{}] {} {}..={} confidence={:?}",
                        segment.segment_index,
                        format_segment_descriptor(segment.status, segment.label),
                        segment.start_index,
                        segment.end_index,
                        segment.confidence
                    );
                    println!("  turns ......... {}", join_turns(&segment.turns));
                    println!("  rationale ..... {}", segment.rationale);
                    for call in &segment.calls {
                        println!(
                            "  call .......... [{}] {} | {}",
                            call.index, call.tool_name, call.summary
                        );
                    }
                    println!();
                }
                if !output.uncovered_spans.is_empty() {
                    println!("Uncovered Regions");
                    println!("{}", "-".repeat(40));
                    for span in &output.uncovered_spans {
                        println!(
                            "{}..={} calls={}",
                            span.start_index,
                            span.end_index,
                            join_indices(&span.call_indices)
                        );
                        println!("  rationale ..... {}", span.rationale);
                    }
                    println!();
                }
                println!("Overall rationale");
                println!("{}", "-".repeat(40));
                println!("{}", output.overall_rationale);
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "persisted_artifact_path": persisted_path,
                    "run": segmented,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }

        print_record_resolution_footer(&resolution);
        Ok(())
    }
}

fn join_turns(turns: &[u32]) -> String {
    turns
        .iter()
        .map(|turn| turn.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_segment_descriptor(
    status: segment::SegmentStatus,
    label: Option<segment::IntentLabel>,
) -> String {
    match (status, label) {
        (segment::SegmentStatus::Labeled, Some(label)) => format!("labeled:{label:?}"),
        (segment::SegmentStatus::Labeled, None) => "labeled:<missing>".to_string(),
        (segment::SegmentStatus::Ambiguous, _) => "ambiguous".to_string(),
    }
}
