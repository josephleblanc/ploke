use std::fs;
use std::path::{Path, PathBuf};

use crate::intervention::spec::{
    AppliedEdit, ArtifactEdit, InterventionExecutionInput, InterventionExecutionOutput,
    InterventionSpec, InterventionSpecError, ValidationResult,
};

struct MaterializedEdit {
    pub target_relpath: PathBuf,
    pub new_text: String,
}

struct StagedEdit {
    pub absolute_path: PathBuf,
    pub original_text: String,
    pub new_text: String,
}

trait InterventionMaterializer {
    fn materialize(
        &self,
        repo_root: &Path,
        spec: &InterventionSpec,
    ) -> Result<MaterializedEdit, InterventionSpecError>;
}

trait InterventionStager {
    fn stage(
        &self,
        repo_root: &Path,
        materialized: MaterializedEdit,
    ) -> Result<StagedEdit, InterventionSpecError>;
}

trait InterventionApplier {
    fn apply(&self, staged: &StagedEdit) -> Result<AppliedEdit, InterventionSpecError>;
}

trait InterventionValidator {
    fn validate(
        &self,
        repo_root: &Path,
        spec: &InterventionSpec,
        applied: &AppliedEdit,
    ) -> Result<ValidationResult, InterventionSpecError>;
}

#[derive(Debug, Default, Clone, Copy)]
struct ToolTextInterventionAdapter;

impl ToolTextInterventionAdapter {
    fn ensure_supported_spec(spec: &InterventionSpec) -> Result<PathBuf, InterventionSpecError> {
        match spec {
            InterventionSpec::ToolGuidanceMutation { .. } => {
                Ok(spec.target_relpath().to_path_buf())
            }
            InterventionSpec::PolicyConfigMutation { .. } => Err(
                InterventionSpecError::Unimplemented("policy/config intervention adapter"),
            ),
        }
    }
}

impl InterventionMaterializer for ToolTextInterventionAdapter {
    fn materialize(
        &self,
        repo_root: &Path,
        spec: &InterventionSpec,
    ) -> Result<MaterializedEdit, InterventionSpecError> {
        let target_relpath = Self::ensure_supported_spec(spec)?;
        let absolute_path = repo_root.join(&target_relpath);
        let original_text = fs::read_to_string(&absolute_path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                InterventionSpecError::MissingTarget(absolute_path.clone())
            } else {
                InterventionSpecError::Io {
                    path: absolute_path.clone(),
                    source,
                }
            }
        })?;

        let new_text = match spec.edit() {
            ArtifactEdit::ReplaceWholeText { new_text } => new_text.clone(),
            ArtifactEdit::AppendText { text } => {
                let mut updated = original_text.clone();
                updated.push_str(text);
                updated
            }
            ArtifactEdit::ReplaceSection {
                start_marker,
                end_marker,
                replacement,
            } => {
                let Some(start) = original_text.find(start_marker) else {
                    return Err(InterventionSpecError::ReplaceSectionMarkersMissing {
                        target: target_relpath.display().to_string(),
                    });
                };
                let search_from = start + start_marker.len();
                let Some(relative_end) = original_text[search_from..].find(end_marker) else {
                    return Err(InterventionSpecError::ReplaceSectionMarkersMissing {
                        target: target_relpath.display().to_string(),
                    });
                };
                let end = search_from + relative_end;
                let mut updated = String::new();
                updated.push_str(&original_text[..search_from]);
                updated.push_str(replacement);
                updated.push_str(&original_text[end..]);
                updated
            }
        };

        Ok(MaterializedEdit {
            target_relpath,
            new_text,
        })
    }
}

impl InterventionStager for ToolTextInterventionAdapter {
    fn stage(
        &self,
        repo_root: &Path,
        materialized: MaterializedEdit,
    ) -> Result<StagedEdit, InterventionSpecError> {
        let absolute_path = repo_root.join(&materialized.target_relpath);
        let original_text =
            fs::read_to_string(&absolute_path).map_err(|source| InterventionSpecError::Io {
                path: absolute_path.clone(),
                source,
            })?;
        Ok(StagedEdit {
            absolute_path,
            original_text,
            new_text: materialized.new_text,
        })
    }
}

