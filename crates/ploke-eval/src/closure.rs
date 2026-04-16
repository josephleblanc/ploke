use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::layout::{batches_dir, campaigns_dir, runs_dir};
use crate::protocol::protocol_aggregate::{ProtocolAggregateError, load_protocol_aggregate};
use crate::protocol_artifacts::{StoredProtocolArtifactFile, list_protocol_artifacts};
use crate::record::read_compressed_record;
use crate::runner::{
    BatchRunSummary, ExecutionLog, IndexingStatusArtifact, SnapshotStatusArtifact,
};
use crate::spec::PrepareError;
use crate::target_registry::{
    BenchmarkFamily, RegistryDatasetSource, RegistryEntry, RegistryEntryState,
    RegistryRecomputeRequest, TargetRegistry, load_target_registry, recompute_target_registry,
    target_registry_path,
};

pub const CLOSURE_STATE_SCHEMA_VERSION: &str = "closure-state.v1";

pub const DEFAULT_REQUIRED_PROCEDURES: [&str; 3] = [
    "tool-call-intent-segments",
    "tool-call-review",
    "tool-call-segment-review",
];

const STORED_TOOL_CALL_INTENT_SEGMENTATION: &str = "tool_call_intent_segmentation";
const STORED_TOOL_CALL_REVIEW: &str = "tool_call_review";
const STORED_TOOL_CALL_SEGMENT_REVIEW: &str = "tool_call_segment_review";

