use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::intervention::{
    INTERVENTION_ISSUE_DETECTION_PROCEDURE, IssueCase, IssueDetectionArtifactInput,
    IssueDetectionOutput, select_primary_issue,
};
use crate::protocol_artifacts::list_protocol_artifacts;
use crate::run_registry::resolve_protocol_run_identity;
use crate::spec::PrepareError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueArtifactRef {
    pub path: PathBuf,
    pub created_at_ms: u64,
    pub procedure_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueRunIdentity {
    pub record_path: PathBuf,
    pub run_dir: PathBuf,
    pub run_id: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDetectionAggregate {
    pub run: IssueRunIdentity,
    pub artifact: IssueArtifactRef,
    pub input: IssueDetectionArtifactInput,
    pub output: IssueDetectionOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_issue: Option<IssueCase>,
}

#[derive(Debug, Error)]
pub enum IssueDetectionAggregateError {
    #[error(transparent)]
    Source(#[from] PrepareError),
    #[error("no issue-detection protocol artifact found for run '{record_path}'")]
    MissingArtifact { record_path: PathBuf },
    #[error("failed to deserialize issue-detection artifact output from '{path}': {detail}")]
    DeserializeOutput { path: PathBuf, detail: String },
    #[error("failed to deserialize issue-detection artifact input from '{path}': {detail}")]
    DeserializeInput { path: PathBuf, detail: String },
}

pub fn load_issue_detection_aggregate(
    record_path: &Path,
) -> Result<IssueDetectionAggregate, IssueDetectionAggregateError> {
    let identity = resolve_protocol_run_identity(record_path)?;
    let mut artifacts = list_protocol_artifacts(record_path)?
        .into_iter()
        .filter(|entry| entry.stored.procedure_name == INTERVENTION_ISSUE_DETECTION_PROCEDURE)
        .collect::<Vec<_>>();
    if artifacts.is_empty() {
        return Err(IssueDetectionAggregateError::MissingArtifact {
            record_path: record_path.to_path_buf(),
        });
    }
    artifacts.sort_by_key(|entry| std::cmp::Reverse(entry.stored.created_at_ms));

    let mut last_error = None;
    for artifact in artifacts {
        let input: IssueDetectionArtifactInput =
            match serde_json::from_value(artifact.stored.input.clone()) {
                Ok(input) => input,
                Err(source) => {
                    last_error = Some(IssueDetectionAggregateError::DeserializeInput {
                        path: artifact.path.clone(),
                        detail: source.to_string(),
                    });
                    continue;
                }
            };
        let output: IssueDetectionOutput =
            match serde_json::from_value(artifact.stored.output.clone()) {
                Ok(output) => output,
                Err(source) => {
                    last_error = Some(IssueDetectionAggregateError::DeserializeOutput {
                        path: artifact.path.clone(),
                        detail: source.to_string(),
                    });
                    continue;
                }
            };
        let primary_issue = select_primary_issue(&output);

        return Ok(IssueDetectionAggregate {
            run: IssueRunIdentity {
                record_path: identity.record_path,
                run_dir: identity.run_dir,
                run_id: identity.run_id,
                subject_id: identity.subject_id,
            },
            artifact: IssueArtifactRef {
                path: artifact.path,
                created_at_ms: artifact.stored.created_at_ms,
                procedure_name: artifact.stored.procedure_name,
            },
            input,
            output,
            primary_issue,
        });
    }

    Err(
        last_error.unwrap_or_else(|| IssueDetectionAggregateError::MissingArtifact {
            record_path: record_path.to_path_buf(),
        }),
    )
}