impl InterventionApplier for ToolTextInterventionAdapter {
    fn apply(&self, staged: &StagedEdit) -> Result<AppliedEdit, InterventionSpecError> {
        fs::write(&staged.absolute_path, &staged.new_text).map_err(|source| {
            InterventionSpecError::Io {
                path: staged.absolute_path.clone(),
                source,
            }
        })?;
        Ok(AppliedEdit {
            absolute_path: staged.absolute_path.clone(),
            changed: staged.original_text != staged.new_text,
        })
    }
}

impl InterventionValidator for ToolTextInterventionAdapter {
    fn validate(
        &self,
        repo_root: &Path,
        spec: &InterventionSpec,
        applied: &AppliedEdit,
    ) -> Result<ValidationResult, InterventionSpecError> {
        let mut checks = Vec::new();
        let target_relpath = spec.target_relpath();

        let allowed = spec
            .validation_policy()
            .allowed_relpaths
            .iter()
            .any(|path| path.as_path() == target_relpath);
        if !allowed {
            return Err(InterventionSpecError::TargetNotAllowed(
                target_relpath.display().to_string(),
            ));
        }
        checks.push("target_allowed".to_string());

        let absolute_path = repo_root.join(target_relpath);
        if spec.validation_policy().require_target_exists && !absolute_path.exists() {
            return Err(InterventionSpecError::MissingTarget(absolute_path));
        }
        if spec.validation_policy().require_target_exists {
            checks.push("target_exists".to_string());
        }

        let content = fs::read_to_string(&applied.absolute_path).map_err(|source| {
            InterventionSpecError::Io {
                path: applied.absolute_path.clone(),
                source,
            }
        })?;
        if spec.validation_policy().require_utf8 {
            checks.push("utf8".to_string());
        }
        if spec.validation_policy().require_nonempty_result && content.trim().is_empty() {
            return Err(InterventionSpecError::ValidationFailed(
                "target content is empty after apply".to_string(),
            ));
        }
        if spec.validation_policy().require_nonempty_result {
            checks.push("nonempty_result".to_string());
        }
        if spec.validation_policy().require_content_change && !applied.changed {
            return Err(InterventionSpecError::ValidationFailed(
                "intervention did not change target content".to_string(),
            ));
        }
        if spec.validation_policy().require_content_change {
            checks.push("content_changed".to_string());
        }
        for marker in &spec.validation_policy().require_markers_after_apply {
            if !content.contains(marker) {
                return Err(InterventionSpecError::ValidationFailed(format!(
                    "required marker missing after apply: {marker}"
                )));
            }
        }
        if !spec
            .validation_policy()
            .require_markers_after_apply
            .is_empty()
        {
            checks.push("required_markers_present".to_string());
        }
        if spec.validation_policy().require_cargo_check {
            return Err(InterventionSpecError::Unimplemented(
                "cargo-check validation backend",
            ));
        }

        Ok(ValidationResult { ok: true, checks })
    }
}

pub fn execute_tool_text_intervention(
    input: &InterventionExecutionInput,
) -> Result<InterventionExecutionOutput, InterventionSpecError> {
    let adapter = ToolTextInterventionAdapter;
    let materialized = adapter.materialize(&input.repo_root, &input.spec)?;
    let staged = adapter.stage(&input.repo_root, materialized)?;
    let applied = adapter.apply(&staged)?;
    let validation = adapter.validate(&input.repo_root, &input.spec, &applied)?;

    Ok(InterventionExecutionOutput {
        spec_id: input.spec.spec_id().to_string(),
        kind: input.spec.kind(),
        target_relpath: input.spec.target_relpath().to_path_buf(),
        absolute_path: applied.absolute_path.clone(),
        changed: applied.changed,
        applied,
        validation,
    })
}
