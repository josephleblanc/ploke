use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::harness_request::{
    EvidenceRootKind, EvidenceRootLocation, ParentNodeRef, PublishedBroadHarnessRequest,
    RequestAdmissionBinding, SubmissionAuthorityBoundary,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedBroadHarnessResult {
    pub(crate) schema: SubmittedBroadHarnessResultSchema,
    pub(crate) request: SubmittedRequestBinding,
    pub(crate) candidate: SubmittedBroadHarnessCandidate,
    pub(crate) return_evidence: SubmittedHarnessReturnEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubmittedBroadHarnessResultSchema {
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedRequestBinding {
    pub(crate) request_id: String,
    pub(crate) request_hash: String,
    pub(crate) parent_node_id: ParentNodeRef,
    pub(crate) workspace_path: PathBuf,
    pub(crate) submitted_result_path: PathBuf,
    pub(crate) admission_binding: RequestAdmissionBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedBroadHarnessCandidate {
    pub(crate) workspace_path: PathBuf,
    pub(crate) submitted_result_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedHarnessReturnEvidence {
    pub(crate) authority_boundary: SubmissionAuthorityBoundary,
    pub(crate) change_summary: SubmittedChangeSummary,
    pub(crate) guiding_evidence: Vec<SubmittedEvidenceCitation>,
    pub(crate) rationale: SubmittedImprovementRationale,
    pub(crate) checks: Vec<SubmittedCheckRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedChangeSummary {
    pub(crate) changed_files: Vec<SubmittedFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedFileChange {
    pub(crate) workspace_relpath: PathBuf,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedEvidenceCitation {
    pub(crate) kind: EvidenceRootKind,
    pub(crate) location: EvidenceRootLocation,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedImprovementRationale {
    pub(crate) hypothesis: String,
    pub(crate) expected_descendant_effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedCheckRecommendation {
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) success_signal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmittedBroadHarnessResultError {
    RequestIdMismatch {
        expected: String,
        actual: String,
    },
    RequestHashMismatch {
        expected: String,
        actual: String,
    },
    ParentNodeMismatch {
        expected: ParentNodeRef,
        actual: ParentNodeRef,
    },
    WorkspacePathMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    SubmittedResultPathMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    RequestAdmissionBindingMismatch {
        expected: RequestAdmissionBinding,
        actual: RequestAdmissionBinding,
    },
    ChangedFileOutsideWorkspace {
        workspace_relpath: PathBuf,
    },
}

impl SubmittedBroadHarnessResult {
    pub(crate) fn bind(
        published: &PublishedBroadHarnessRequest,
        return_evidence: SubmittedHarnessReturnEvidence,
    ) -> Result<Self, SubmittedBroadHarnessResultError> {
        let submitted = Self {
            schema: SubmittedBroadHarnessResultSchema::V1,
            request: SubmittedRequestBinding::from_published(published),
            candidate: SubmittedBroadHarnessCandidate {
                workspace_path: published.workspace_path().to_path_buf(),
                submitted_result_path: published.submitted_result_path().to_path_buf(),
            },
            return_evidence,
        };
        submitted.verify_request(published)?;
        Ok(submitted)
    }

    pub(crate) fn verify_request(
        &self,
        published: &PublishedBroadHarnessRequest,
    ) -> Result<(), SubmittedBroadHarnessResultError> {
        let expected = SubmittedRequestBinding::from_published(published);
        if self.request.request_id != expected.request_id {
            return Err(SubmittedBroadHarnessResultError::RequestIdMismatch {
                expected: expected.request_id,
                actual: self.request.request_id.clone(),
            });
        }
        if self.request.request_hash != expected.request_hash {
            return Err(SubmittedBroadHarnessResultError::RequestHashMismatch {
                expected: expected.request_hash,
                actual: self.request.request_hash.clone(),
            });
        }
        if self.request.parent_node_id != expected.parent_node_id {
            return Err(SubmittedBroadHarnessResultError::ParentNodeMismatch {
                expected: expected.parent_node_id,
                actual: self.request.parent_node_id.clone(),
            });
        }
        if self.request.workspace_path != expected.workspace_path {
            return Err(SubmittedBroadHarnessResultError::WorkspacePathMismatch {
                expected: expected.workspace_path,
                actual: self.request.workspace_path.clone(),
            });
        }
        if self.request.submitted_result_path != expected.submitted_result_path {
            return Err(
                SubmittedBroadHarnessResultError::SubmittedResultPathMismatch {
                    expected: expected.submitted_result_path,
                    actual: self.request.submitted_result_path.clone(),
                },
            );
        }
        if self.request.admission_binding != expected.admission_binding {
            return Err(
                SubmittedBroadHarnessResultError::RequestAdmissionBindingMismatch {
                    expected: expected.admission_binding,
                    actual: self.request.admission_binding.clone(),
                },
            );
        }
        if self.candidate.workspace_path != expected.workspace_path {
            return Err(SubmittedBroadHarnessResultError::WorkspacePathMismatch {
                expected: expected.workspace_path,
                actual: self.candidate.workspace_path.clone(),
            });
        }
        if self.candidate.submitted_result_path != expected.submitted_result_path {
            return Err(
                SubmittedBroadHarnessResultError::SubmittedResultPathMismatch {
                    expected: expected.submitted_result_path,
                    actual: self.candidate.submitted_result_path.clone(),
                },
            );
        }
        self.verify_changed_files()
    }

    pub(crate) fn request(&self) -> &SubmittedRequestBinding {
        &self.request
    }

    pub(crate) fn candidate(&self) -> &SubmittedBroadHarnessCandidate {
        &self.candidate
    }

    fn verify_changed_files(&self) -> Result<(), SubmittedBroadHarnessResultError> {
        for changed_file in &self.return_evidence.change_summary.changed_files {
            let relpath = &changed_file.workspace_relpath;
            if relpath.is_absolute()
                || relpath
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(
                    SubmittedBroadHarnessResultError::ChangedFileOutsideWorkspace {
                        workspace_relpath: relpath.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

impl SubmittedRequestBinding {
    fn from_published(published: &PublishedBroadHarnessRequest) -> Self {
        Self {
            request_id: published.request_id().to_string(),
            request_hash: published.request_hash().to_string(),
            parent_node_id: published.request().parent_node_id.clone(),
            workspace_path: published.workspace_path().to_path_buf(),
            submitted_result_path: published.submitted_result_path().to_path_buf(),
            admission_binding: published.admission_binding().clone(),
        }
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub(crate) fn submitted_result_path(&self) -> &Path {
        &self.submitted_result_path
    }

    pub(crate) fn admission_binding(&self) -> &RequestAdmissionBinding {
        &self.admission_binding
    }
}

impl SubmittedBroadHarnessCandidate {
    pub(crate) fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub(crate) fn submitted_result_path(&self) -> &Path {
        &self.submitted_result_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::prototype1_state::backend::EditSurfaceAdmission;
    use crate::cli::prototype1_state::edit_surface::harness_request::HarnessChildBudget;
    use std::fs;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        prototype_root: PathBuf,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let prototype_root = temp.path().join("prototype1");
            let prompt_dir = prototype_root.join("messages/edit-harness-request");
            let result_dir = prototype_root.join("messages/edit-harness-result");
            fs::create_dir_all(&prompt_dir).expect("create prompt dir");
            fs::create_dir_all(&result_dir).expect("create result dir");
            Self {
                _temp: temp,
                prototype_root,
                request_path: prompt_dir.join("parent-node-7.json"),
                prompt_path: prompt_dir.join("parent-node-7.md"),
                submitted_result_path: result_dir.join("parent-node-7.json"),
            }
        }

        fn published_request(&self) -> PublishedBroadHarnessRequest {
            let admission_binding = self.request_admission_binding(
                "artifact:/tmp/ploke-workspace",
                "workspace except ploke-eval",
            );
            PublishedBroadHarnessRequest::prototype1_workspace(
                "parent-node-7".to_string(),
                PathBuf::from("/tmp/ploke-workspace"),
                HarnessChildBudget {
                    min_children: 1,
                    max_children: 3,
                },
                &self.prototype_root,
                self.request_path.clone(),
                self.prompt_path.clone(),
                self.submitted_result_path.clone(),
                admission_binding,
            )
        }

        fn bound_published_request(&self) -> PublishedBroadHarnessRequest {
            self.published_request().with_admission_binding(
                self.request_admission_binding("artifact:broad-base", "policy:broad-boundary"),
            )
        }

        fn request_admission_binding(
            &self,
            artifact_id: &str,
            policy_id: &str,
        ) -> RequestAdmissionBinding {
            let _ = self;
            let coordinate = crate::loop_graph::Coordinate {
                runtime_id: crate::loop_graph::RuntimeId::new(),
                target: crate::loop_graph::OperationTarget::Artifact {
                    artifact_id: crate::loop_graph::ArtifactId::new(artifact_id),
                },
            };
            let admission = EditSurfaceAdmission::new(
                coordinate,
                crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
                    policy_id,
                ),
            );
            RequestAdmissionBinding::from_admission(&admission)
                .expect("request admission binding should project from admission")
        }
    }

    #[test]
    fn submitted_result_round_trips_with_request_binding() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");

        let request_json =
            serde_json::to_string(&published).expect("serialize published broad harness request");
        let submitted_json =
            serde_json::to_string(&submitted).expect("serialize submitted broad harness result");

        let decoded_request = serde_json::from_str::<PublishedBroadHarnessRequest>(&request_json)
            .expect("deserialize published broad harness request");
        let decoded_submitted =
            serde_json::from_str::<SubmittedBroadHarnessResult>(&submitted_json)
                .expect("deserialize submitted broad harness result");

        assert_eq!(decoded_request, published);
        assert_eq!(decoded_submitted, submitted);
        assert_eq!(
            decoded_submitted.request().workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7")
                .as_path()
        );
        assert_eq!(
            decoded_submitted.request().submitted_result_path(),
            fixture.submitted_result_path.as_path()
        );
        assert_eq!(
            decoded_submitted.candidate().workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7")
                .as_path()
        );
        assert_eq!(
            decoded_submitted.candidate().submitted_result_path(),
            fixture.submitted_result_path.as_path()
        );
        assert_eq!(
            decoded_submitted.return_evidence.authority_boundary,
            SubmissionAuthorityBoundary::submitted_evidence_only()
        );
        assert_eq!(
            decoded_submitted
                .return_evidence
                .change_summary
                .changed_files[0]
                .workspace_relpath,
            Path::new("crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs")
        );
        decoded_submitted
            .verify_request(&published)
            .expect("submitted result matches published request");
    }

    #[test]
    fn submitted_result_rejects_request_hash_mismatch() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let mut submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");
        submitted.request.request_hash = "tampered-request-hash".to_string();

        let err = submitted
            .verify_request(&published)
            .expect_err("tampered request hash should be rejected");

        assert!(matches!(
            err,
            SubmittedBroadHarnessResultError::RequestHashMismatch { expected, actual }
                if expected == published.request_hash() && actual == "tampered-request-hash"
        ));
    }

    #[test]
    fn submitted_result_round_trips_request_admission_binding() {
        let fixture = Fixture::new();
        let published = fixture.bound_published_request();
        let submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");

        let submitted_json =
            serde_json::to_string(&submitted).expect("serialize submitted broad harness result");
        let decoded_submitted =
            serde_json::from_str::<SubmittedBroadHarnessResult>(&submitted_json)
                .expect("deserialize submitted broad harness result");

        assert_eq!(
            decoded_submitted.request().admission_binding(),
            published.admission_binding()
        );
        assert_eq!(
            decoded_submitted
                .request()
                .admission_binding()
                .target_artifact_id(),
            &crate::loop_graph::ArtifactId::new("artifact:broad-base")
        );
    }

    #[test]
    fn submitted_result_rejects_request_admission_binding_mismatch() {
        let fixture = Fixture::new();
        let published = fixture.bound_published_request();
        let mut submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");
        let tampered_binding = {
            let coordinate = crate::loop_graph::Coordinate {
                runtime_id: crate::loop_graph::RuntimeId::new(),
                target: crate::loop_graph::OperationTarget::Artifact {
                    artifact_id: crate::loop_graph::ArtifactId::new("artifact:other-base"),
                },
            };
            RequestAdmissionBinding::new(
                coordinate,
                crate::loop_graph::ArtifactId::new("artifact:other-base"),
                "policy:broad-boundary",
            )
            .expect("tampered binding should still construct")
        };
        submitted.request.admission_binding = tampered_binding.clone();

        let err = submitted
            .verify_request(&published)
            .expect_err("tampered request admission binding should be rejected");

        assert!(matches!(
            err,
            SubmittedBroadHarnessResultError::RequestAdmissionBindingMismatch { expected, actual }
                if expected == *published.admission_binding()
                    && actual == tampered_binding
        ));
    }

    #[test]
    fn submitted_result_rejects_missing_request_admission_binding() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");
        let mut json =
            serde_json::to_value(&submitted).expect("serialize submitted broad harness result");

        json.get_mut("request")
            .expect("submitted result has a request object")
            .as_object_mut()
            .expect("submitted request is a JSON object")
            .remove("admission_binding");

        assert!(
            serde_json::from_value::<SubmittedBroadHarnessResult>(json).is_err(),
            "missing authority binding should not deserialize"
        );
    }

    #[test]
    fn submitted_result_rejects_workspace_escape_in_changed_files() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let mut submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");
        submitted.return_evidence.change_summary.changed_files[0].workspace_relpath =
            PathBuf::from("../backend.rs");

        let err = submitted
            .verify_request(&published)
            .expect_err("parent-dir changed file should be rejected");

        assert!(matches!(
            err,
            SubmittedBroadHarnessResultError::ChangedFileOutsideWorkspace { workspace_relpath }
                if workspace_relpath == Path::new("../backend.rs")
        ));
    }

    #[test]
    fn submitted_result_binds_second_publication_identity() {
        let fixture = Fixture::new();
        let first = fixture.published_request();
        fs::write(first.prompt_path(), "published prompt").expect("write first prompt");
        let second = fixture.published_request();

        let submitted = SubmittedBroadHarnessResult::bind(&second, sample_return_evidence())
            .expect("bind second submitted broad harness result");

        assert_eq!(submitted.request().request_id, second.request_id());
        assert_eq!(
            submitted.request().workspace_path(),
            second.workspace_path()
        );
        assert_eq!(
            submitted.request().submitted_result_path(),
            second.submitted_result_path()
        );
        assert_eq!(
            submitted.candidate().workspace_path(),
            second.workspace_path()
        );
        assert_eq!(
            submitted.candidate().submitted_result_path(),
            second.submitted_result_path()
        );
    }

    fn sample_return_evidence() -> SubmittedHarnessReturnEvidence {
        SubmittedHarnessReturnEvidence {
            authority_boundary: SubmissionAuthorityBoundary::submitted_evidence_only(),
            change_summary: SubmittedChangeSummary {
                changed_files: vec![SubmittedFileChange {
                    workspace_relpath: PathBuf::from(
                        "crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_request.rs",
                    ),
                    summary: "Replace child-plan output with a submitted-result contract."
                        .to_string(),
                }],
            },
            guiding_evidence: vec![SubmittedEvidenceCitation {
                kind: EvidenceRootKind::HistoryBlocks,
                location: EvidenceRootLocation::Directory {
                    path: PathBuf::from("/tmp/prototype1/history/blocks"),
                },
                summary: "Recent broad-harness failures point at authority confusion around child-plan output."
                    .to_string(),
            }],
            rationale: SubmittedImprovementRationale {
                hypothesis: "Separating submitted evidence from ChildPlan authority keeps admission in ploke-eval."
                    .to_string(),
                expected_descendant_effect:
                    "Future broad harness descendants can be checked and admitted without treating harness output as authority."
                        .to_string(),
            },
            checks: vec![SubmittedCheckRecommendation {
                label: "edit_surface tests".to_string(),
                command: "cargo test -p ploke-eval edit_surface".to_string(),
                success_signal: "Typed request/result tests pass and no path binds a submitted result as ChildPlan authority."
                    .to_string(),
            }],
        }
    }
}
