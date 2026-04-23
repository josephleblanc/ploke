use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::intervention::execute::execute_tool_text_intervention;
use crate::intervention::spec::{
    InterventionApplyInput, InterventionApplyOutput, InterventionExecutionInput,
    InterventionSpecError, TreatmentStateRef,
};

pub const INTERVENTION_APPLY_PROCEDURE: &str = "intervention_apply";

fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn read_target(path: &Path) -> Result<String, InterventionSpecError> {
    fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            InterventionSpecError::MissingTarget(path.to_path_buf())
        } else {
            InterventionSpecError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })
}

pub fn execute_intervention_apply(
    input: &InterventionApplyInput,
) -> Result<InterventionApplyOutput, InterventionSpecError> {
    let absolute_path = input.repo_root.join(&input.target_relpath);
    let before = read_target(&absolute_path)?;
    if before != input.expected_source_content {
        return Err(InterventionSpecError::SourceContentMismatch {
            target: input.target_relpath.display().to_string(),
        });
    }

    let execution = execute_tool_text_intervention(&InterventionExecutionInput {
        repo_root: input.repo_root.clone(),
        spec: input.candidate.spec.clone(),
    })?;
    let after = read_target(&absolute_path)?;
    let apply_id = format!("apply-{}", Uuid::new_v4());

    Ok(InterventionApplyOutput {
        treatment_state: TreatmentStateRef {
            source_state_id: input.source_state_id.clone(),
            apply_id,
        },
        candidate_id: input.candidate.candidate_id.clone(),
        target_relpath: input.target_relpath.clone(),
        absolute_path,
        changed: execution.changed,
        source_content_hash: sha256_hex(&before),
        applied_content_hash: sha256_hex(&after),
        validation: execution.validation,
    })
}
