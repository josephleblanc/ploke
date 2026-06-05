use std::fs;
use std::path::{Path, PathBuf};

use ploke_protocol::ProtocolReasoningPolicy;
use ploke_records::protocol::InterventionApplyArtifact;

use super::PROTOCOL_HTTP_MAX_ATTEMPTS;
use super::handlers::closure::protocol_llm_config;
use crate::intervention::{
    INTERVENTION_APPLY_PROCEDURE, INTERVENTION_SYNTHESIS_PROCEDURE, InterventionApplyInput,
    InterventionApplyOutput, InterventionSynthesisInput, IssueCase, execute_intervention_apply,
    operation_target_artifact_id, synthesize_intervention_with_llm,
};
use crate::protocol_artifacts::write_protocol_artifact;
use crate::record::read_compressed_record;
use crate::spec::PrepareError;

pub(crate) async fn persist_intervention_synthesis_for_record(
    record_path: &Path,
    issue: IssueCase,
    source_state_id: String,
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<crate::intervention::InterventionSynthesisOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let target_relpath = PathBuf::from(issue.target_tool.description_artifact_relpath());
    let source_content =
        fs::read_to_string(&target_relpath).map_err(|source| PrepareError::ReadManifest {
            path: target_relpath.clone(),
            source,
        })?;
    let input = InterventionSynthesisInput {
        issue,
        source_state_id,
        source_content,
        // The generic CLI path does not yet know the durable Artifact target
        // for this record. Downstream layers will preserve a fallback text-file
        // surface id, but future backend-aware callers should pass an
        // OperationTarget here instead of relying on that fallback.
        operation_target: None,
    };
    let cfg = protocol_llm_config(
        model_id,
        None,
        provider,
        120,
        PROTOCOL_HTTP_MAX_ATTEMPTS,
        3200,
        ProtocolReasoningPolicy::default(),
    )?;
    let run = synthesize_intervention_with_llm(input.clone(), cfg.clone())
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "intervention_synthesis",
            detail: err.to_string(),
        })?;
    write_protocol_artifact(
        record_path,
        INTERVENTION_SYNTHESIS_PROCEDURE,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &input,
        &run.output,
        &run.artifact,
    )?;
    Ok(run.output)
}

pub(crate) fn persist_intervention_apply_for_record(
    record_path: &Path,
    synthesis: &crate::intervention::InterventionSynthesisOutput,
    candidate_id: &str,
    repo_root: &Path,
) -> Result<InterventionApplyOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let candidate = synthesis
        .candidate_set
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "intervention apply candidate '{}' not found for subject '{}'",
                candidate_id, subject_id
            ),
        })?;
    let base_artifact_id = synthesis
        .candidate_set
        .operation_target
        .as_ref()
        .and_then(operation_target_artifact_id)
        .cloned();
    let patch_id = candidate.patch_id.clone();
    let input = InterventionApplyInput {
        source_state_id: synthesis.candidate_set.source_state_id.clone(),
        candidate,
        target_relpath: synthesis.candidate_set.target_relpath.clone(),
        expected_source_content: synthesis.candidate_set.source_content.clone(),
        repo_root: repo_root.to_path_buf(),
        base_artifact_id,
        patch_id,
    };
    let output =
        execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
            phase: "intervention_apply",
            detail: source.to_string(),
        })?;
    let artifact = InterventionApplyArtifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_APPLY_PROCEDURE,
        &subject_id,
        None,
        None,
        &input,
        &output,
        &artifact,
    )?;
    Ok(output)
}
