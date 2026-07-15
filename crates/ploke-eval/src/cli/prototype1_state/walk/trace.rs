//! Filesystem-authoritative inspection of one completed evaluation run.
//!
//! This read model follows the canonical run registry and reuses passive
//! record carriers. It deliberately does not read the mutable in-flight turn
//! trace, replay-admit provider responses, or claim database authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

use flate2::read::GzDecoder;
use ploke_protocol::{LocalAnalysisAssessment, LocalAnalysisTargetKind, ToolCallNeighborhood};
use ploke_records::{
    agent_turn::{AgentTurnSummaryRecord, ObservedTurnEventRecord},
    ids::{CampaignId, InstanceId},
    llm_response::{RawFullResponseRecord, decode_full_response_lines},
    protocol::{ArtifactBody, ArtifactFile, SCHEMA_V1 as PROTOCOL_SCHEMA_V1},
    run_record::{RUN_RECORD_SCHEMA_VERSION, RunRecord},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    inner::{RunRegistration, core::RegisteredRunRole, registry::RunPhaseStatus},
    layout,
    protocol_artifacts::{
        ProtocolArtifactLoadFailure, ProtocolArtifactLoadResult,
        list_protocol_artifact_load_results,
    },
    run_registry::{
        RunExecutionStatus, completed_record_paths_for_instances_root,
        load_registration_for_record_path,
    },
    spec::PrepareError,
};

use super::{config, epoch::ServerEpoch, protocol::SessionVersion};

const ERROR_PHASE: &str = "prototype1_state_walk_evaluation_trace";

/// Stable public coordinate for one evaluation attempt inside a campaign view.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRunCoordinate {
    pub campaign: CampaignId,
    pub instance: InstanceId,
    pub run_id: String,
}

/// One canonical evidence file and the digest of its exact stored bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceSource {
    pub kind: TraceSourceKind,
    pub path: PathBuf,
    pub content_sha256: String,
}

/// One typed trace value bound to the integrity record for its exact source
/// bytes. The source remains adjacent to the value across the public wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEvidence<T> {
    pub value: T,
    pub source: TraceSource,
}

/// Semantic role of a file included in an evaluation trace observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceSourceKind {
    Registration,
    TurnSummary,
    RunRecord,
    ModelResponses,
    ProtocolArtifact,
}

/// One provider response in physical JSONL order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelExchange {
    pub source_line: usize,
    pub record: RawFullResponseRecord,
}

/// Discovery entry backed by one completed global run registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationRunEntry {
    pub coordinate: EvaluationRunCoordinate,
    pub registration: TraceEvidence<RunRegistration>,
}

/// Completed-run inventory scoped to the campaign admitted by this checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationTraceIndex {
    pub version: SessionVersion,
    pub epoch: ServerEpoch,
    pub campaign: CampaignId,
    pub instances_root: PathBuf,
    pub authority: TraceAuthority,
    pub runs: Vec<EvaluationRunEntry>,
}

/// Authority surface used to discover and load this trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceAuthority {
    RunRegistry,
}

/// Canonical completed evaluation evidence. Existing record types retain their
/// persisted meaning; this carrier only binds them to source integrity data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedEvaluationTrace {
    pub registration: TraceEvidence<RunRegistration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<TraceEvidence<AgentTurnSummaryRecord>>,
    pub run: TraceEvidence<RunRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchanges: Option<TraceEvidence<Vec<ModelExchange>>>,
    pub protocol: Vec<TraceEvidence<ArtifactFile>>,
}

/// Lifecycle-sensitive result for an exact run. Mutable trace files are never
/// opened until the authoritative registration says execution is completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvaluationTraceState {
    NotCompleted {
        registration: TraceEvidence<RunRegistration>,
    },
    Completed {
        trace: CompletedEvaluationTrace,
    },
}

/// Revision- and epoch-tagged exact-run observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationTraceSnapshot {
    pub coordinate: EvaluationRunCoordinate,
    pub version: SessionVersion,
    pub epoch: ServerEpoch,
    pub authority: TraceAuthority,
    pub trace: EvaluationTraceState,
}

impl CompletedEvaluationTrace {
    /// Visit every exact source included in this observation.
    pub fn sources(&self) -> impl Iterator<Item = &TraceSource> {
        std::iter::once(&self.registration.source)
            .chain(self.turn.iter().map(|turn| &turn.source))
            .chain(std::iter::once(&self.run.source))
            .chain(self.exchanges.iter().map(|exchanges| &exchanges.source))
            .chain(self.protocol.iter().map(|artifact| &artifact.source))
    }
}

#[derive(Debug, Clone)]
struct TraceScope {
    campaign: CampaignId,
    instances_root: PathBuf,
    registry_root: PathBuf,
}