#[derive(Debug, Clone)]
pub struct ClosureRecomputeRequest {
    pub campaign_id: String,
    pub model_id: Option<String>,
    pub provider_slug: Option<String>,
    pub dataset_keys: Vec<String>,
    pub dataset_files: Vec<PathBuf>,
    pub required_procedures: Vec<String>,
    pub runs_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureState {
    pub schema_version: String,
    pub campaign_id: String,
    pub updated_at: String,
    pub config: ClosureConfig,
    pub registry: RegistryClosureSummary,
    pub eval: EvalClosureSummary,
    pub protocol: ProtocolClosureSummary,
    pub instances: Vec<ClosureInstanceRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureConfig {
    #[serde(default = "default_benchmark_family")]
    pub benchmark_family: BenchmarkFamily,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_path: Option<PathBuf>,
    pub dataset_sources: Vec<ClosureDatasetSource>,
    pub required_procedures: Vec<String>,
    pub runs_root: PathBuf,
}

pub type ClosureDatasetSource = RegistryDatasetSource;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ClosureClass {
    Complete,
    Failed,
    Missing,
    Ineligible,
    Incompatible,
    Partial,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RegistryInstanceStatus {
    Mapped,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryClosureSummary {
    pub expected_total: usize,
    pub mapped_total: usize,
    pub missing_total: usize,
    pub ambiguous_total: usize,
    pub status: ClosureClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalClosureSummary {
    pub expected_total: usize,
    pub complete_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub partial_total: usize,
    pub in_progress_total: usize,
    pub status: ClosureClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transition_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolClosureSummary {
    pub expected_total: usize,
    pub full_total: usize,
    pub partial_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub incompatible_total: usize,
    pub in_progress_total: usize,
    pub status: ClosureClass,
    pub required_procedures: Vec<String>,
    pub status_by_procedure: BTreeMap<String, ProcedureClosureSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_transition_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureClosureSummary {
    pub expected_total: usize,
    pub complete_total: usize,
    pub failed_total: usize,
    pub missing_total: usize,
    pub incompatible_total: usize,
    pub partial_total: usize,
    pub ineligible_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureInstanceRow {
    pub instance_id: String,
    pub dataset_label: String,
    pub repo_family: String,
    pub registry_status: RegistryInstanceStatus,
    pub eval_status: ClosureClass,
    pub protocol_status: ClosureClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_failure: Option<String>,
    pub artifacts: ClosureArtifactRefs,
    pub protocol_procedures: BTreeMap<String, ClosureClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_counts: Option<ClosureProtocolCounts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClosureArtifactRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_manifest: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_log: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexing_status: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_failure: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_status: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_anchor: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_failure_sources: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureProtocolCounts {
    pub total_calls: usize,
    pub reviewed_calls: usize,
    pub total_segments: usize,
    pub usable_segments: usize,
    pub mismatched_segments: usize,
    pub missing_segments: usize,
}

#[derive(Debug, Clone, Default)]
struct BatchFailureInfo {
    errors: Vec<String>,
    sources: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct ProtocolInstanceAssessment {
    overall: ClosureClass,
    procedures: BTreeMap<String, ClosureClass>,
    counts: Option<ClosureProtocolCounts>,
    anchor_path: Option<PathBuf>,
    failure: Option<String>,
    last_event_at: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SegmentEvidenceCounts {
    usable: usize,
    mismatched: usize,
    missing: usize,
}

pub fn closure_state_path(campaign_id: &str) -> Result<PathBuf, PrepareError> {
    Ok(campaigns_dir()?
        .join(campaign_id)
        .join("closure-state.json"))
}

pub fn load_closure_state(campaign_id: &str) -> Result<ClosureState, PrepareError> {
    let path = closure_state_path(campaign_id)?;
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

pub fn recompute_closure_state(
    request: ClosureRecomputeRequest,
) -> Result<(PathBuf, ClosureState), PrepareError> {
    let campaign_id = request.campaign_id.clone();
    let existing = load_closure_state(&request.campaign_id).ok();
    let (config, registry_source) = resolve_config(request, existing)?;
    let batch_failures = collect_batch_failures()?;

    let mut instances = registry_source
        .entries
        .iter()
        .into_iter()
        .map(|entry| build_instance_row(&config, entry, &batch_failures))
        .collect::<Result<Vec<_>, _>>()?;
    instances.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));

    let registry = summarize_registry(&instances);
    let eval = summarize_eval(&instances);
    let protocol = summarize_protocol(&instances, &config.required_procedures);

    let state = ClosureState {
        schema_version: CLOSURE_STATE_SCHEMA_VERSION.to_string(),
        campaign_id: config_campaign_id(&config, &campaign_id),
        updated_at: Utc::now().to_rfc3339(),
        config,
        registry,
        eval,
        protocol,
        instances,
    };

    let path = closure_state_path(&state.campaign_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(&state).map_err(PrepareError::Serialize)?;
    fs::write(&path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.clone(),
        source,
    })?;
    Ok((path, state))
}

pub fn render_closure_status(state: &ClosureState) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "campaign {} | updated {} | model {} | provider {}\n",
        state.campaign_id,
        state.updated_at,
        state.config.model_id.as_deref().unwrap_or("unspecified"),
        state
            .config
            .provider_slug
            .as_deref()
            .unwrap_or("unspecified")
    ));
    out.push_str(&format!(
        "registry [{}] | mapped {}/{} | missing {} | ambiguous {}\n",
        format_closure_class(state.registry.status),
        state.registry.mapped_total,
        state.registry.expected_total,
        state.registry.missing_total,
        state.registry.ambiguous_total
    ));
    out.push_str(&format!(
        "eval [{}] | progress {}/{} | success {} | fail {} | partial {} | missing {}\n",
        format_closure_class(state.eval.status),
        state.eval.complete_total + state.eval.failed_total + state.eval.partial_total,
        state.eval.expected_total,
        state.eval.complete_total,
        state.eval.failed_total,
        state.eval.partial_total,
        state.eval.missing_total
    ));
    out.push_str(&format!(
        "protocol [{}] | progress {}/{} | full {} | partial {} | incompatible {} | fail {} | missing {}\n",
        format_closure_class(state.protocol.status),
        state.protocol.full_total
            + state.protocol.partial_total
            + state.protocol.incompatible_total
            + state.protocol.failed_total,
        state.protocol.expected_total,
        state.protocol.full_total,
        state.protocol.partial_total,
        state.protocol.incompatible_total,
        state.protocol.failed_total,
        state.protocol.missing_total
    ));

    let eval_failures = state
        .instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Failed)
        .take(5)
        .collect::<Vec<_>>();
    if !eval_failures.is_empty() {
        out.push_str("\nEval failures\n");
        for row in eval_failures {
            out.push_str(&format!(
                "  - {}: {}\n",
                row.instance_id,
                row.eval_failure.as_deref().unwrap_or("failed")
            ));
        }
    }

    let protocol_frontier = state
        .instances
        .iter()
        .filter(|row| {
            matches!(
                row.protocol_status,
                ClosureClass::Partial | ClosureClass::Missing | ClosureClass::Incompatible
            )
        })
        .filter(|row| row.eval_status == ClosureClass::Complete)
        .take(8)
        .collect::<Vec<_>>();
    if !protocol_frontier.is_empty() {
        out.push_str("\nProtocol frontier\n");
        for row in protocol_frontier {
            out.push_str(&format!(
                "  - {} [{}]",
                row.instance_id,
                format_closure_class(row.protocol_status)
            ));
            if let Some(counts) = &row.protocol_counts {
                out.push_str(&format!(
                    " calls {}/{} | segments u{} m{} x{}",
                    counts.reviewed_calls,
                    counts.total_calls,
                    counts.usable_segments,
                    counts.mismatched_segments,
                    counts.missing_segments
                ));
            }
            out.push('\n');
        }
    }

    out
}

fn default_benchmark_family() -> BenchmarkFamily {
    BenchmarkFamily::MultiSweBenchRust
}

fn resolve_config(
    request: ClosureRecomputeRequest,
    existing: Option<ClosureState>,
) -> Result<(ClosureConfig, TargetRegistry), PrepareError> {
    let prior = existing.map(|state| state.config);

    let required_procedures = if request.required_procedures.is_empty() {
        prior
            .as_ref()
            .map(|config| config.required_procedures.clone())
            .unwrap_or_else(|| {
                DEFAULT_REQUIRED_PROCEDURES
                    .iter()
                    .map(|value| value.to_string())
                    .collect()
            })
    } else {
        request
            .required_procedures
            .into_iter()
            .map(normalize_cli_procedure_name)
            .collect()
    };

    let runs_root = request
        .runs_root
        .or_else(|| prior.as_ref().map(|config| config.runs_root.clone()))
        .unwrap_or(runs_dir()?);

    let benchmark_family = prior
        .as_ref()
        .map(|config| config.benchmark_family)
        .unwrap_or_else(default_benchmark_family);

    let (registry_path, registry_source) =
        if request.dataset_keys.is_empty() && request.dataset_files.is_empty() {
            let registry = load_target_registry(benchmark_family)?;
            let path = prior
                .as_ref()
                .and_then(|config| config.registry_path.clone())
                .unwrap_or(target_registry_path(benchmark_family)?);
            (path, registry)
        } else {
            recompute_target_registry(RegistryRecomputeRequest {
                benchmark_family,
                dataset_keys: request.dataset_keys,
                dataset_files: request.dataset_files,
            })?
        };

    Ok((
        ClosureConfig {
            benchmark_family,
            model_id: request
                .model_id
                .or_else(|| prior.as_ref().and_then(|config| config.model_id.clone())),
            provider_slug: request.provider_slug.or_else(|| {
                prior
                    .as_ref()
                    .and_then(|config| config.provider_slug.clone())
            }),
            registry_path: Some(registry_path),
            dataset_sources: registry_source.dataset_sources.clone(),
            required_procedures,
            runs_root,
        },
        registry_source,
    ))
}

fn config_campaign_id(_config: &ClosureConfig, requested: &str) -> String {
    requested.to_string()
}

fn collect_batch_failures() -> Result<HashMap<String, BatchFailureInfo>, PrepareError> {
    let root = batches_dir()?;
    let mut failures = HashMap::new();
    if !root.exists() {
        return Ok(failures);
    }
    for entry in fs::read_dir(&root).map_err(|source| PrepareError::ReadBatchManifest {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadBatchManifest {
            path: root.clone(),
            source,
        })?;
        let summary_path = entry.path().join("batch-run-summary.json");
        if !summary_path.exists() {
            continue;
        }
        let text = fs::read_to_string(&summary_path).map_err(|source| {
            PrepareError::ReadBatchManifest {
                path: summary_path.clone(),
                source,
            }
        })?;
        let summary: BatchRunSummary =
            serde_json::from_str(&text).map_err(|source| PrepareError::ParseBatchManifest {
                path: summary_path.clone(),
                source,
            })?;
        for result in summary.instance_results {
            if result.status != "failed" {
                continue;
            }
            let info = failures
                .entry(result.task_id)
                .or_insert_with(BatchFailureInfo::default);
            if let Some(error) = result.error {
                info.errors.push(error);
            }
            info.sources.push(summary_path.clone());
        }
    }
    Ok(failures)
}

fn build_instance_row(
    config: &ClosureConfig,
    entry: &RegistryEntry,
    batch_failures: &HashMap<String, BatchFailureInfo>,
) -> Result<ClosureInstanceRow, PrepareError> {
    let run_dir = config.runs_root.join(&entry.instance_id);
    let run_manifest = run_dir.join("run.json");
    let record_path = run_dir.join("record.json.gz");
    let execution_log_path = run_dir.join("execution-log.json");
    let indexing_status_path = run_dir.join("indexing-status.json");
    let parse_failure_path = run_dir.join("parse-failure.json");
    let snapshot_status_path = run_dir.join("snapshot-status.json");

    let mut artifacts = ClosureArtifactRefs::default();
    if run_manifest.exists() {
        artifacts.run_manifest = Some(run_manifest.clone());
    }
    if record_path.exists() {
        artifacts.record_path = Some(record_path.clone());
    }
    if execution_log_path.exists() {
        artifacts.execution_log = Some(execution_log_path.clone());
    }
    if indexing_status_path.exists() {
        artifacts.indexing_status = Some(indexing_status_path.clone());
    }
    if parse_failure_path.exists() {
        artifacts.parse_failure = Some(parse_failure_path.clone());
    }
    if snapshot_status_path.exists() {
        artifacts.snapshot_status = Some(snapshot_status_path.clone());
    }

    let registry_status = RegistryInstanceStatus::Mapped;
    let (eval_status, eval_failure, eval_last_event, protocol) = match &entry.state {
        RegistryEntryState::Active => {
            let (eval_status, eval_failure, eval_last_event) = classify_eval_status(
                &run_dir,
                &record_path,
                &indexing_status_path,
                &parse_failure_path,
                &execution_log_path,
                &snapshot_status_path,
                batch_failures.get(&entry.instance_id),
                &mut artifacts,
            )?;

            let protocol = if eval_status == ClosureClass::Complete {
                assess_protocol_state(&record_path, &config.required_procedures)?
            } else {
                let procedures = config
                    .required_procedures
                    .iter()
                    .map(|name| (name.clone(), ClosureClass::Ineligible))
                    .collect::<BTreeMap<_, _>>();
                ProtocolInstanceAssessment {
                    overall: ClosureClass::Ineligible,
                    procedures,
                    counts: None,
                    anchor_path: None,
                    failure: None,
                    last_event_at: None,
                }
            };

            (eval_status, eval_failure, eval_last_event, protocol)
        }
        RegistryEntryState::Ineligible { reason } => {
            let procedures = config
                .required_procedures
                .iter()
                .map(|name| (name.clone(), ClosureClass::Ineligible))
                .collect::<BTreeMap<_, _>>();
            (
                ClosureClass::Ineligible,
                Some(reason.clone()),
                None,
                ProtocolInstanceAssessment {
                    overall: ClosureClass::Ineligible,
                    procedures,
                    counts: None,
                    anchor_path: None,
                    failure: None,
                    last_event_at: None,
                },
            )
        }
    };

    artifacts.protocol_anchor = protocol.anchor_path.clone();
    let last_event_at = latest_timestamp(eval_last_event, protocol.last_event_at);

    Ok(ClosureInstanceRow {
        instance_id: entry.instance_id.clone(),
        dataset_label: entry.dataset_label.clone(),
        repo_family: entry.repo_family.clone(),
        registry_status,
        eval_status,
        protocol_status: protocol.overall,
        eval_failure,
        protocol_failure: protocol.failure,
        artifacts,
        protocol_procedures: protocol.procedures,
        protocol_counts: protocol.counts,
        last_event_at,
    })
}

fn classify_eval_status(
    run_dir: &Path,
    record_path: &Path,
    indexing_status_path: &Path,
    parse_failure_path: &Path,
    execution_log_path: &Path,
    snapshot_status_path: &Path,
    batch_failure: Option<&BatchFailureInfo>,
    artifacts: &mut ClosureArtifactRefs,
) -> Result<(ClosureClass, Option<String>, Option<String>), PrepareError> {
    if record_path.exists() {
        return Ok((
            ClosureClass::Complete,
            None,
            file_timestamp_string(record_path)?,
        ));
    }

    if let Some(batch_failure) = batch_failure {
        artifacts.batch_failure_sources = batch_failure.sources.clone();
        return Ok((
            ClosureClass::Failed,
            Some(batch_failure.errors.join(" | ")),
            newest_timestamp_from_paths(&batch_failure.sources)?,
        ));
    }

    if parse_failure_path.exists() {
        let message = fs::read_to_string(parse_failure_path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "parse failure persisted".to_string());
        return Ok((
            ClosureClass::Failed,
            Some(message),
            file_timestamp_string(parse_failure_path)?,
        ));
    }

    if indexing_status_path.exists() {
        let text = fs::read_to_string(indexing_status_path).map_err(|source| {
            PrepareError::ReadManifest {
                path: indexing_status_path.to_path_buf(),
                source,
            }
        })?;
        let status: IndexingStatusArtifact =
            serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
                path: indexing_status_path.to_path_buf(),
                source,
            })?;
        let timestamp = file_timestamp_string(indexing_status_path)?;
        if matches!(
            status.status.as_str(),
            "failed" | "timed_out" | "event_stream_closed"
        ) {
            return Ok((ClosureClass::Failed, Some(status.detail), timestamp));
        }
        return Ok((ClosureClass::Partial, Some(status.detail), timestamp));
    }

    if execution_log_path.exists()
        || snapshot_status_path.exists()
        || run_dir.join("repo-state.json").exists()
    {
        if snapshot_status_path.exists() {
            let text = fs::read_to_string(snapshot_status_path).map_err(|source| {
                PrepareError::ReadManifest {
                    path: snapshot_status_path.to_path_buf(),
                    source,
                }
            })?;
            let _snapshot_status: SnapshotStatusArtifact =
                serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
                    path: snapshot_status_path.to_path_buf(),
                    source,
                })?;
        }
        if execution_log_path.exists() {
            let text = fs::read_to_string(execution_log_path).map_err(|source| {
                PrepareError::ReadManifest {
                    path: execution_log_path.to_path_buf(),
                    source,
                }
            })?;
            let _execution_log: ExecutionLog =
                serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
                    path: execution_log_path.to_path_buf(),
                    source,
                })?;
        }
        let timestamp = newest_timestamp_from_paths(&[
            execution_log_path.to_path_buf(),
            snapshot_status_path.to_path_buf(),
            run_dir.join("repo-state.json"),
        ])?;
        return Ok((
            ClosureClass::Partial,
            Some("run artifacts exist without final record".to_string()),
            timestamp,
        ));
    }

