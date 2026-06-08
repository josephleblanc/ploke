use std::{error::Error, fmt, marker::PhantomData};

use crate::intervention::{Prototype1NodeRecord, ResolvedTreatmentBranch};

use super::history::{ArtifactRef, ArtifactSurface, SubjectRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection<T> {
    selected: T,
    _target: PhantomData<fn() -> T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Artifact {
    node: Prototype1NodeRecord,
    candidate: SubjectRef,
    artifact_ref: ArtifactRef,
    artifact_surface: ArtifactSurface,
    resolved: ResolvedTreatmentBranch,
    source: Source,
    primary_runtime_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    CurrentGeneration,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactMismatch {
    field: &'static str,
    resolved: String,
    node: String,
}

impl Selection<Artifact> {
    pub(super) fn artifact(
        node: Prototype1NodeRecord,
        candidate: SubjectRef,
        resolved: ResolvedTreatmentBranch,
        artifact_surface: ArtifactSurface,
        source: Source,
        primary_runtime_id: Option<String>,
    ) -> Result<Self, ArtifactMismatch> {
        ArtifactMismatch::verify(&node, &resolved)?;
        let artifact_ref = artifact_ref(&node);
        Ok(Self {
            selected: Artifact {
                node,
                candidate,
                artifact_ref,
                artifact_surface,
                resolved,
                source,
                primary_runtime_id,
            },
            _target: PhantomData,
        })
    }

    #[cfg(test)]
    pub(crate) fn artifact_for_test(
        node: Prototype1NodeRecord,
        candidate: SubjectRef,
        resolved: ResolvedTreatmentBranch,
        artifact_surface: ArtifactSurface,
        source: Source,
        primary_runtime_id: Option<String>,
    ) -> Result<Self, ArtifactMismatch> {
        Self::artifact(
            node,
            candidate,
            resolved,
            artifact_surface,
            source,
            primary_runtime_id,
        )
    }

    pub(crate) fn selected(&self) -> &Artifact {
        &self.selected
    }
}

impl Artifact {
    pub(crate) fn node(&self) -> &Prototype1NodeRecord {
        &self.node
    }

    pub(crate) fn candidate(&self) -> &SubjectRef {
        &self.candidate
    }

    pub(crate) fn artifact_ref(&self) -> &ArtifactRef {
        &self.artifact_ref
    }

    pub(crate) fn artifact_surface(&self) -> &ArtifactSurface {
        &self.artifact_surface
    }

    #[cfg(test)]
    pub(crate) fn branch_id(&self) -> &str {
        &self.resolved.branch.branch_id
    }

    pub(crate) fn resolved(&self) -> &ResolvedTreatmentBranch {
        &self.resolved
    }

    pub(crate) fn source(&self) -> Source {
        self.source
    }

    pub(crate) fn primary_runtime_id(&self) -> Option<&str> {
        self.primary_runtime_id.as_deref()
    }
}

fn artifact_ref(node: &Prototype1NodeRecord) -> ArtifactRef {
    if let Some(artifact_id) = node.derived_artifact_id.as_ref() {
        return ArtifactRef::from_artifact_id(artifact_id.clone());
    }
    if let Some(artifact_id) = node.base_artifact_id.as_ref() {
        return ArtifactRef::from_artifact_id(artifact_id.clone());
    }
    ArtifactRef::from_branch_id(node.branch_id.clone())
}

impl ArtifactMismatch {
    fn verify(node: &Prototype1NodeRecord, resolved: &ResolvedTreatmentBranch) -> Result<(), Self> {
        Self::same("branch", &resolved.branch.branch_id, &node.branch_id)?;
        Self::same(
            "candidate",
            &resolved.branch.candidate_id,
            &node.candidate_id,
        )?;
        Self::same(
            "source state",
            &resolved.source_state_id,
            &node.source_state_id,
        )?;
        if resolved.target_relpath != node.target_relpath {
            return Err(Self {
                field: "target",
                resolved: resolved.target_relpath.display().to_string(),
                node: node.target_relpath.display().to_string(),
            });
        }
        Ok(())
    }

    fn same(field: &'static str, resolved: &str, node: &str) -> Result<(), Self> {
        if resolved == node {
            return Ok(());
        }
        Err(Self {
            field,
            resolved: resolved.to_string(),
            node: node.to_string(),
        })
    }
}

impl fmt::Display for ArtifactMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selected Artifact {} mismatch: resolved={}, node_record={}",
            self.field, self.resolved, self.node
        )
    }
}

impl Error for ArtifactMismatch {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::intervention::{
        Prototype1NodeRecord, Prototype1NodeStatus, ResolvedTreatmentBranch, TreatmentBranchNode,
        TreatmentBranchStatus,
    };

    use super::*;

    fn node() -> Prototype1NodeRecord {
        Prototype1NodeRecord {
            schema_version: "test".to_string(),
            node_id: "node-1".to_string(),
            parent_node_id: Some("parent-1".to_string()),
            generation: 1,
            instance_id: "instance-1".to_string(),
            source_state_id: "source-1".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: "branch-1".to_string(),
            candidate_id: "candidate-1".to_string(),
            target_relpath: PathBuf::from("crates/ploke-core/tool_text/read_file.md"),
            node_dir: PathBuf::from("/tmp/node-1"),
            workspace_root: PathBuf::from("/tmp/workspace"),
            binary_path: PathBuf::from("/tmp/workspace/target/debug/ploke-eval"),
            runner_request_path: PathBuf::from("/tmp/node-1/runner-request.json"),
            runner_result_path: PathBuf::from("/tmp/node-1/runner-result.json"),
            status: Prototype1NodeStatus::Succeeded,
            created_at: "2026-04-26T00:00:00Z".to_string(),
            updated_at: "2026-04-26T00:00:00Z".to_string(),
        }
    }

    fn resolved_for(node: &Prototype1NodeRecord) -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            parent_branch_id: node.parent_branch_id.clone(),
            target_relpath: node.target_relpath.clone(),
            source_content: "old".to_string(),
            source_content_hash: "old-hash".to_string(),
            selected_branch_id: Some(node.branch_id.clone()),
            branch: TreatmentBranchNode {
                branch_id: node.branch_id.clone(),
                candidate_id: node.candidate_id.clone(),
                patch_id: None,
                branch_label: "candidate 1".to_string(),
                synthesized_spec_id: "spec-1".to_string(),
                proposed_content: "new".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: TreatmentBranchStatus::Selected,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
            },
        }
    }

    #[test]
    fn artifact_selection_rejects_resolved_candidate_mismatch() {
        let node = node();
        let mut resolved = resolved_for(&node);
        resolved.branch.candidate_id = "candidate-other".to_string();

        let err = Selection::artifact(
            node,
            SubjectRef::new("candidate:node-1:plan_index=0"),
            resolved,
            ArtifactSurface::test("node-1"),
            Source::History,
            Some("runtime:node-1".to_string()),
        )
        .expect_err("mismatched registry candidate must not construct an Artifact selection");

        assert!(
            err.to_string()
                .contains("selected Artifact candidate mismatch")
        );
    }
}