pub(crate) fn load_index(
    repo_root: &Path,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<EvaluationTraceIndex, PrepareError> {
    let scope = TraceScope::load(repo_root)?;
    load_index_for(scope, version, epoch)
}

pub(crate) fn load_run(
    repo_root: &Path,
    coordinate: EvaluationRunCoordinate,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<EvaluationTraceSnapshot, PrepareError> {
    let scope = TraceScope::load(repo_root)?;
    load_run_for(scope, coordinate, version, epoch)
}

impl TraceScope {
    fn load(repo_root: &Path) -> Result<Self, PrepareError> {
        let snapshot = config::load(repo_root)?;
        Ok(Self {
            campaign: snapshot.identity.record.campaign_id,
            instances_root: snapshot.campaign.resolved.instances_root,
            registry_root: layout::registries_dir()?,
        })
    }
}

fn load_index_for(
    scope: TraceScope,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<EvaluationTraceIndex, PrepareError> {
    let mut runs = Vec::new();
    let mut seen = BTreeSet::new();
    for record_path in completed_record_paths_for_instances_root(&scope.instances_root)? {
        let registration = load_registration_for_record_path(&record_path)?.ok_or_else(|| {
            trace_error(format!(
                "completed record '{}' has no authoritative run registration",
                record_path.display()
            ))
        })?;
        let coordinate = coordinate_for(&registration)?;
        if !seen.insert(coordinate.clone()) {
            return Err(trace_error(format!(
                "duplicate completed run coordinate '{}:{}'",
                coordinate.instance, coordinate.run_id
            )));
        }
        validate_registration(&scope, &coordinate, &registration)?;
        if registration.lifecycle.execution_status != RunExecutionStatus::Completed {
            return Err(trace_error(format!(
                "completed-run index selected '{}' with lifecycle {:?}",
                coordinate.run_id, registration.lifecycle.execution_status
            )));
        }
        if record_path != registration.artifacts.record_path {
            return Err(trace_error(format!(
                "completed-run index path '{}' disagrees with registration record path '{}'",
                record_path.display(),
                registration.artifacts.record_path.display()
            )));
        }
        let source = load_registration_source(&registration)?;
        runs.push(EvaluationRunEntry {
            coordinate,
            registration: evidence(registration, source),
        });
    }
    runs.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));

    Ok(EvaluationTraceIndex {
        version,
        epoch,
        campaign: scope.campaign,
        instances_root: scope.instances_root,
        authority: TraceAuthority::RunRegistry,
        runs,
    })
}

fn load_run_for(
    scope: TraceScope,
    coordinate: EvaluationRunCoordinate,
    version: SessionVersion,
    epoch: ServerEpoch,
) -> Result<EvaluationTraceSnapshot, PrepareError> {
    validate_coordinate(&coordinate)?;
    let path = scope
        .registry_root
        .join("runs")
        .join(format!("{}.json", coordinate.run_id));
    let (registration, source) = load_registration(&path)?;
    validate_registration(&scope, &coordinate, &registration)?;

    let trace = if registration.lifecycle.execution_status == RunExecutionStatus::Completed {
        EvaluationTraceState::Completed {
            trace: load_completed(&coordinate, registration, source)?,
        }
    } else {
        EvaluationTraceState::NotCompleted {
            registration: evidence(registration, source),
        }
    };

    Ok(EvaluationTraceSnapshot {
        coordinate,
        version,
        epoch,
        authority: TraceAuthority::RunRegistry,
        trace,
    })
}

fn load_completed(
    coordinate: &EvaluationRunCoordinate,
    registration: RunRegistration,
    registration_source: TraceSource,
) -> Result<CompletedEvaluationTrace, PrepareError> {
    let (run, run_source) = load_run_record(&registration.artifacts.record_path)?;
    if run.schema_version != RUN_RECORD_SCHEMA_VERSION {
        return Err(trace_error(format!(
            "run '{}' uses unsupported record schema '{}'",
            coordinate.run_id, run.schema_version
        )));
    }
    if run.metadata.benchmark.instance_id != coordinate.instance.as_str() {
        return Err(trace_error(format!(
            "run record instance '{}' disagrees with requested instance '{}'",
            run.metadata.benchmark.instance_id, coordinate.instance
        )));
    }

    let turn = load_turn(&registration)?;
    if registration.frozen_spec.run_role == RegisteredRunRole::Treatment && turn.is_none() {
        return Err(trace_error(format!(
            "completed treatment run '{}' has no sealed agent-turn summary",
            coordinate.run_id
        )));
    }
    if let Some((summary, _)) = &turn {
        validate_turn(summary, &run, coordinate)?;
    }

    let exchanges = load_exchanges(&registration, turn.as_ref().map(|(record, _)| record))?;
    let protocol = load_protocol(&registration.artifacts.record_path)?;
    validate_protocol_lifecycle(&registration, &protocol)?;

    Ok(CompletedEvaluationTrace {
        registration: evidence(registration, registration_source),
        turn: turn.map(|(record, source)| evidence(record, source)),
        run: evidence(run, run_source),
        exchanges: exchanges.map(|(records, source)| evidence(records, source)),
        protocol: protocol
            .into_iter()
            .map(|(file, source)| evidence(file, source))
            .collect(),
    })
}

fn coordinate_for(registration: &RunRegistration) -> Result<EvaluationRunCoordinate, PrepareError> {
    let instance = registration.frozen_spec.task_id.clone();
    let campaign = registration
        .frozen_spec
        .campaign_id
        .clone()
        .ok_or_else(|| {
            trace_error(format!(
                "registration '{}' has no owning campaign identity",
                registration.run_id
            ))
        })?;
    let coordinate = EvaluationRunCoordinate {
        campaign,
        instance: InstanceId(instance),
        run_id: registration.run_id.clone(),
    };
    validate_coordinate(&coordinate)?;
    Ok(coordinate)
}

fn validate_coordinate(coordinate: &EvaluationRunCoordinate) -> Result<(), PrepareError> {
    if coordinate.campaign.as_str().is_empty() {
        return Err(trace_error("campaign identity must not be empty"));
    }
    validate_component("instance", coordinate.instance.as_str())?;
    validate_component("run id", &coordinate.run_id)
}

fn validate_component(label: &str, value: &str) -> Result<(), PrepareError> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(trace_error(format!(
            "{label} must be exactly one normal path component"
        )));
    }
    Ok(())
}

fn validate_registration(
    scope: &TraceScope,
    coordinate: &EvaluationRunCoordinate,
    registration: &RunRegistration,
) -> Result<(), PrepareError> {
    use crate::inner::registry::RUN_REGISTRATION_SCHEMA_VERSION;

    if registration.schema_version != RUN_REGISTRATION_SCHEMA_VERSION {
        return Err(trace_error(format!(
            "run '{}' uses unsupported registration schema '{}'",
            coordinate.run_id, registration.schema_version
        )));
    }
    if registration.run_id != coordinate.run_id {
        return Err(trace_error(format!(
            "registration run id '{}' disagrees with requested run id '{}'",
            registration.run_id, coordinate.run_id
        )));
    }
    if registration.frozen_spec.task_id != coordinate.instance.as_str() {
        return Err(trace_error(format!(
            "registration task '{}' disagrees with requested instance '{}'",
            registration.frozen_spec.task_id, coordinate.instance
        )));
    }
    if registration.frozen_spec.campaign_id.as_ref() != Some(&coordinate.campaign) {
        return Err(trace_error(format!(
            "registration campaign '{}' disagrees with requested campaign '{}'",
            registration
                .frozen_spec
                .campaign_id
                .as_ref()
                .map_or("-", CampaignId::as_str),
            coordinate.campaign
        )));
    }
    if registration.intent.freeze() != registration.frozen_spec {
        return Err(trace_error(format!(
            "registration '{}' intent does not reproduce its frozen specification",
            coordinate.run_id
        )));
    }
    let fingerprint = registration
        .frozen_spec
        .fingerprint()
        .map_err(PrepareError::Serialize)?;
    if fingerprint != registration.spec_fingerprint {
        return Err(trace_error(format!(
            "registration '{}' frozen-spec fingerprint is invalid",
            coordinate.run_id
        )));
    }

    let run_root = &registration.artifacts.run_root;
    if !run_root.starts_with(&scope.instances_root)
        || run_root.file_name().and_then(|name| name.to_str()) != Some(&coordinate.run_id)
        || run_root
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != Some("runs")
        || run_root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != Some(coordinate.instance.as_str())
    {
        return Err(trace_error(format!(
            "registration '{}' run root '{}' is outside the selected campaign/instance layout",
            coordinate.run_id,
            run_root.display()
        )));
    }
    let runs_dir = run_root.parent().expect("validated run root has a parent");
    if registration.frozen_spec.storage_roots.runs_dir != runs_dir
        || registration.artifacts.record_path != run_root.join("record.json.gz")
        || registration.record_path() != registration.artifacts.record_path
    {
        return Err(trace_error(format!(
            "registration '{}' artifact roots do not match its frozen storage coordinates",
            coordinate.run_id
        )));
    }
    if registration.registry_path()
        != scope
            .registry_root
            .join("runs")
            .join(format!("{}.json", coordinate.run_id))
    {
        return Err(trace_error(format!(
            "registration '{}' does not resolve to the selected global registry",
            coordinate.run_id
        )));
    }
    for (label, path, expected) in [
        (
            "turn trace",
            registration.artifacts.turn_trace.as_ref(),
            "agent-turn-trace.json",
        ),
        (
            "turn summary",
            registration.artifacts.turn_summary.as_ref(),
            "agent-turn-summary.json",
        ),
        (
            "model responses",
            registration.artifacts.full_response_trace.as_ref(),
            "llm-full-responses.jsonl",
        ),
    ] {
        if let Some(path) = path
            && (path.parent() != Some(run_root.as_path())
                || path.file_name().and_then(|name| name.to_str()) != Some(expected))
        {
            return Err(trace_error(format!(
                "registration '{}' {label} path '{}' is not canonical",
                coordinate.run_id,
                path.display()
            )));
        }
    }
    let protocol_dir = layout::protocol_artifacts_dir_for_run(run_root);
    if registration.artifacts.protocol_artifacts_dir != protocol_dir {
        return Err(trace_error(format!(
            "registration '{}' protocol artifact directory '{}' is not canonical",
            coordinate.run_id,
            registration.artifacts.protocol_artifacts_dir.display()
        )));
    }
    Ok(())
}