    Ok((ClosureClass::Missing, None, None))
}

fn assess_protocol_state(
    record_path: &Path,
    required_procedures: &[String],
) -> Result<ProtocolInstanceAssessment, PrepareError> {
    let mut procedures = required_procedures
        .iter()
        .map(|name| (name.clone(), ClosureClass::Missing))
        .collect::<BTreeMap<_, _>>();
    let artifacts = list_protocol_artifacts(record_path)?;
    if artifacts.is_empty() {
        return Ok(ProtocolInstanceAssessment {
            overall: ClosureClass::Missing,
            procedures,
            counts: None,
            anchor_path: None,
            failure: None,
            last_event_at: None,
        });
    }

    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let total_calls = record.tool_calls().len();
    let latest_anchor = latest_artifact_path(&artifacts, STORED_TOOL_CALL_INTENT_SEGMENTATION);
    let call_reviews = distinct_count_by(&artifacts, STORED_TOOL_CALL_REVIEW, call_review_index);
    let segment_review_count = distinct_count_by(
        &artifacts,
        STORED_TOOL_CALL_SEGMENT_REVIEW,
        segment_review_index,
    );

    if let Some(name) = procedures.get_mut("tool-call-intent-segments") {
        *name = if latest_anchor.is_some() {
            ClosureClass::Complete
        } else {
            ClosureClass::Missing
        };
    }
    if let Some(name) = procedures.get_mut("tool-call-review") {
        *name = classify_fraction(call_reviews, total_calls);
    }

    let mut counts = None;
    let mut overall;
    let mut failure = None;
    let mut last_event_at = newest_protocol_timestamp(&artifacts);

    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => {
            let segment_counts = segment_evidence_counts_from_aggregate(&aggregate);
            let usable_segments = segment_counts.usable;
            let mismatched_segments = segment_counts.mismatched;
            let missing_segments = segment_counts.missing;
            let total_segments = aggregate.segmentation.segments.len();
            let segment_state = classify_segment_review_state(
                usable_segments,
                mismatched_segments,
                missing_segments,
            );
            if let Some(name) = procedures.get_mut("tool-call-segment-review") {
                *name = segment_state;
            }
            counts = Some(ClosureProtocolCounts {
                total_calls,
                reviewed_calls: aggregate.coverage.reviewed_call_count,
                total_segments,
                usable_segments,
                mismatched_segments,
                missing_segments,
            });
            overall = summarize_overall_protocol(&procedures);
            last_event_at = latest_timestamp(last_event_at, file_timestamp_string(record_path)?);
        }
        Err(ProtocolAggregateError::MissingAnchor { .. }) => {
            if let Some(name) = procedures.get_mut("tool-call-segment-review") {
                *name = if segment_review_count > 0 {
                    ClosureClass::Incompatible
                } else {
                    ClosureClass::Missing
                };
            }
            overall = summarize_overall_protocol(&procedures);
        }
        Err(err) => {
            if let Some(name) = procedures.get_mut("tool-call-segment-review") {
                *name = ClosureClass::Failed;
            }
            failure = Some(err.to_string());
            overall = ClosureClass::Failed;
        }
    }

    if overall == ClosureClass::Missing {
        overall = summarize_overall_protocol(&procedures);
    }

    Ok(ProtocolInstanceAssessment {
        overall,
        procedures,
        counts,
        anchor_path: latest_anchor,
        failure,
        last_event_at,
    })
}

