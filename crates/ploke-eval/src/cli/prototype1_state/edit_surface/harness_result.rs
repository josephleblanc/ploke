use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::harness_request::{
    EvidenceRootKind, EvidenceRootLocation, ParentNodeRef, PublishedBroadHarnessRequest,
    RequestAdmissionBinding, SubmissionAuthorityBoundary, contract,
};
use crate::cli::prototype1_state::history::ArtifactSurface;
use crate::loop_graph::ArtifactId;

pub(crate) mod transaction {
    use std::{
        marker::PhantomData,
        path::{Path, PathBuf},
    };

    use serde::{Deserialize, Serialize};

    use super::{ArtifactId, ArtifactSurface, RequestAdmissionBinding};
    use crate::cli::prototype1_state::edit_surface::harness_request::{child, request};

    pub(crate) mod state {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Admitted {}
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(bound = "")]
    pub(crate) struct Transaction<S> {
        pub(crate) schema_version: u32,
        request: request::Reference<request::Broad, request::Published>,
        admission: Admission,
        workspace: Workspace,
        artifact: Derivation,
        changes: ChangeSet,
        submission: Submission,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        executor: Option<Executor>,
        #[serde(skip)]
        _state: PhantomData<fn() -> S>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Admission {
        binding: RequestAdmissionBinding,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Workspace {
        source_root: PathBuf,
        candidate_root: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_head: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Derivation {
        base_artifact_id: ArtifactId,
        derived_artifact_id: ArtifactId,
        surface: ArtifactSurface,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct ChangeSet {
        paths: Vec<PathBuf>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Submission {
        result_path: PathBuf,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Executor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record_path: Option<PathBuf>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum Error {
        EmptyChangeSet,
        ChangedPathOutsideWorkspace { path: PathBuf },
    }

    impl Admission {
        pub(crate) fn new(binding: RequestAdmissionBinding) -> Self {
            Self { binding }
        }

        pub(crate) fn binding(&self) -> &RequestAdmissionBinding {
            &self.binding
        }
    }

    impl Workspace {
        pub(crate) fn new(
            source_root: PathBuf,
            candidate_root: PathBuf,
            base_head: Option<String>,
        ) -> Self {
            Self {
                source_root,
                candidate_root,
                base_head,
            }
        }

        pub(crate) fn source_root(&self) -> &Path {
            &self.source_root
        }

        pub(crate) fn candidate_root(&self) -> &Path {
            &self.candidate_root
        }

        pub(crate) fn base_head(&self) -> Option<&str> {
            self.base_head.as_deref()
        }
    }

    impl Derivation {
        pub(crate) fn new(
            base_artifact_id: ArtifactId,
            derived_artifact_id: ArtifactId,
            surface: ArtifactSurface,
        ) -> Self {
            Self {
                base_artifact_id,
                derived_artifact_id,
                surface,
            }
        }

        pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
            &self.base_artifact_id
        }

        pub(crate) fn derived_artifact_id(&self) -> &ArtifactId {
            &self.derived_artifact_id
        }

        pub(crate) fn surface(&self) -> &ArtifactSurface {
            &self.surface
        }
    }

    impl ChangeSet {
        pub(crate) fn new(paths: Vec<PathBuf>) -> Result<Self, Error> {
            if paths.is_empty() {
                return Err(Error::EmptyChangeSet);
            }
            if let Some(path) = paths.iter().find(|path| !is_normal_relative(path)) {
                return Err(Error::ChangedPathOutsideWorkspace { path: path.clone() });
            }
            Ok(Self { paths })
        }

        pub(crate) fn paths(&self) -> &[PathBuf] {
            &self.paths
        }
    }

    impl Submission {
        pub(crate) fn new(result_path: PathBuf) -> Self {
            Self { result_path }
        }

        pub(crate) fn result_path(&self) -> &Path {
            &self.result_path
        }
    }

    impl Executor {
        pub(crate) fn new(
            run_id: Option<String>,
            attempt_id: Option<String>,
            record_path: Option<PathBuf>,
        ) -> Self {
            Self {
                run_id,
                attempt_id,
                record_path,
            }
        }

        pub(crate) fn run_id(&self) -> Option<&str> {
            self.run_id.as_deref()
        }

        pub(crate) fn attempt_id(&self) -> Option<&str> {
            self.attempt_id.as_deref()
        }

        pub(crate) fn record_path(&self) -> Option<&Path> {
            self.record_path.as_deref()
        }
    }

    impl Transaction<state::Admitted> {
        pub(crate) fn admit(
            request: request::Reference<request::Broad, request::Published>,
            admission: Admission,
            workspace: Workspace,
            artifact: Derivation,
            changes: ChangeSet,
            submission: Submission,
            executor: Option<Executor>,
        ) -> Self {
            Self {
                schema_version: 1,
                request,
                admission,
                workspace,
                artifact,
                changes,
                submission,
                executor,
                _state: PhantomData,
            }
        }

        pub(crate) fn request(&self) -> &request::Reference<request::Broad, request::Published> {
            &self.request
        }

        pub(crate) fn admission(&self) -> &Admission {
            &self.admission
        }

        pub(crate) fn workspace(&self) -> &Workspace {
            &self.workspace
        }

        pub(crate) fn artifact(&self) -> &Derivation {
            &self.artifact
        }

        pub(crate) fn changes(&self) -> &ChangeSet {
            &self.changes
        }

        pub(crate) fn submission(&self) -> &Submission {
            &self.submission
        }

        pub(crate) fn executor(&self) -> Option<&Executor> {
            self.executor.as_ref()
        }

        pub(crate) fn with_executor(mut self, executor: Executor) -> Self {
            self.executor = Some(executor);
            self
        }

        pub(crate) fn child_evidence(&self) -> child::Evidence {
            child::Evidence::admitted(
                self.request.clone(),
                self.admission.binding.clone(),
                self.submission.result_path.clone(),
                self.changes.paths.clone(),
                self.artifact.surface.clone(),
            )
            .with_workspace(child::WorkspaceEvidence::new(
                self.workspace.source_root.clone(),
                self.workspace.candidate_root.clone(),
                self.workspace.base_head.clone(),
            ))
            .with_artifact(child::ArtifactEvidence::new(
                self.artifact.base_artifact_id.clone(),
                self.artifact.derived_artifact_id.clone(),
            ))
            .with_executor(self.executor.as_ref().map(|executor| {
                child::ExecutorEvidence::new(
                    executor.run_id.clone(),
                    executor.attempt_id.clone(),
                    executor.record_path.clone(),
                )
            }))
        }
    }

    fn is_normal_relative(path: &Path) -> bool {
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmittedBroadHarnessResult {
    pub(crate) schema: SubmittedBroadHarnessResultSchema,
    pub(crate) request: SubmittedRequestBinding,
    pub(crate) candidate: SubmittedBroadHarnessCandidate,
    #[serde(default = "contract::Bundle::empty")]
    pub(crate) contract: contract::Bundle,
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
            contract: published.request().contract.clone(),
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
    use crate::cli::prototype1_state::history::ArtifactSurface;
    use crate::loop_graph::ArtifactId;
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
        assert_eq!(decoded_submitted.contract, published.request().contract);
        assert_eq!(decoded_submitted.contract.validation.commands.len(), 2);
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
    fn admitted_transaction_round_trips_with_structural_axes() {
        let fixture = Fixture::new();
        let published = fixture.bound_published_request();
        let transaction = sample_transaction(&published);

        let json = serde_json::to_string(&transaction).expect("serialize transaction");
        let decoded =
            serde_json::from_str::<transaction::Transaction<transaction::state::Admitted>>(&json)
                .expect("deserialize transaction");

        assert_eq!(decoded, transaction);
        assert_eq!(decoded.request(), &published.reference());
        assert_eq!(decoded.admission().binding(), published.admission_binding());
        assert_eq!(
            decoded.workspace().source_root(),
            Path::new("/tmp/ploke-workspace")
        );
        assert_eq!(decoded.workspace().base_head(), Some("git:base-head"));
        assert_eq!(
            decoded.artifact().base_artifact_id(),
            &ArtifactId::new("artifact:broad-base")
        );
        assert_eq!(
            decoded.artifact().derived_artifact_id(),
            &ArtifactId::new("artifact:broad-derived")
        );
        assert_eq!(
            decoded.changes().paths(),
            &[
                PathBuf::from("src/first.rs"),
                PathBuf::from("src/second.rs")
            ]
        );
        assert_eq!(
            decoded.submission().result_path(),
            published.submitted_result_path()
        );
        assert_eq!(
            decoded.executor().and_then(|executor| executor.run_id()),
            Some("run-1")
        );
    }

    #[test]
    fn admitted_transaction_projects_child_evidence() {
        let fixture = Fixture::new();
        let published = fixture.bound_published_request();
        let transaction = sample_transaction(&published);

        let evidence = transaction.child_evidence();

        assert_eq!(evidence.request(), &published.reference());
        assert_eq!(evidence.admission_binding(), published.admission_binding());
        assert_eq!(
            evidence.submitted_result_path(),
            published.submitted_result_path()
        );
        assert_eq!(
            evidence.changed_paths(),
            &[
                PathBuf::from("src/first.rs"),
                PathBuf::from("src/second.rs")
            ]
        );
        assert_eq!(
            evidence.artifact_surface(),
            transaction.artifact().surface()
        );
        let workspace = evidence.workspace().expect("workspace projection");
        assert_eq!(workspace.source_root, PathBuf::from("/tmp/ploke-workspace"));
        assert_eq!(workspace.candidate_root, published.workspace_path());
        assert_eq!(workspace.base_head.as_deref(), Some("git:base-head"));
        let artifact = evidence.artifact().expect("artifact projection");
        assert_eq!(
            artifact.base_artifact_id,
            ArtifactId::new("artifact:broad-base")
        );
        assert_eq!(
            artifact.derived_artifact_id,
            ArtifactId::new("artifact:broad-derived")
        );
        let executor = evidence.executor().expect("executor projection");
        assert_eq!(executor.run_id.as_deref(), Some("run-1"));
        assert_eq!(executor.attempt_id.as_deref(), Some("attempt-1"));
        assert_eq!(
            executor.record_path.as_deref(),
            Some(Path::new("attempts/run-1.json"))
        );
    }

    #[test]
    fn submitted_result_alone_does_not_drive_child_projection() {
        let fixture = Fixture::new();
        let published = fixture.bound_published_request();
        let mut submitted = SubmittedBroadHarnessResult::bind(&published, sample_return_evidence())
            .expect("bind submitted broad harness result");
        submitted.return_evidence.change_summary.changed_files = vec![SubmittedFileChange {
            workspace_relpath: PathBuf::from("src/submitted-only.rs"),
            summary: "submitted summary is not admission".to_string(),
        }];
        submitted
            .verify_request(&published)
            .expect("submitted evidence still binds to request");

        let evidence = sample_transaction(&published).child_evidence();

        assert_eq!(
            evidence.changed_paths(),
            &[
                PathBuf::from("src/first.rs"),
                PathBuf::from("src/second.rs")
            ]
        );
        assert_ne!(
            evidence.changed_paths(),
            &[PathBuf::from("src/submitted-only.rs")]
        );
    }

    #[test]
    fn admitted_transaction_rejects_unadmitted_change_sets() {
        assert_eq!(
            transaction::ChangeSet::new(Vec::new()).expect_err("empty changes reject"),
            transaction::Error::EmptyChangeSet
        );
        assert_eq!(
            transaction::ChangeSet::new(vec![PathBuf::from("../outside.rs")])
                .expect_err("parent path rejects"),
            transaction::Error::ChangedPathOutsideWorkspace {
                path: PathBuf::from("../outside.rs")
            }
        );
        assert_eq!(
            transaction::ChangeSet::new(vec![PathBuf::from("/tmp/outside.rs")])
                .expect_err("absolute path rejects"),
            transaction::Error::ChangedPathOutsideWorkspace {
                path: PathBuf::from("/tmp/outside.rs")
            }
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

    fn sample_transaction(
        published: &PublishedBroadHarnessRequest,
    ) -> transaction::Transaction<transaction::state::Admitted> {
        transaction::Transaction::admit(
            published.reference(),
            transaction::Admission::new(published.admission_binding().clone()),
            transaction::Workspace::new(
                PathBuf::from("/tmp/ploke-workspace"),
                published.workspace_path().to_path_buf(),
                Some("git:base-head".to_string()),
            ),
            transaction::Derivation::new(
                ArtifactId::new("artifact:broad-base"),
                ArtifactId::new("artifact:broad-derived"),
                ArtifactSurface::test("broad-transaction"),
            ),
            transaction::ChangeSet::new(vec![
                PathBuf::from("src/first.rs"),
                PathBuf::from("src/second.rs"),
            ])
            .expect("valid transaction change set"),
            transaction::Submission::new(published.submitted_result_path().to_path_buf()),
            Some(transaction::Executor::new(
                Some("run-1".to_string()),
                Some("attempt-1".to_string()),
                Some(PathBuf::from("attempts/run-1.json")),
            )),
        )
    }
}