fn load_registration_source(registration: &RunRegistration) -> Result<TraceSource, PrepareError> {
    let path = registration.registry_path();
    let (loaded, source) = load_registration(&path)?;
    if &loaded != registration {
        return Err(trace_error(format!(
            "registration '{}' changed while the trace index was loaded",
            path.display()
        )));
    }
    Ok(source)
}

fn load_registration(path: &Path) -> Result<(RunRegistration, TraceSource), PrepareError> {
    let before = read_bytes(path)?;
    let registration =
        RunRegistration::load(path).map_err(|error| trace_error(error.to_string()))?;
    let after = read_bytes(path)?;
    if before != after {
        return Err(trace_error(format!(
            "registration '{}' changed while it was read",
            path.display()
        )));
    }
    Ok((
        registration,
        source(TraceSourceKind::Registration, path, &before),
    ))
}

fn load_run_record(path: &Path) -> Result<(RunRecord, TraceSource), PrepareError> {
    let bytes = read_bytes(path)?;
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut json = Vec::new();
    decoder
        .read_to_end(&mut json)
        .map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
    let record = serde_json::from_slice(&json).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })?;
    Ok((record, source(TraceSourceKind::RunRecord, path, &bytes)))
}

fn load_turn(
    registration: &RunRegistration,
) -> Result<Option<(AgentTurnSummaryRecord, TraceSource)>, PrepareError> {
    let Some(path) = registration.artifacts.turn_summary.as_deref() else {
        return Ok(None);
    };
    let bytes = read_bytes(path)?;
    let record = serde_json::from_slice::<AgentTurnSummaryRecord>(&bytes).map_err(|source| {
        PrepareError::ParseManifest {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let after = read_bytes(path)?;
    if bytes != after {
        return Err(trace_error(format!(
            "sealed turn summary '{}' changed while it was read",
            path.display()
        )));
    }
    Ok(Some((
        record,
        source(TraceSourceKind::TurnSummary, path, &bytes),
    )))
}

fn validate_turn(
    summary: &AgentTurnSummaryRecord,
    run: &RunRecord,
    coordinate: &EvaluationRunCoordinate,
) -> Result<(), PrepareError> {
    let artifact = &summary.0;
    if artifact.task_id != coordinate.instance.as_str() {
        return Err(trace_error(format!(
            "turn summary task '{}' disagrees with requested instance '{}'",
            artifact.task_id, coordinate.instance
        )));
    }
    let terminal = artifact.terminal_record.as_ref().ok_or_else(|| {
        trace_error(format!(
            "completed run '{}' turn summary has no terminal record",
            coordinate.run_id
        ))
    })?;
    let has_finished = artifact.events.iter().any(|event| {
        matches!(event, ObservedTurnEventRecord::TurnFinished(finished) if finished == terminal)
    });
    if !has_finished {
        return Err(trace_error(format!(
            "completed run '{}' turn summary has no matching TurnFinished event",
            coordinate.run_id
        )));
    }
    let matches = run
        .phases
        .agent_turns
        .iter()
        .filter_map(|turn| turn.agent_turn_artifact.as_ref())
        .filter(|embedded| *embedded == artifact)
        .count();
    if matches != 1 {
        return Err(trace_error(format!(
            "completed run '{}' record contains {matches} exact copies of its sealed turn summary",
            coordinate.run_id
        )));
    }
    Ok(())
}

fn load_exchanges(
    registration: &RunRegistration,
    turn: Option<&AgentTurnSummaryRecord>,
) -> Result<Option<(Vec<ModelExchange>, TraceSource)>, PrepareError> {
    let Some(path) = registration.artifacts.full_response_trace.as_deref() else {
        return Ok(None);
    };
    let bytes = read_bytes(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|source| {
        trace_error(format!(
            "model response trace '{}' is not UTF-8: {source}",
            path.display()
        ))
    })?;
    let lines = decode_full_response_lines(text).map_err(|source| {
        trace_error(format!(
            "failed to decode model response trace '{}': {source}",
            path.display()
        ))
    })?;
    if let Some(summary) = turn {
        let terminal =
            summary.0.terminal_record.as_ref().ok_or_else(|| {
                trace_error("sealed turn summary lost its validated terminal record")
            })?;
        let assistant =
            uuid::Uuid::parse_str(&terminal.assistant_message_id).map_err(|source| {
                trace_error(format!(
                    "terminal assistant message id '{}' is invalid: {source}",
                    terminal.assistant_message_id
                ))
            })?;
        if lines
            .iter()
            .any(|line| !line.record().matches_assistant_message(assistant))
        {
            return Err(trace_error(format!(
                "model response trace '{}' contains a response for a different assistant message",
                path.display()
            )));
        }
    } else if !lines.is_empty() {
        return Err(trace_error(format!(
            "run '{}' has model responses but no sealed turn summary",
            registration.run_id
        )));
    }
    let records = lines
        .into_iter()
        .map(|line| ModelExchange {
            source_line: line.line(),
            record: line.into_record(),
        })
        .collect();
    Ok(Some((
        records,
        source(TraceSourceKind::ModelResponses, path, &bytes),
    )))
}

fn load_protocol(record_path: &Path) -> Result<Vec<(ArtifactFile, TraceSource)>, PrepareError> {
    let before = protocol_bytes(record_path)?;
    let loaded = list_protocol_artifact_load_results(record_path)?;
    let after = protocol_bytes(record_path)?;
    if before != after {
        return Err(trace_error(format!(
            "protocol artifact set for '{}' changed while it was read",
            record_path.display()
        )));
    }
    let mut artifacts = Vec::new();
    for result in loaded {
        match result {
            ProtocolArtifactLoadResult::Loaded(file) => {
                if file.artifact.schema_version != PROTOCOL_SCHEMA_V1 {
                    return Err(trace_error(format!(
                        "protocol artifact '{}' uses unsupported schema '{}'",
                        file.path.display(),
                        file.artifact.schema_version
                    )));
                }
                validate_protocol_payload(&file)?;
                let bytes = before.get(&file.path).ok_or_else(|| {
                    trace_error(format!(
                        "typed protocol artifact '{}' was absent from the stable source snapshot",
                        file.path.display()
                    ))
                })?;
                let source = source(TraceSourceKind::ProtocolArtifact, &file.path, bytes);
                artifacts.push((
                    ArtifactFile {
                        path: file.path,
                        artifact: file.artifact,
                    },
                    source,
                ));
            }
            ProtocolArtifactLoadResult::Unloaded(failure) => {
                return Err(protocol_failure(failure));
            }
        }
    }
    artifacts.sort_by(|left, right| left.0.path.cmp(&right.0.path));
    if artifacts.len() != before.len() {
        return Err(trace_error(format!(
            "typed protocol loader returned {} artifact(s) for {} stable JSON source file(s)",
            artifacts.len(),
            before.len()
        )));
    }
    Ok(artifacts)
}

fn protocol_bytes(record_path: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, PrepareError> {
    let run_root = record_path.parent().ok_or_else(|| {
        trace_error(format!(
            "run record '{}' has no containing run directory",
            record_path.display()
        ))
    })?;
    let mut files = BTreeMap::new();
    for directory in layout::protocol_artifact_read_dirs_for_run(run_root) {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(PrepareError::ReadProtocolArtifact {
                    path: directory,
                    source,
                });
            }
        };
        for entry in entries {
            let entry = entry.map_err(|source| PrepareError::ReadProtocolArtifact {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = read_bytes(&path)?;
            if files.insert(path.clone(), bytes).is_some() {
                return Err(trace_error(format!(
                    "protocol artifact '{}' was discovered through more than one source directory",
                    path.display()
                )));
            }
        }
    }
    Ok(files)
}

fn protocol_failure(failure: ProtocolArtifactLoadFailure) -> PrepareError {
    let detail = match failure {
        ProtocolArtifactLoadFailure::Decode(record) => format!(
            "protocol artifact '{}' failed typed decoding: {}",
            record
                .path
                .as_deref()
                .map_or_else(|| "-".to_string(), |path| path.display().to_string()),
            record.error
        ),
        ProtocolArtifactLoadFailure::Identity(mismatch) => mismatch.to_string(),
    };
    trace_error(detail)
}

fn validate_protocol_payload(
    file: &crate::protocol_artifacts::DecodedProtocolArtifactFile,
) -> Result<(), PrepareError> {
    let expected_run = file.artifact.run_id.as_str();
    let expected_subject = file.artifact.subject_id.as_str();
    let fields: Vec<(&str, &str, &str)> = match &file.artifact.body {
        ArtifactBody::ToolCallIntentSegmentation(payload) => vec![
            (
                "input.subject_id",
                payload.input.subject_id.as_str(),
                expected_subject,
            ),
            (
                "output.sequence.subject_id",
                payload.output.sequence.subject_id.as_str(),
                expected_subject,
            ),
        ],
        ArtifactBody::ToolCallReview(payload) => {
            validate_call_review(&file.path, &payload.input, &payload.output)?;
            vec![
                (
                    "input.subject_id",
                    payload.input.subject_id.as_str(),
                    expected_subject,
                ),
                (
                    "output.packet.subject_id",
                    payload.output.packet.subject_id.as_str(),
                    expected_subject,
                ),
            ]
        }
        ArtifactBody::ToolCallSegmentReview(payload) => vec![
            (
                "input.subject_id",
                payload.input.subject_id.as_str(),
                expected_subject,
            ),
            (
                "input.sequence.subject_id",
                payload.input.sequence.subject_id.as_str(),
                expected_subject,
            ),
            (
                "output.packet.subject_id",
                payload.output.packet.subject_id.as_str(),
                expected_subject,
            ),
        ],
        ArtifactBody::InterventionIssueDetection(payload) => vec![
            ("input.run_id", payload.input.run_id.as_str(), expected_run),
            (
                "input.subject_id",
                payload.input.subject_id.as_str(),
                expected_subject,
            ),
        ],
        ArtifactBody::InterventionSynthesis(_) | ArtifactBody::InterventionApply(_) => Vec::new(),
    };
    for (field, actual, expected) in fields {
        if actual != expected {
            return Err(trace_error(format!(
                "protocol artifact '{}' {field} '{}' disagrees with stored identity '{}'",
                file.path.display(),
                actual,
                expected
            )));
        }
    }
    Ok(())
}

fn validate_call_review(
    path: &Path,
    input: &ToolCallNeighborhood,
    output: &LocalAnalysisAssessment,
) -> Result<(), PrepareError> {
    let index = input.focal.index;
    if output.packet.target_kind != LocalAnalysisTargetKind::FocalCall {
        return Err(trace_error(format!(
            "tool-call review artifact '{}' output target is {:?}, not a focal call",
            path.display(),
            output.packet.target_kind
        )));
    }
    if output.packet.focal_call_index != Some(index) {
        return Err(trace_error(format!(
            "tool-call review artifact '{}' input focal index {} disagrees with output focal index {:?}",
            path.display(),
            index,
            output.packet.focal_call_index
        )));
    }
    let mut calls = output
        .packet
        .calls
        .iter()
        .filter(|call| call.index == index);
    let Some(call) = calls.next() else {
        return Err(trace_error(format!(
            "tool-call review artifact '{}' output packet has no focal call at index {}",
            path.display(),
            index
        )));
    };
    if calls.next().is_some() {
        return Err(trace_error(format!(
            "tool-call review artifact '{}' output packet repeats focal call index {}",
            path.display(),
            index
        )));
    }
    if call.tool_name != input.focal.tool_name {
        return Err(trace_error(format!(
            "tool-call review artifact '{}' input focal tool '{}' disagrees with output focal tool '{}'",
            path.display(),
            input.focal.tool_name,
            call.tool_name
        )));
    }
    Ok(())
}

fn validate_protocol_lifecycle(
    registration: &RunRegistration,
    artifacts: &[(ArtifactFile, TraceSource)],
) -> Result<(), PrepareError> {
    let status = registration.lifecycle.protocol.status;
    let artifact_count = artifacts.len();
    let inconsistent = match status {
        RunPhaseStatus::NotStarted | RunPhaseStatus::Skipped => artifact_count != 0,
        RunPhaseStatus::Completed => artifact_count == 0,
        RunPhaseStatus::InProgress | RunPhaseStatus::Failed => false,
    };
    if inconsistent {
        return Err(trace_error(format!(
            "run '{}' protocol lifecycle {:?} disagrees with {} typed artifact(s)",
            registration.run_id, status, artifact_count
        )));
    }
    match registration.artifacts.protocol_anchor.as_ref() {
        Some(anchor) if !artifacts.iter().any(|(file, _)| &file.path == anchor) => {
            return Err(trace_error(format!(
                "run '{}' protocol anchor '{}' does not name a loaded typed artifact",
                registration.run_id,
                anchor.display()
            )));
        }
        Some(anchor) => {
            let anchored = artifacts
                .iter()
                .find(|(file, _)| &file.path == anchor)
                .expect("validated protocol anchor is loaded");
            let latest = artifacts
                .iter()
                .map(|(file, _)| file.artifact.created_at_ms)
                .max()
                .expect("a loaded protocol anchor implies a nonempty artifact set");
            if anchored.0.artifact.created_at_ms != latest {
                return Err(trace_error(format!(
                    "run '{}' protocol anchor '{}' is not a latest typed artifact",
                    registration.run_id,
                    anchor.display()
                )));
            }
        }
        None if artifact_count != 0 => {
            return Err(trace_error(format!(
                "run '{}' has {} typed protocol artifact(s) but no artifact anchor",
                registration.run_id, artifact_count
            )));
        }
        None if status == RunPhaseStatus::Completed => {
            return Err(trace_error(format!(
                "run '{}' completed protocol lifecycle has no typed artifact anchor",
                registration.run_id
            )));
        }
        None => {}
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, PrepareError> {
    fs::read(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn source(kind: TraceSourceKind, path: &Path, bytes: &[u8]) -> TraceSource {
    TraceSource {
        kind,
        path: path.to_path_buf(),
        content_sha256: hex_sha256(bytes),
    }
}

fn evidence<T>(value: T, source: TraceSource) -> TraceEvidence<T> {
    TraceEvidence { value, source }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn trace_error(detail: impl Into<String>) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: ERROR_PHASE,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{Compression, write::GzEncoder};
    use ploke_records::{
        agent_turn::{
            AgentTurnArtifactRecord, ObservedTurnEventRecord, PatchArtifactRecord,
            ToolRequestRecord, TurnFinishedRecord,
        },
        run_record::{TurnOutcome, TurnRecord},
        tool_contracts::ToolArgumentsJson,
    };
    use serde_json::json;
    use tempfile::TempDir;

    use crate::{
        cli::prototype1_state::walk::{
            epoch::{TRANSITION_GRAPH_VERSION, WALK_PROTOCOL_VERSION},
            ipc,
            protocol::{WalkRequest, WalkRequestBody, WalkResponse},
        },
        inner::{RunIntent, RunStorageRoots, core::RegisteredRunRole, registry::RunRegistration},
        spec::EvalBudget,
    };

    use super::*;

    const INSTANCE: &str = "org__repo-1";
    const RUN_ID: &str = "run-trace-fixture";
    const ASSISTANT_ID: &str = "8e32b33b-6de5-4e1c-9fa1-14bc2059913f";

    struct Fixture {
        _root: TempDir,
        _env: crate::test_support::EnvGuard,
        scope: TraceScope,
        registration: RunRegistration,
    }

    #[test]
    fn completed_trace_preserves_physical_model_order_and_source_hashes() {
        let mut fixture = Fixture::new(RegisteredRunRole::Treatment);
        let artifact = turn_artifact();
        fixture.seal(Some(artifact.clone()));

        let summary_path = fixture
            .registration
            .artifacts
            .turn_summary
            .clone()
            .expect("summary path");
        fs::write(
            &summary_path,
            serde_json::to_vec(&AgentTurnSummaryRecord(artifact.clone())).expect("summary json"),
        )
        .expect("write summary");
        let mutable_trace = fixture
            .registration
            .artifacts
            .run_root
            .join("agent-turn-trace.json");
        fixture.registration.artifacts.turn_trace = Some(mutable_trace.clone());
        fs::write(mutable_trace, b"{truncated mutable trace").expect("write mutable trace");
        write_run_record(&fixture.registration.artifacts.record_path, Some(artifact));
        let response_path = fixture
            .registration
            .artifacts
            .full_response_trace
            .clone()
            .expect("response path");
        fs::write(
            &response_path,
            format!("{}\n\n{}\n", response_json(9), response_json(2)),
        )
        .expect("write responses");
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let snapshot = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load completed trace");
        let EvaluationTraceState::Completed { trace } = snapshot.trace else {
            panic!("expected completed trace");
        };
        let exchanges = &trace.exchanges.as_ref().expect("recorded exchanges").value;
        assert_eq!(
            exchanges
                .iter()
                .map(|exchange| exchange.source_line)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(exchanges[0].record.response_index().get(), 9);
        assert_eq!(exchanges[1].record.response_index().get(), 2);
        assert_eq!(
            trace.turn.as_ref().map(|turn| &turn.value.0),
            Some(&artifact_for_assert())
        );
        for source in trace.sources() {
            let bytes = fs::read(&source.path).expect("source remains readable");
            assert_eq!(source.content_sha256, hex_sha256(&bytes));
        }
    }

    #[tokio::test]
    async fn completed_trace_roundtrips_through_walk_ipc() {
        let mut fixture = Fixture::new(RegisteredRunRole::Treatment);
        let artifact = turn_artifact();
        fixture.seal(Some(artifact.clone()));
        let summary_path = fixture
            .registration
            .artifacts
            .turn_summary
            .clone()
            .expect("summary path");
        fs::write(
            summary_path,
            serde_json::to_vec(&AgentTurnSummaryRecord(artifact.clone())).expect("summary json"),
        )
        .expect("write summary");
        write_run_record(&fixture.registration.artifacts.record_path, Some(artifact));
        let response_path = fixture
            .registration
            .artifacts
            .full_response_trace
            .clone()
            .expect("response path");
        fs::write(response_path, format!("{}\n", response_json(1))).expect("write responses");
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let snapshot = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load completed trace");
        let response = WalkResponse::evaluation_trace(snapshot);
        let (mut sender, mut receiver) =
            tokio::net::UnixStream::pair().expect("create IPC stream pair");
        let send = tokio::spawn(async move { ipc::send(&mut sender, &response).await });
        let decoded: WalkResponse = ipc::recv(&mut receiver)
            .await
            .expect("receive completed trace response");
        send.await.expect("join IPC sender").expect("send response");

        let WalkResponse::EvaluationTrace { snapshot } = decoded else {
            panic!("expected evaluation trace response");
        };
        let EvaluationTraceState::Completed { trace } = snapshot.trace else {
            panic!("expected completed evaluation trace");
        };
        let turn = &trace.turn.expect("sealed turn summary").value.0;
        let request = turn.events.iter().find_map(|event| match event {
            ObservedTurnEventRecord::ToolRequested(request) => Some(request),
            _ => None,
        });
        let request = request.expect("persisted tool request");
        assert_eq!(
            request.call_id,
            "function-call-4ab08ea8-ecc1-4fe9-81dc-243b586654d1"
        );
        assert_eq!(request.tool, "request_code_context");
        assert_eq!(
            request.arguments.as_str(),
            r#"{  "search_term" : "replacement multiline printer pcre2"  }"#
        );
    }

    #[test]
    fn non_completed_run_never_opens_mutable_turn_trace() {
        let mut fixture = Fixture::new(RegisteredRunRole::Treatment);
        let trace_path = fixture
            .registration
            .artifacts
            .run_root
            .join("agent-turn-trace.json");
        fixture.registration.artifacts.turn_trace = Some(trace_path.clone());
        fs::write(&trace_path, b"{truncated").expect("write invalid mutable trace");
        fixture.registration.persist().expect("persist running run");

        let snapshot = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load lifecycle-only trace");
        let response = WalkResponse::evaluation_trace(snapshot.clone());
        let encoded = serde_json::to_value(&response).expect("serialize trace response");
        let decoded: WalkResponse =
            serde_json::from_value(encoded.clone()).expect("deserialize trace response");
        assert_eq!(
            serde_json::to_value(decoded).expect("reserialize trace response"),
            encoded
        );
        let EvaluationTraceState::NotCompleted { registration } = snapshot.trace else {
            panic!("expected lifecycle-only result");
        };
        assert_eq!(
            registration.value.lifecycle.execution_status,
            RunExecutionStatus::Registered
        );
        assert_eq!(registration.source.kind, TraceSourceKind::Registration);
    }

    #[test]
    fn completed_index_excludes_non_completed_registrations() {
        let mut completed = Fixture::new(RegisteredRunRole::Control);
        completed.seal(None);
        write_run_record(&completed.registration.artifacts.record_path, None);
        completed
            .registration
            .persist()
            .expect("persist completed run");

        let running = RunRegistration::register_with_run_id(
            RunIntent {
                task_id: "org__repo-2".to_string(),
                repo_root: completed.scope.instances_root.join("repo-2"),
                storage_roots: RunStorageRoots::new(
                    completed.scope.registry_root.clone(),
                    completed
                        .scope
                        .instances_root
                        .join("org__repo-2")
                        .join("runs"),
                ),
                base_sha: None,
                budget: EvalBudget::default(),
                model_id: None,
                provider_slug: None,
                campaign_id: Some(completed.scope.campaign.clone()),
                batch_id: None,
                run_arm_id: "shell-only".to_string(),
                run_role: RegisteredRunRole::Control,
            },
            "run-not-completed",
        )
        .expect("running registration");
        running.persist().expect("persist running run");

        let index = load_index_for(
            completed.scope.clone(),
            SessionVersion::empty(),
            completed.epoch(),
        )
        .expect("load completed index");
        assert_eq!(index.runs.len(), 1);
        assert_eq!(index.runs[0].coordinate.run_id, RUN_ID);
    }

    #[test]
    fn nested_treatment_preserves_its_owning_campaign_coordinate() {
        let mut fixture = Fixture::nested_treatment();
        let artifact = turn_artifact();
        fixture.seal(Some(artifact.clone()));
        let summary_path = fixture
            .registration
            .artifacts
            .turn_summary
            .clone()
            .expect("summary path");
        fs::write(
            summary_path,
            serde_json::to_vec(&AgentTurnSummaryRecord(artifact.clone())).expect("summary json"),
        )
        .expect("write summary");
        fs::write(
            fixture
                .registration
                .artifacts
                .full_response_trace
                .as_ref()
                .expect("response path"),
            format!("{}\n", response_json(0)),
        )
        .expect("write responses");
        write_run_record(&fixture.registration.artifacts.record_path, Some(artifact));
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let index = load_index_for(
            fixture.scope.clone(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load nested treatment index");
        assert_eq!(index.runs.len(), 1);
        assert_eq!(
            index.runs[0].coordinate.campaign.as_str(),
            "campaign-trace-treatment"
        );
        assert_eq!(
            index.runs[0]
                .registration
                .value
                .frozen_spec
                .campaign_id
                .as_ref(),
            Some(&index.runs[0].coordinate.campaign)
        );
        load_run_for(
            fixture.scope.clone(),
            index.runs[0].coordinate.clone(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load exact nested treatment trace");
    }

    #[test]
    fn completed_protocol_lifecycle_requires_typed_artifacts() {
        let mut fixture = Fixture::new(RegisteredRunRole::Control);
        fixture.seal(None);
        fixture.registration.lifecycle.protocol.status = RunPhaseStatus::Completed;
        write_run_record(&fixture.registration.artifacts.record_path, None);
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let error = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect_err("completed protocol lifecycle without artifacts must fail");
        assert!(error.to_string().contains("protocol lifecycle Completed"));
    }

    #[test]
    fn valid_anchored_protocol_artifact_is_exposed() {
        let mut fixture = Fixture::new(RegisteredRunRole::Control);
        complete_control(&mut fixture);
        let anchor = write_issue_artifact(
            &fixture,
            "issue-1.json",
            PROTOCOL_SCHEMA_V1,
            RUN_ID,
            INSTANCE,
            41,
        );
        fixture.registration.lifecycle.protocol.status = RunPhaseStatus::Completed;
        fixture.registration.artifacts.protocol_anchor = Some(anchor.clone());
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let snapshot = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect("load valid protocol evidence");
        let EvaluationTraceState::Completed { trace } = snapshot.trace else {
            panic!("expected completed trace");
        };
        assert_eq!(trace.protocol.len(), 1);
        assert_eq!(trace.protocol[0].value.path, anchor);
        assert_eq!(
            trace.protocol[0].value.artifact.schema_version,
            PROTOCOL_SCHEMA_V1
        );
    }

    #[test]
    fn protocol_trace_rejects_future_schema() {
        let mut fixture = Fixture::new(RegisteredRunRole::Control);
        complete_control(&mut fixture);
        let anchor = write_issue_artifact(
            &fixture,
            "future.json",
            "protocol-artifact.v999",
            RUN_ID,
            INSTANCE,
            42,
        );
        fixture.registration.lifecycle.protocol.status = RunPhaseStatus::InProgress;
        fixture.registration.artifacts.protocol_anchor = Some(anchor);
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let error = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect_err("future protocol schema must fail");
        assert!(error.to_string().contains("unsupported schema"));
    }

    #[test]
    fn protocol_trace_rejects_nested_input_identity_mismatch() {
        for (field, input_run, input_subject) in [
            ("input.run_id", "run-other", INSTANCE),
            ("input.subject_id", RUN_ID, "org__other-2"),
        ] {
            let mut fixture = Fixture::new(RegisteredRunRole::Control);
            complete_control(&mut fixture);
            let anchor = write_issue_artifact(
                &fixture,
                "mismatch.json",
                PROTOCOL_SCHEMA_V1,
                input_run,
                input_subject,
                43,
            );
            fixture.registration.lifecycle.protocol.status = RunPhaseStatus::InProgress;
            fixture.registration.artifacts.protocol_anchor = Some(anchor);
            fixture
                .registration
                .persist()
                .expect("persist completed run");

            let error = load_run_for(
                fixture.scope.clone(),
                fixture.coordinate(),
                SessionVersion::empty(),
                fixture.epoch(),
            )
            .expect_err("nested protocol identity mismatch must fail");
            assert!(
                error.to_string().contains(field),
                "unexpected identity error: {error}"
            );
        }
    }

    #[test]
    fn call_review_validation_requires_one_matching_focal_call() {
        let input = review_neighborhood(4, "read_file");
        let mut output = review_assessment(4, "read_file");
        let path = Path::new("/tmp/review.json");

        validate_call_review(path, &input, &output).expect("valid focal-call review");

        output.packet.target_kind = LocalAnalysisTargetKind::IntentSegment;
        let error =
            validate_call_review(path, &input, &output).expect_err("non-focal target must fail");
        assert!(error.to_string().contains("not a focal call"));

        output.packet.target_kind = LocalAnalysisTargetKind::FocalCall;
        let focal = output.packet.calls.remove(0);
        let error =
            validate_call_review(path, &input, &output).expect_err("missing focal call must fail");
        assert!(error.to_string().contains("has no focal call"));
        output.packet.calls.push(focal);

        output.packet.focal_call_index = Some(3);
        let error = validate_call_review(path, &input, &output)
            .expect_err("mismatched focal index must fail");
        assert!(
            error
                .to_string()
                .contains("disagrees with output focal index")
        );

        output.packet.focal_call_index = Some(4);
        output.packet.calls[0].tool_name = "cargo".to_string();
        let error = validate_call_review(path, &input, &output)
            .expect_err("mismatched focal tool must fail");
        assert!(
            error
                .to_string()
                .contains("disagrees with output focal tool")
        );

        output.packet.calls[0].tool_name = "read_file".to_string();
        output.packet.calls.push(output.packet.calls[0].clone());
        let error = validate_call_review(path, &input, &output)
            .expect_err("repeated focal index must fail");
        assert!(error.to_string().contains("repeats focal call index"));
    }

    #[test]
    fn protocol_trace_rejects_missing_stale_and_outdated_anchors() {
        for anchor_case in ["missing", "stale", "outdated"] {
            let mut fixture = Fixture::new(RegisteredRunRole::Control);
            complete_control(&mut fixture);
            let first = write_issue_artifact(
                &fixture,
                "issue-1.json",
                PROTOCOL_SCHEMA_V1,
                RUN_ID,
                INSTANCE,
                44,
            );
            let latest = if anchor_case == "outdated" {
                Some(write_issue_artifact(
                    &fixture,
                    "issue-2.json",
                    PROTOCOL_SCHEMA_V1,
                    RUN_ID,
                    INSTANCE,
                    45,
                ))
            } else {
                None
            };
            fixture.registration.lifecycle.protocol.status = RunPhaseStatus::InProgress;
            fixture.registration.artifacts.protocol_anchor = match anchor_case {
                "missing" => None,
                "stale" => Some(
                    fixture
                        .registration
                        .artifacts
                        .protocol_artifacts_dir
                        .join("absent.json"),
                ),
                "outdated" => Some(first),
                _ => unreachable!("fixed anchor cases"),
            };
            fixture
                .registration
                .persist()
                .expect("persist completed run");

            let error = load_run_for(
                fixture.scope.clone(),
                fixture.coordinate(),
                SessionVersion::empty(),
                fixture.epoch(),
            )
            .expect_err("invalid protocol anchor must fail");
            let detail = error.to_string();
            match anchor_case {
                "missing" => assert!(detail.contains("no artifact anchor")),
                "stale" => assert!(detail.contains("does not name a loaded typed artifact")),
                "outdated" => assert!(detail.contains("is not a latest typed artifact")),
                _ => unreachable!("fixed anchor cases"),
            }
            drop(latest);
        }
    }

    #[test]
    fn completed_treatment_requires_sealed_summary() {
        let mut fixture = Fixture::new(RegisteredRunRole::Treatment);
        fixture.seal(None);
        fixture.registration.artifacts.turn_summary = None;
        fixture.registration.artifacts.full_response_trace = None;
        write_run_record(&fixture.registration.artifacts.record_path, None);
        fixture
            .registration
            .persist()
            .expect("persist completed run");

        let error = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect_err("missing treatment summary must fail");
        assert!(error.to_string().contains("no sealed agent-turn summary"));
    }

    #[test]
    fn exact_trace_rejects_path_coordinates_and_protocol_decode_failure() {
        let fixture = Fixture::new(RegisteredRunRole::Control);
        let traversal = EvaluationRunCoordinate {
            campaign: fixture.scope.campaign.clone(),
            instance: InstanceId("../escape".to_string()),
            run_id: RUN_ID.to_string(),
        };
        let error = load_run_for(
            fixture.scope.clone(),
            traversal,
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect_err("path traversal must fail");
        assert!(error.to_string().contains("one normal path component"));

        let request = WalkRequest {
            client_protocol: None,
            client_epoch: None,
            body: WalkRequestBody::EvaluationTrace {
                coordinate: fixture.coordinate(),
            },
        };
        let encoded = serde_json::to_value(&request).expect("serialize trace request");
        let decoded: WalkRequest =
            serde_json::from_value(encoded.clone()).expect("deserialize trace request");
        assert_eq!(decoded, request);
        let mut unknown = encoded;
        unknown["body"]["coordinate"]["unexpected"] = json!(true);
        assert!(
            serde_json::from_value::<WalkRequest>(unknown).is_err(),
            "trace coordinate must reject unknown fields"
        );

        let mut fixture = fixture;
        fixture.seal(None);
        write_run_record(&fixture.registration.artifacts.record_path, None);
        fixture
            .registration
            .persist()
            .expect("persist completed run");
        let protocol_dir = &fixture.registration.artifacts.protocol_artifacts_dir;
        fs::create_dir_all(protocol_dir).expect("create protocol dir");
        fs::write(protocol_dir.join("broken.json"), b"{broken").expect("write bad protocol");
        let error = load_run_for(
            fixture.scope.clone(),
            fixture.coordinate(),
            SessionVersion::empty(),
            fixture.epoch(),
        )
        .expect_err("malformed protocol artifact must fail");
        assert!(error.to_string().contains("failed typed decoding"));
    }

    impl Fixture {
        fn new(role: RegisteredRunRole) -> Self {
            Self::build(role, false)
        }

        fn nested_treatment() -> Self {
            Self::build(RegisteredRunRole::Treatment, true)
        }

        fn build(role: RegisteredRunRole, nested: bool) -> Self {
            let root = tempfile::tempdir().expect("temp root");
            let home = root.path().join("eval-home");
            let env = crate::test_support::env_guard_os(vec![(
                "PLOKE_EVAL_HOME",
                home.as_os_str().to_os_string(),
            )]);
            let scope = TraceScope {
                campaign: CampaignId::from("campaign-trace-fixture"),
                instances_root: home
                    .join("instances")
                    .join("prototype1")
                    .join("campaign-trace-fixture"),
                registry_root: home.join("registries"),
            };
            let campaign = if nested {
                CampaignId::from("campaign-trace-treatment")
            } else {
                scope.campaign.clone()
            };
            let runs_dir = if nested {
                scope
                    .instances_root
                    .join("treatments")
                    .join("branch-fixture")
                    .join("instances")
                    .join(INSTANCE)
                    .join("runs")
            } else {
                scope.instances_root.join(INSTANCE).join("runs")
            };
            let intent = RunIntent {
                task_id: INSTANCE.to_string(),
                repo_root: root.path().join("repo"),
                storage_roots: RunStorageRoots::new(scope.registry_root.clone(), runs_dir),
                base_sha: Some("deadbeef".to_string()),
                budget: EvalBudget::default(),
                model_id: Some("test/model".to_string()),
                provider_slug: Some("test".to_string()),
                campaign_id: Some(campaign),
                batch_id: None,
                run_arm_id: match role {
                    RegisteredRunRole::Control => "shell-only",
                    RegisteredRunRole::Treatment => "structured-current-policy",
                }
                .to_string(),
                run_role: role,
            };
            let registration =
                RunRegistration::register_with_run_id(intent, RUN_ID).expect("registration");
            registration.persist().expect("persist registration");
            Self {
                _root: root,
                _env: env,
                scope,
                registration,
            }
        }

        fn coordinate(&self) -> EvaluationRunCoordinate {
            EvaluationRunCoordinate {
                campaign: self
                    .registration
                    .frozen_spec
                    .campaign_id
                    .clone()
                    .expect("fixture campaign"),
                instance: InstanceId(INSTANCE.to_string()),
                run_id: RUN_ID.to_string(),
            }
        }

        fn epoch(&self) -> ServerEpoch {
            ServerEpoch {
                protocol_version: WALK_PROTOCOL_VERSION,
                transition_graph_version: TRANSITION_GRAPH_VERSION.to_string(),
                repo_root: self.scope.instances_root.clone(),
                exe_path: PathBuf::from("/tmp/ploke-eval"),
                exe_modified_unix_ms: None,
                git_head: None,
                active_branch: None,
                source_status_hash: None,
            }
        }

        fn seal(&mut self, artifact: Option<AgentTurnArtifactRecord>) {
            fs::create_dir_all(&self.registration.artifacts.run_root).expect("create run root");
            self.registration.artifacts.turn_summary = artifact.as_ref().map(|_| {
                self.registration
                    .artifacts
                    .run_root
                    .join("agent-turn-summary.json")
            });
            self.registration.artifacts.full_response_trace = artifact.as_ref().map(|_| {
                self.registration
                    .artifacts
                    .run_root
                    .join("llm-full-responses.jsonl")
            });
            self.registration.mark_completed();
        }
    }

    fn turn_artifact() -> AgentTurnArtifactRecord {
        artifact_for_assert()
    }

    fn artifact_for_assert() -> AgentTurnArtifactRecord {
        let terminal = TurnFinishedRecord {
            session_id: "session-1".to_string(),
            request_id: "request-1".to_string(),
            parent_id: "parent-1".to_string(),
            assistant_message_id: ASSISTANT_ID.to_string(),
            outcome: "completed".to_string(),
            error_id: None,
            summary: "done".to_string(),
            attempts: 1,
        };
        AgentTurnArtifactRecord {
            task_id: INSTANCE.to_string(),
            selected_model: "test/model".to_string(),
            model_route: None,
            issue_prompt: "Fix the fixture.".to_string(),
            user_message_id: "user-1".to_string(),
            events: vec![
                ObservedTurnEventRecord::ToolRequested(ToolRequestRecord {
                    request_id: "request-1".to_string(),
                    parent_id: "parent-1".to_string(),
                    call_id: "function-call-4ab08ea8-ecc1-4fe9-81dc-243b586654d1".to_string(),
                    tool: "request_code_context".to_string(),
                    arguments: ToolArgumentsJson::from(
                        r#"{  "search_term" : "replacement multiline printer pcre2"  }"#,
                    ),
                }),
                ObservedTurnEventRecord::TurnFinished(terminal.clone()),
            ],
            prompt_debug: None,
            terminal_record: Some(terminal),
            final_assistant_message: None,
            patch_artifact: PatchArtifactRecord {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: Vec::new(),
            llm_response: Some("done".to_string()),
        }
    }

    fn write_run_record(path: &Path, artifact: Option<AgentTurnArtifactRecord>) {
        let mut record: RunRecord = serde_json::from_value(json!({
            "schema_version": RUN_RECORD_SCHEMA_VERSION,
            "manifest_id": "manifest-fixture",
            "metadata": {
                "benchmark": {
                    "instance_id": INSTANCE,
                    "repo_root": "/tmp/repo",
                    "base_sha": null
                },
                "agent": {},
                "runtime": {},
                "budget": {
                    "max_turns": 1,
                    "max_tool_calls": 1,
                    "wall_clock_secs": 1
                }
            },
            "phases": {},
            "db_time_travel_index": []
        }))
        .expect("run record");
        if let Some(artifact) = artifact {
            record.phases.agent_turns.push(TurnRecord {
                turn_number: 1,
                started_at: "2026-07-14T00:00:00Z".to_string(),
                ended_at: "2026-07-14T00:00:01Z".to_string(),
                db_timestamp_micros: 1,
                issue_prompt: "Fix the fixture.".to_string(),
                llm_request: None,
                llm_response: None,
                tool_calls: Vec::new(),
                outcome: TurnOutcome::Content,
                agent_turn_artifact: Some(artifact),
            });
        }
        let file = fs::File::create(path).expect("create record");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder
            .write_all(&serde_json::to_vec(&record).expect("record json"))
            .expect("write record");
        encoder.finish().expect("finish record");
    }

    fn complete_control(fixture: &mut Fixture) {
        fixture.seal(None);
        write_run_record(&fixture.registration.artifacts.record_path, None);
    }

    fn write_issue_artifact(
        fixture: &Fixture,
        name: &str,
        schema: &str,
        input_run: &str,
        input_subject: &str,
        created_at_ms: u64,
    ) -> PathBuf {
        let directory = &fixture.registration.artifacts.protocol_artifacts_dir;
        fs::create_dir_all(directory).expect("create protocol directory");
        let path = directory.join(name);
        fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema_version": schema,
                "procedure_name": "intervention_issue_detection",
                "subject_id": INSTANCE,
                "run_id": RUN_ID,
                "created_at_ms": created_at_ms,
                "model_id": "test/model",
                "provider_slug": "test",
                "input": {
                    "run_id": input_run,
                    "subject_id": input_subject,
                    "total_calls_in_run": 0,
                    "anchor_segment_count": 0,
                    "protocol_reviewed_call_count": 0,
                    "protocol_reviewed_segment_count": 0,
                    "protocol_artifact_count": 0
                },
                "output": {"cases": []},
                "artifact": {"case_count": 0}
            }))
            .expect("serialize protocol artifact"),
        )
        .expect("write protocol artifact");
        path
    }

    fn response_json(index: usize) -> String {
        json!({
            "assistant_message_id": ASSISTANT_ID,
            "response_index": index,
            "response": {
                "id": format!("response-{index}"),
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {"role": "assistant", "content": "done"}
                }],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }
        })
        .to_string()
    }

    fn review_neighborhood(index: usize, tool: &str) -> ToolCallNeighborhood {
        serde_json::from_value(json!({
            "subject_id": INSTANCE,
            "total_calls_in_run": 5,
            "total_calls_in_turn": 1,
            "turn": {
                "turn": 1,
                "tool_count": 1,
                "failed_tool_count": 0,
                "patch_proposed": false,
                "patch_applied": false
            },
            "before": [],
            "focal": review_call(index, tool),
            "after": []
        }))
        .expect("review neighborhood")
    }

    fn review_assessment(index: usize, tool: &str) -> LocalAnalysisAssessment {
        serde_json::from_value(json!({
            "packet": {
                "subject_id": INSTANCE,
                "target_kind": "focal_call",
                "target_id": format!("call:{index}"),
                "scope_summary": "one source read",
                "total_calls_in_scope": 1,
                "total_calls_in_run": 5,
                "turn_span": [1],
                "focal_call_index": index,
                "calls": [review_call(index, tool)]
            },
            "signals": {
                "scope_turn_count": 1,
                "repeated_tool_name_count": 0,
                "distinct_tool_count": 1,
                "search_calls_in_scope": 0,
                "read_calls_in_scope": 1,
                "browse_calls_in_scope": 0,
                "edit_calls_in_scope": 0,
                "execute_calls_in_scope": 0,
                "failed_calls_in_scope": 0,
                "similar_search_neighbors": 0,
                "directory_pivots": 0
            },
            "usefulness": {
                "verdict": "key_progress",
                "confidence": "high",
                "rationale": "It found the target."
            },
            "redundancy": {
                "verdict": "distinct",
                "confidence": "high",
                "rationale": "It was the first read."
            },
            "recoverability": {
                "verdict": "no_recovery_needed",
                "confidence": "high",
                "rationale": "The call succeeded."
            },
            "overall": "focused_progress",
            "overall_confidence": "high",
            "synthesis_rationale": "The read directly advanced the task."
        }))
        .expect("review assessment")
    }

    fn review_call(index: usize, tool: &str) -> serde_json::Value {
        json!({
            "index": index,
            "turn": 1,
            "tool_name": tool,
            "tool_kind": "read",
            "failed": false,
            "latency_ms": 7,
            "summary": "read source",
            "args_preview": "src/lib.rs",
            "result_preview": "source"
        })
    }
}