fn segment_evidence_counts_from_aggregate(
    aggregate: &crate::protocol::protocol_aggregate::ProtocolAggregate,
) -> SegmentEvidenceCounts {
    let accepted_segment_indices = aggregate
        .segment_reviews
        .iter()
        .map(|row| row.basis.segment_index)
        .collect::<BTreeSet<_>>();
    let mismatched_segment_indices = aggregate
        .skipped_segment_reviews
        .iter()
        .map(|row| row.segment_index)
        .collect::<BTreeSet<_>>();

    let mut usable = 0usize;
    let mut mismatched = 0usize;
    let mut missing = 0usize;

    for basis in &aggregate.segmentation.segments {
        if accepted_segment_indices.contains(&basis.segment_index) {
            usable += 1;
        } else if mismatched_segment_indices.contains(&basis.segment_index) {
            mismatched += 1;
        } else {
            missing += 1;
        }
    }

    SegmentEvidenceCounts {
        usable,
        mismatched,
        missing,
    }
}

fn summarize_registry(instances: &[ClosureInstanceRow]) -> RegistryClosureSummary {
    let expected_total = instances.len();
    let mapped_total = instances
        .iter()
        .filter(|row| row.registry_status == RegistryInstanceStatus::Mapped)
        .count();
    let ambiguous_total = instances
        .iter()
        .filter(|row| row.registry_status == RegistryInstanceStatus::Ambiguous)
        .count();
    let missing_total = expected_total.saturating_sub(mapped_total + ambiguous_total);
    let status = if expected_total == 0 {
        ClosureClass::Missing
    } else if missing_total == 0 && ambiguous_total == 0 {
        ClosureClass::Complete
    } else if mapped_total == 0 {
        ClosureClass::Missing
    } else {
        ClosureClass::Partial
    };

    RegistryClosureSummary {
        expected_total,
        mapped_total,
        missing_total,
        ambiguous_total,
        status,
    }
}

fn summarize_eval(instances: &[ClosureInstanceRow]) -> EvalClosureSummary {
    let relevant = instances
        .iter()
        .filter(|row| row.eval_status != ClosureClass::Ineligible)
        .collect::<Vec<_>>();
    let expected_total = relevant.len();
    let complete_total = relevant
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
        .count();
    let failed_total = relevant
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Failed)
        .count();
    let partial_total = relevant
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Partial)
        .count();
    let missing_total = relevant
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Missing)
        .count();
    let status = if expected_total == 0 {
        ClosureClass::Missing
    } else if complete_total == expected_total {
        ClosureClass::Complete
    } else if complete_total == 0 && failed_total == 0 && partial_total == 0 {
        ClosureClass::Missing
    } else {
        ClosureClass::Partial
    };

    let last_transition_at = relevant
        .iter()
        .filter_map(|row| row.last_event_at.clone())
        .max();

    EvalClosureSummary {
        expected_total,
        complete_total,
        failed_total,
        missing_total,
        partial_total,
        in_progress_total: 0,
        status,
        last_transition_at,
    }
}

fn summarize_protocol(
    instances: &[ClosureInstanceRow],
    required_procedures: &[String],
) -> ProtocolClosureSummary {
    let relevant = instances
        .iter()
        .filter(|row| row.eval_status == ClosureClass::Complete)
        .collect::<Vec<_>>();
    let expected_total = relevant.len();
    let full_total = relevant
        .iter()
        .filter(|row| row.protocol_status == ClosureClass::Complete)
        .count();
    let partial_total = relevant
        .iter()
        .filter(|row| row.protocol_status == ClosureClass::Partial)
        .count();
    let failed_total = relevant
        .iter()
        .filter(|row| row.protocol_status == ClosureClass::Failed)
        .count();
    let missing_total = relevant
        .iter()
        .filter(|row| row.protocol_status == ClosureClass::Missing)
        .count();
    let incompatible_total = relevant
        .iter()
        .filter(|row| row.protocol_status == ClosureClass::Incompatible)
        .count();
    let status = if expected_total == 0 {
        ClosureClass::Missing
    } else if full_total == expected_total {
        ClosureClass::Complete
    } else if full_total == 0 && partial_total == 0 && incompatible_total == 0 && failed_total == 0
    {
        ClosureClass::Missing
    } else {
        ClosureClass::Partial
    };

    let mut status_by_procedure = BTreeMap::new();
    for procedure in required_procedures {
        let mut summary = ProcedureClosureSummary {
            expected_total,
            complete_total: 0,
            failed_total: 0,
            missing_total: 0,
            incompatible_total: 0,
            partial_total: 0,
            ineligible_total: 0,
        };
        for row in &relevant {
            match row
                .protocol_procedures
                .get(procedure)
                .copied()
                .unwrap_or(ClosureClass::Missing)
            {
                ClosureClass::Complete => summary.complete_total += 1,
                ClosureClass::Failed => summary.failed_total += 1,
                ClosureClass::Missing => summary.missing_total += 1,
                ClosureClass::Incompatible => summary.incompatible_total += 1,
                ClosureClass::Partial => summary.partial_total += 1,
                ClosureClass::Ineligible => summary.ineligible_total += 1,
            }
        }
        status_by_procedure.insert(procedure.clone(), summary);
    }

    let last_transition_at = relevant
        .iter()
        .filter_map(|row| row.last_event_at.clone())
        .max();

    ProtocolClosureSummary {
        expected_total,
        full_total,
        partial_total,
        failed_total,
        missing_total,
        incompatible_total,
        in_progress_total: 0,
        status,
        required_procedures: required_procedures.to_vec(),
        status_by_procedure,
        last_transition_at,
    }
}

fn classify_fraction(done: usize, total: usize) -> ClosureClass {
    if total == 0 || done == 0 {
        ClosureClass::Missing
    } else if done >= total {
        ClosureClass::Complete
    } else {
        ClosureClass::Partial
    }
}

fn classify_segment_review_state(usable: usize, mismatched: usize, missing: usize) -> ClosureClass {
    if usable == 0 && mismatched == 0 && missing == 0 {
        ClosureClass::Missing
    } else if usable > 0 && mismatched == 0 && missing == 0 {
        ClosureClass::Complete
    } else if usable == 0 && mismatched > 0 && missing == 0 {
        ClosureClass::Incompatible
    } else if usable == 0 && mismatched == 0 && missing > 0 {
        ClosureClass::Missing
    } else {
        ClosureClass::Partial
    }
}

fn summarize_overall_protocol(procedures: &BTreeMap<String, ClosureClass>) -> ClosureClass {
    let mut any_complete = false;
    let mut any_partial = false;
    let mut any_incompatible = false;
    let mut any_failed = false;
    let mut all_missing = true;

    for status in procedures.values().copied() {
        match status {
            ClosureClass::Complete => {
                any_complete = true;
                all_missing = false;
            }
            ClosureClass::Partial => {
                any_partial = true;
                all_missing = false;
            }
            ClosureClass::Incompatible => {
                any_incompatible = true;
                all_missing = false;
            }
            ClosureClass::Failed => {
                any_failed = true;
                all_missing = false;
            }
            ClosureClass::Missing => {}
            ClosureClass::Ineligible => {
                all_missing = false;
            }
        }
    }

    if any_failed {
        ClosureClass::Failed
    } else if procedures
        .values()
        .all(|status| *status == ClosureClass::Complete)
    {
        ClosureClass::Complete
    } else if !any_complete
        && !any_partial
        && any_incompatible
        && procedures
            .values()
            .all(|status| matches!(status, ClosureClass::Incompatible | ClosureClass::Missing))
    {
        ClosureClass::Incompatible
    } else if all_missing {
        ClosureClass::Missing
    } else {
        ClosureClass::Partial
    }
}

fn distinct_count_by(
    artifacts: &[StoredProtocolArtifactFile],
    stored_name: &str,
    key_fn: fn(&StoredProtocolArtifactFile) -> Option<usize>,
) -> usize {
    artifacts
        .iter()
        .filter(|entry| entry.stored.procedure_name == stored_name)
        .filter_map(key_fn)
        .collect::<BTreeSet<_>>()
        .len()
}

fn call_review_index(entry: &StoredProtocolArtifactFile) -> Option<usize> {
    entry
        .stored
        .output
        .get("packet")
        .and_then(|value| value.get("focal_call_index"))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .or_else(|| {
            entry
                .stored
                .output
                .get("neighborhood")
                .and_then(|value| value.get("focal"))
                .and_then(|value| value.get("index"))
                .and_then(|value| value.as_u64())
                .map(|value| value as usize)
        })
}

fn segment_review_index(entry: &StoredProtocolArtifactFile) -> Option<usize> {
    entry
        .stored
        .output
        .get("packet")
        .and_then(|value| value.get("segment_index"))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .or_else(|| {
            entry
                .stored
                .output
                .get("packet")
                .and_then(|value| value.get("target_id"))
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse::<usize>().ok())
        })
}

fn latest_artifact_path(
    artifacts: &[StoredProtocolArtifactFile],
    stored_name: &str,
) -> Option<PathBuf> {
    artifacts
        .iter()
        .filter(|entry| entry.stored.procedure_name == stored_name)
        .max_by_key(|entry| entry.stored.created_at_ms)
        .map(|entry| entry.path.clone())
}

fn newest_protocol_timestamp(artifacts: &[StoredProtocolArtifactFile]) -> Option<String> {
    artifacts
        .iter()
        .max_by_key(|entry| entry.stored.created_at_ms)
        .map(|entry| {
            chrono::DateTime::<Utc>::from_timestamp_millis(entry.stored.created_at_ms as i64)
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339())
        })
}

fn normalize_cli_procedure_name(name: String) -> String {
    match name.as_str() {
        "tool_call_intent_segmentation" | "tool-call-intent-segmentation" => {
            "tool-call-intent-segments".to_string()
        }
        "tool_call_review" => "tool-call-review".to_string(),
        "tool_call_segment_review" => "tool-call-segment-review".to_string(),
        _ => name.replace('_', "-"),
    }
}

fn file_timestamp_string(path: &Path) -> Result<Option<String>, PrepareError> {
    if !path.exists() {
        return Ok(None);
    }
    let modified = fs::metadata(path)
        .map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?
        .modified()
        .map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source: std::io::Error::other(source),
        })?;
    Ok(system_time_to_rfc3339(modified))
}

fn newest_timestamp_from_paths(paths: &[PathBuf]) -> Result<Option<String>, PrepareError> {
    let mut newest: Option<SystemTime> = None;
    for path in paths {
        if !path.exists() {
            continue;
        }
        let modified = fs::metadata(path)
            .map_err(|source| PrepareError::ReadManifest {
                path: path.clone(),
                source,
            })?
            .modified()
            .map_err(|source| PrepareError::ReadManifest {
                path: path.clone(),
                source: std::io::Error::other(source),
            })?;
        newest = Some(match newest {
            Some(current) => current.max(modified),
            None => modified,
        });
    }
    Ok(newest.and_then(system_time_to_rfc3339))
}

fn system_time_to_rfc3339(time: SystemTime) -> Option<String> {
    let datetime = chrono::DateTime::<Utc>::from(time);
    Some(datetime.to_rfc3339())
}

fn latest_timestamp(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn format_closure_class(value: ClosureClass) -> &'static str {
    match value {
        ClosureClass::Complete => "complete",
        ClosureClass::Failed => "failed",
        ClosureClass::Missing => "missing",
        ClosureClass::Ineligible => "ineligible",
        ClosureClass::Incompatible => "incompatible",
        ClosureClass::Partial => "partial",
    }
}
