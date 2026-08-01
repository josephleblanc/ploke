use std::{
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::cli::prototype1_state::{backend::EditSurfaceAdmission, history::ArtifactSurface};
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};

use super::{surface, surface_policy::SurfacePolicy};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BroadHarnessRequest {
    pub(crate) schema: BroadHarnessRequestSchema,
    pub(crate) parent_node_id: ParentNodeRef,
    pub(crate) workspace: HarnessWorkspace,
    pub(crate) edit_policy: SurfacePolicy,
    #[serde(default)]
    pub(crate) graph_restriction: GraphRestriction,
    pub(crate) child_budget: HarnessChildBudget,
    pub(crate) protected_core: ProtectedCorePointer,
    pub(crate) evaluation: EvaluationBrief,
    #[serde(default)]
    pub(crate) planning: PlanningGuidance,
    #[serde(default = "contract::Bundle::empty")]
    pub(crate) contract: contract::Bundle,
    pub(crate) evidence_roots: Vec<EvidenceRoot>,
    pub(crate) return_evidence: ReturnEvidenceContract,
    pub(crate) instructions: Vec<HarnessInstruction>,
}

// structural-naming:allow compatibility alias; active carrier is request::Request<request::Broad, request::Published>.
pub(crate) type PublishedBroadHarnessRequest = request::Request<request::Broad, request::Published>;

// structural-naming:allow compatibility alias; active carrier is request::Binding<surface::SurfacePolicyId>.
pub(crate) type RequestAdmissionBinding = request::Binding<surface::SurfacePolicyId>;

pub(crate) mod contract {
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Bundle {
        pub(crate) digest: Digest,
        pub(crate) validation: Validation,
        pub(crate) attempt: Attempt,
    }

    impl Bundle {
        pub(crate) fn prototype1(prototype_root: &Path) -> Self {
            Self {
                digest: Digest {
                    paths: vec![
                        PathRef {
                            label: "latest evaluation reports".to_string(),
                            path: prototype_root.join("evaluations"),
                            role: PathRole::LatestEvaluations,
                        },
                        PathRef {
                            label: "node protocol artifacts".to_string(),
                            path: prototype_root.join("nodes"),
                            role: PathRole::ProtocolArtifacts,
                        },
                        PathRef {
                            label: "sealed history blocks".to_string(),
                            path: prototype_root.join("history/blocks"),
                            role: PathRole::HistoryBlocks,
                        },
                    ],
                    facts: vec![
                        Fact {
                            label: "edit submission".to_string(),
                            value:
                                "Use the available edit tools to stage the candidate change; do not create separate result or bookkeeping files."
                                    .to_string(),
                            source: None,
                        },
                        Fact {
                            label: "oracle reports".to_string(),
                            value: "Use final_report.json when present and cite the source path."
                                .to_string(),
                            source: None,
                        },
                    ],
                },
                // Admission only needs to prove the candidate is a runnable
                // child Artifact. Immutability of the policy-bearing
                // `crates/ploke-eval` surface is already enforced structurally:
                // the `WorkspaceExceptPlokeEval` surface policy rejects edits to
                // that root before apply, and `validate_child_surface` re-checks
                // the surface-digest commitment before build. Candidate quality
                // is decided downstream by child self-evaluation and selection,
                // not at admission. So the gate is buildability of the
                // ploke-eval binary the child runs: a candidate that edits a
                // dependency crate (allowed by the broad edit scope) and breaks
                // ploke-eval's compile is not a runnable child. Running
                // ploke-eval's own test target here instead recompiled its
                // dev-dependency-heavy tests, which the broad edit scope can
                // break for reasons unrelated to runnability, making admission
                // unsatisfiable for any non-trivial edit.
                validation: Validation {
                    commands: vec![Command {
                        label: "compile ploke-eval".to_string(),
                        program: "cargo".to_string(),
                        args: vec![
                            "check".to_string(),
                            "-p".to_string(),
                            "ploke-eval".to_string(),
                        ],
                        workdir: Workdir::CandidateWorkspace,
                        success: "command exits successfully".to_string(),
                    }],
                },
                attempt: Attempt {
                    max_attempts: 4,
                    timeout: Timeout {
                        turn_seconds: 900,
                        tool_seconds: 180,
                    },
                    retry: Retry {
                        on_rejected_surface: true,
                        on_no_edit: true,
                        on_tool_failure: true,
                    },
                },
            }
        }

        pub(crate) fn empty() -> Self {
            Self {
                digest: Digest {
                    paths: Vec::new(),
                    facts: Vec::new(),
                },
                validation: Validation {
                    commands: Vec::new(),
                },
                attempt: Attempt {
                    max_attempts: 1,
                    timeout: Timeout {
                        turn_seconds: 0,
                        tool_seconds: 0,
                    },
                    retry: Retry {
                        on_rejected_surface: false,
                        on_no_edit: false,
                        on_tool_failure: false,
                    },
                },
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Digest {
        pub(crate) paths: Vec<PathRef>,
        pub(crate) facts: Vec<Fact>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct PathRef {
        pub(crate) label: String,
        pub(crate) path: PathBuf,
        pub(crate) role: PathRole,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum PathRole {
        LatestEvaluations,
        ProtocolArtifacts,
        HistoryBlocks,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Fact {
        pub(crate) label: String,
        pub(crate) value: String,
        pub(crate) source: Option<PathBuf>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Validation {
        pub(crate) commands: Vec<Command>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Command {
        pub(crate) label: String,
        pub(crate) program: String,
        pub(crate) args: Vec<String>,
        pub(crate) workdir: Workdir,
        pub(crate) success: String,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum Workdir {
        CandidateWorkspace,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Attempt {
        pub(crate) max_attempts: u32,
        pub(crate) timeout: Timeout,
        pub(crate) retry: Retry,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Timeout {
        pub(crate) turn_seconds: u64,
        pub(crate) tool_seconds: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Retry {
        pub(crate) on_rejected_surface: bool,
        pub(crate) on_no_edit: bool,
        pub(crate) on_tool_failure: bool,
    }
}

pub(crate) mod request {
    use std::{marker::PhantomData, path::PathBuf};

    use serde::{Deserialize, Deserializer, Serialize, de};

    use crate::loop_graph::{ArtifactId, Coordinate};

    use super::{BroadHarnessRequest, RequestAdmissionPolicyId, RequestSchema};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Broad {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Draft {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Published {}

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(transparent)]
    pub(crate) struct Identity<K> {
        value: String,
        #[serde(skip)]
        _kind: PhantomData<fn() -> K>,
    }

    impl<K> Identity<K> {
        pub(super) fn new(value: impl Into<String>) -> Self {
            Self {
                value: value.into(),
                _kind: PhantomData,
            }
        }

        pub(crate) fn as_str(&self) -> &str {
            &self.value
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(transparent)]
    pub(crate) struct Hash<K> {
        value: String,
        #[serde(skip)]
        _kind: PhantomData<fn() -> K>,
    }

    impl<K> Hash<K> {
        pub(super) fn new(value: impl Into<String>) -> Self {
            Self {
                value: value.into(),
                _kind: PhantomData,
            }
        }

        pub(super) fn empty() -> Self {
            Self::new(String::new())
        }

        pub(crate) fn as_str(&self) -> &str {
            &self.value
        }
    }

    impl<K> std::fmt::Display for Hash<K> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.as_str())
        }
    }

    impl<K> PartialEq<&str> for Hash<K> {
        fn eq(&self, other: &&str) -> bool {
            self.as_str() == *other
        }
    }

    impl<K> PartialEq<str> for Hash<K> {
        fn eq(&self, other: &str) -> bool {
            self.as_str() == other
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(bound = "")]
    pub(crate) struct Reference<K, S> {
        identity: Identity<K>,
        hash: Hash<K>,
        #[serde(skip)]
        _state: PhantomData<fn() -> S>,
    }

    impl<K, S> Reference<K, S> {
        pub(super) fn new(identity: Identity<K>, hash: Hash<K>) -> Self {
            Self {
                identity,
                hash,
                _state: PhantomData,
            }
        }

        pub(crate) fn request_id(&self) -> &str {
            self.identity.as_str()
        }

        pub(crate) fn request_hash(&self) -> &Hash<K> {
            &self.hash
        }
    }

    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    #[serde(bound = "")]
    pub(crate) struct Binding<P> {
        coordinate: Coordinate,
        target_artifact_id: ArtifactId,
        policy_id: RequestAdmissionPolicyId,
        #[serde(skip)]
        _policy: PhantomData<fn() -> P>,
    }

    impl<P> Binding<P> {
        pub(super) fn new_unchecked(
            coordinate: Coordinate,
            target_artifact_id: ArtifactId,
            policy_id: RequestAdmissionPolicyId,
        ) -> Self {
            Self {
                coordinate,
                target_artifact_id,
                policy_id,
                _policy: PhantomData,
            }
        }

        pub(crate) fn coordinate(&self) -> &Coordinate {
            &self.coordinate
        }

        pub(crate) fn target_artifact_id(&self) -> &ArtifactId {
            &self.target_artifact_id
        }

        pub(crate) fn policy_id(&self) -> &RequestAdmissionPolicyId {
            &self.policy_id
        }
    }

    impl<'de, P> Deserialize<'de> for Binding<P> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            struct Record {
                coordinate: Coordinate,
                target_artifact_id: ArtifactId,
                policy_id: RequestAdmissionPolicyId,
            }

            let record = Record::deserialize(deserializer)?;
            match &record.coordinate.target {
                crate::loop_graph::OperationTarget::Artifact { artifact_id }
                    if artifact_id == &record.target_artifact_id =>
                {
                    Ok(Self::new_unchecked(
                        record.coordinate,
                        record.target_artifact_id,
                        record.policy_id,
                    ))
                }
                actual_target => Err(de::Error::custom(format!(
                    "request binding coordinate target {:?} does not match target artifact {}",
                    actual_target, record.target_artifact_id
                ))),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(bound = "")]
    pub(crate) struct Request<K, S> {
        pub(crate) schema: RequestSchema,
        pub(crate) request_id: Identity<K>,
        pub(crate) request_hash: Hash<K>,
        pub(crate) request_path: PathBuf,
        pub(crate) prompt_path: PathBuf,
        pub(crate) submitted_result_path: PathBuf,
        pub(crate) admission_binding:
            Binding<crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId>,
        pub(crate) request: BroadHarnessRequest,
        #[serde(skip)]
        _state: PhantomData<fn() -> S>,
    }

    impl<K> Request<K, Published> {
        pub(super) fn new_published(
            schema: RequestSchema,
            request_id: Identity<K>,
            request_hash: Hash<K>,
            request_path: PathBuf,
            prompt_path: PathBuf,
            submitted_result_path: PathBuf,
            admission_binding: Binding<
                crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId,
            >,
            request: BroadHarnessRequest,
        ) -> Self {
            Self {
                schema,
                request_id,
                request_hash,
                request_path,
                prompt_path,
                submitted_result_path,
                admission_binding,
                request,
                _state: PhantomData,
            }
        }
    }
}

pub(crate) mod child {
    use std::path::{Path, PathBuf};

    use serde::{Deserialize, Serialize};

    use crate::loop_graph::ArtifactId;

    use super::{ArtifactSurface, RequestAdmissionBinding, request};

    /// Request-bound evidence carried by a child plan minted from an admitted
    /// broad harness result.
    ///
    /// The active transition is still owned by `ploke-eval`: this is the durable
    /// projection that keeps the child from downgrading into an unbound
    /// text-branch candidate after admission.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct Evidence {
        pub(crate) schema_version: u32,
        pub(crate) request: request::Reference<request::Broad, request::Published>,
        pub(crate) admission_binding: RequestAdmissionBinding,
        pub(crate) submitted_result_path: PathBuf,
        pub(crate) changed_paths: Vec<PathBuf>,
        pub(crate) artifact_surface: ArtifactSurface,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) workspace: Option<WorkspaceEvidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) artifact: Option<ArtifactEvidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) executor: Option<ExecutorEvidence>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct WorkspaceEvidence {
        pub(crate) source_root: PathBuf,
        pub(crate) candidate_root: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) base_head: Option<String>,
    }

    impl WorkspaceEvidence {
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
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct ArtifactEvidence {
        pub(crate) base_artifact_id: ArtifactId,
        pub(crate) derived_artifact_id: ArtifactId,
    }

    impl ArtifactEvidence {
        pub(crate) fn new(base_artifact_id: ArtifactId, derived_artifact_id: ArtifactId) -> Self {
            Self {
                base_artifact_id,
                derived_artifact_id,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub(crate) struct ExecutorEvidence {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) run_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) attempt_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) record_path: Option<PathBuf>,
    }

    impl ExecutorEvidence {
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
    }

    impl Evidence {
        pub(crate) fn admitted(
            request: request::Reference<request::Broad, request::Published>,
            admission_binding: RequestAdmissionBinding,
            submitted_result_path: PathBuf,
            changed_paths: Vec<PathBuf>,
            artifact_surface: ArtifactSurface,
        ) -> Self {
            Self {
                schema_version: 1,
                request,
                admission_binding,
                submitted_result_path,
                changed_paths,
                artifact_surface,
                workspace: None,
                artifact: None,
                executor: None,
            }
        }

        pub(crate) fn with_workspace(mut self, workspace: WorkspaceEvidence) -> Self {
            self.workspace = Some(workspace);
            self
        }

        pub(crate) fn with_artifact(mut self, artifact: ArtifactEvidence) -> Self {
            self.artifact = Some(artifact);
            self
        }

        pub(crate) fn with_executor(mut self, executor: Option<ExecutorEvidence>) -> Self {
            self.executor = executor;
            self
        }

        pub(crate) fn request(&self) -> &request::Reference<request::Broad, request::Published> {
            &self.request
        }

        pub(crate) fn admission_binding(&self) -> &RequestAdmissionBinding {
            &self.admission_binding
        }

        pub(crate) fn submitted_result_path(&self) -> &Path {
            &self.submitted_result_path
        }

        pub(crate) fn changed_paths(&self) -> &[PathBuf] {
            &self.changed_paths
        }

        pub(crate) fn artifact_surface(&self) -> &ArtifactSurface {
            &self.artifact_surface
        }

        pub(crate) fn workspace(&self) -> Option<&WorkspaceEvidence> {
            self.workspace.as_ref()
        }

        pub(crate) fn artifact(&self) -> Option<&ArtifactEvidence> {
            self.artifact.as_ref()
        }

        pub(crate) fn executor(&self) -> Option<&ExecutorEvidence> {
            self.executor.as_ref()
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BroadHarnessRequestSchema {
    V1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RequestSchema {
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Publication {
    sequence: u32,
}

impl Publication {
    fn allocate(
        request_path: &Path,
        prompt_path: &Path,
        submitted_result_path: &Path,
        workspace_path: &Path,
    ) -> Self {
        let mut sequence = 1;
        loop {
            let publication = Self { sequence };
            if publication.is_available(
                request_path,
                prompt_path,
                submitted_result_path,
                workspace_path,
            ) {
                return publication;
            }
            sequence += 1;
        }
    }

    fn request_id(self, parent_node_id: &str) -> String {
        match self.sequence {
            1 => format!("broad-harness-request:{parent_node_id}"),
            sequence => format!("broad-harness-request:{parent_node_id}:r{sequence}"),
        }
    }

    fn prompt_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn request_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn submitted_result_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn workspace_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_component_path(path, sequence),
        }
    }

    fn is_available(
        self,
        request_path: &Path,
        prompt_path: &Path,
        submitted_result_path: &Path,
        workspace_path: &Path,
    ) -> bool {
        !path_exists(&self.request_path(request_path))
            && !path_exists(&self.prompt_path(prompt_path))
            && !path_exists(&self.submitted_result_path(submitted_result_path))
            && !path_exists(&self.workspace_path(workspace_path))
    }
}

fn path_exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

fn suffixed_file_path(path: &Path, sequence: u32) -> PathBuf {
    let mut name = path
        .file_stem()
        .map(OsString::from)
        .or_else(|| path.file_name().map(OsString::from))
        .expect("published request path should end with a filename");
    name.push(format!("-r{sequence}"));
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    join_parent(path, name)
}

fn suffixed_component_path(path: &Path, sequence: u32) -> PathBuf {
    let mut name = path
        .file_name()
        .map(OsString::from)
        .expect("published request path should end with a path component");
    name.push(format!("-r{sequence}"));
    join_parent(path, name)
}

fn join_parent(path: &Path, name: OsString) -> PathBuf {
    match path.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct ParentNodeRef(String);

impl ParentNodeRef {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RequestBindingError {
    CoordinateTargetMismatch {
        expected_target_artifact_id: ArtifactId,
        actual_target: OperationTarget,
    },
}

impl request::Binding<surface::SurfacePolicyId> {
    pub(crate) fn new(
        coordinate: Coordinate,
        target_artifact_id: ArtifactId,
        policy_id: impl Into<String>,
    ) -> Result<Self, RequestBindingError> {
        match &coordinate.target {
            OperationTarget::Artifact { artifact_id } if artifact_id == &target_artifact_id => {
                Ok(Self::new_unchecked(
                    coordinate,
                    target_artifact_id,
                    RequestAdmissionPolicyId::new(policy_id),
                ))
            }
            actual_target => Err(RequestBindingError::CoordinateTargetMismatch {
                expected_target_artifact_id: target_artifact_id,
                actual_target: actual_target.clone(),
            }),
        }
    }

    pub(crate) fn from_admission(
        admission: &EditSurfaceAdmission,
    ) -> Result<Self, RequestBindingError> {
        let coordinate = admission.coordinate().clone();
        let target_artifact_id = match &coordinate.target {
            OperationTarget::Artifact { artifact_id } => artifact_id.clone(),
            actual_target => {
                return Err(RequestBindingError::CoordinateTargetMismatch {
                    expected_target_artifact_id: ArtifactId::new(
                        "<admission target artifact>".to_string(),
                    ),
                    actual_target: actual_target.clone(),
                });
            }
        };
        Self::new(coordinate, target_artifact_id, admission.policy().as_str())
    }

    fn prototype1_workspace(source_repository_path: &Path, edit_policy: SurfacePolicy) -> Self {
        let target_artifact_id = ArtifactId::new(source_repository_path.display().to_string());
        let coordinate = Coordinate {
            runtime_id: RuntimeId::new(),
            target: OperationTarget::Artifact {
                artifact_id: target_artifact_id.clone(),
            },
        };
        Self::new(coordinate, target_artifact_id, edit_policy.label())
            .expect("prototype workspace admission binding should match its target artifact")
    }

    pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
        self.target_artifact_id()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct RequestAdmissionPolicyId(String);

impl RequestAdmissionPolicyId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct HarnessWorkspace {
    pub(crate) source_repository_path: PathBuf,
    pub(crate) candidate_workspace_path: PathBuf,
}

impl HarnessWorkspace {
    fn new(source_repository_path: PathBuf, candidate_workspace_path: PathBuf) -> Self {
        Self {
            source_repository_path,
            candidate_workspace_path,
        }
    }

    pub(crate) fn source_repository_path(&self) -> &Path {
        &self.source_repository_path
    }

    pub(crate) fn candidate_workspace_path(&self) -> &Path {
        &self.candidate_workspace_path
    }

    fn display_source_repository(&self) -> impl fmt::Display + '_ {
        self.source_repository_path.display()
    }

    fn display_candidate_workspace(&self) -> impl fmt::Display + '_ {
        self.candidate_workspace_path.display()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct HarnessChildBudget {
    pub(crate) min_children: u32,
    pub(crate) max_children: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct GraphRestriction {
    pub(crate) mode: GraphRestrictionMode,
    pub(crate) nearest_items: usize,
    pub(crate) seed_modules: Vec<PathBuf>,
    pub(crate) source: GraphRestrictionSource,
}

impl GraphRestriction {
    fn tool_neighborhood(nearest_items: usize) -> Self {
        Self {
            mode: GraphRestrictionMode::ToolNeighborhood,
            nearest_items,
            seed_modules: vec![PathBuf::from("crates/ploke-tui/src/tools/mod.rs")],
            source: GraphRestrictionSource::CodeGraphCozo,
        }
    }
}

impl Default for GraphRestriction {
    fn default() -> Self {
        Self::tool_neighborhood(DEFAULT_GRAPH_NEAREST_ITEMS)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphRestrictionMode {
    ToolNeighborhood,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphRestrictionSource {
    CodeGraphCozo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PlanningGuidance {
    pub(crate) schema_version: String,
    pub(crate) planner: PlannerProfile,
    pub(crate) target_pipeline: String,
    pub(crate) cited_evidence: Vec<PlanningEvidence>,
    pub(crate) pipeline_scope: String,
    pub(crate) edit_intent: String,
    pub(crate) response_contract: Vec<PlanResponseField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifact_path: Option<PathBuf>,
}

impl PlanningGuidance {
    fn prototype1(evidence_roots: &[EvidenceRoot]) -> Self {
        Self {
            schema_version: "prototype1-guided-planning.v1".to_string(),
            planner: PlannerProfile {
                role: PlannerRole::HighCapacityReview,
                route: PlannerRoute::DirectGoogle,
            },
            target_pipeline: "Prototype 1 tool-use and edit-surface pipeline".to_string(),
            cited_evidence: evidence_roots.iter().map(PlanningEvidence::from_root).collect(),
            pipeline_scope:
                "Protocol diagnostics, recent child evidence, tool-call failures, and the ploke-tui tool modules seeded into the code graph."
                    .to_string(),
            edit_intent:
                "Choose a bounded code change inside the graph-restricted tool neighborhood that is likely to improve future descendant evaluation."
                    .to_string(),
            response_contract: vec![
                PlanResponseField::TargetPipeline,
                PlanResponseField::EvidenceCitations,
                PlanResponseField::PipelineScope,
                PlanResponseField::EditIntent,
            ],
            artifact_path: None,
        }
    }
}

impl Default for PlanningGuidance {
    fn default() -> Self {
        Self::prototype1(&[])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PlannerProfile {
    pub(crate) role: PlannerRole,
    pub(crate) route: PlannerRoute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlannerRole {
    HighCapacityReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PlannerRoute {
    DirectGoogle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PlanningEvidence {
    pub(crate) kind: EvidenceRootKind,
    pub(crate) role: EvidenceRole,
    pub(crate) location: EvidenceRootLocation,
}

impl PlanningEvidence {
    fn from_root(root: &EvidenceRoot) -> Self {
        Self {
            kind: root.kind,
            role: root.role,
            location: root.location.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanResponseField {
    TargetPipeline,
    EvidenceCitations,
    PipelineScope,
    EditIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProtectedCorePointer {
    pub(crate) anchor: ProtectedCoreAnchor,
    pub(crate) policy: ProtectedCorePolicy,
    pub(crate) consequence: ProtectedCoreConsequence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProtectedCoreAnchor {
    AuthorityConstant {
        code_path: PathBuf,
        symbol: ProtectedCoreSymbol,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCoreSymbol {
    EvalCoreSurfaceRoot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCorePolicy {
    WorkspaceExceptPlokeEvalAuthoritySet,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCoreConsequence {
    RejectBeforeAdmissionOrInvalidDescendant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvaluationBrief {
    pub(crate) scope: EvaluationScope,
    pub(crate) selection: SelectionAuthority,
    pub(crate) guidance: GuidancePolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationScope {
    Prototype1DescendantPerformance,
}

impl EvaluationScope {
    fn benchmark_name(self) -> &'static str {
        match self {
            Self::Prototype1DescendantPerformance => "Prototype 1 descendant performance",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionAuthority {
    HistoryBackedSuccessorSelection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuidancePolicy {
    ProtocolDiagnosticsAreContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvidenceRoot {
    pub(crate) kind: EvidenceRootKind,
    pub(crate) location: EvidenceRootLocation,
    pub(crate) role: EvidenceRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceRootKind {
    SubmittedResultOutput,
    HistoryBlocks,
    Evaluations,
    Nodes,
    ProtocolArtifacts,
    Oracle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EvidenceRootLocation {
    Directory {
        path: PathBuf,
    },
    File {
        path: PathBuf,
    },
    NodeScopedDirectory {
        nodes_root: PathBuf,
        child_relpath: PathBuf,
    },
    AttachedReport {
        report: AttachedReport,
    },
}

impl EvidenceRootLocation {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Directory { path } => path.display().to_string(),
            Self::File { path } => path.display().to_string(),
            Self::NodeScopedDirectory {
                nodes_root,
                child_relpath,
            } => format!(
                "{}/<node>/{}",
                nodes_root.display(),
                child_relpath.display()
            ),
            Self::AttachedReport { report } => report.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttachedReport {
    FinalReportJson,
}

impl AttachedReport {
    fn label(self) -> &'static str {
        match self {
            Self::FinalReportJson => "final_report.json when present",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceRole {
    OutputBox,
    SealedHistory,
    EvaluationPayloads,
    RuntimeEvidence,
    GuidanceOnly,
    OracleSummary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HarnessInstruction {
    InspectRepositoryAndEvidence,
    TreatProtocolDiagnosticsAsGuidance,
    EditBroadSurfaceOutsideProtectedCore,
    ChooseLikelyDescendantImprovement,
    #[serde(alias = "write_submitted_result")]
    StageCandidateChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReturnEvidenceContract {
    pub(crate) authority_boundary: SubmissionAuthorityBoundary,
    pub(crate) fields: Vec<ReturnEvidenceField>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReturnEvidenceField {
    ChangeSummary,
    GuidingEvidence,
    ImprovementRationale,
    SuggestedChecks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmissionAuthorityBoundary {
    pub(crate) admission: SubmissionAuthorityClaim,
    pub(crate) grant: SubmissionAuthorityClaim,
    pub(crate) child_plan: SubmissionAuthorityClaim,
}

impl SubmissionAuthorityBoundary {
    pub(crate) fn submitted_evidence_only() -> Self {
        Self {
            admission: SubmissionAuthorityClaim::NotClaimed,
            grant: SubmissionAuthorityClaim::NotClaimed,
            child_plan: SubmissionAuthorityClaim::NotClaimed,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubmissionAuthorityClaim {
    NotClaimed,
}

impl BroadHarnessRequest {
    pub(crate) fn prototype1_workspace(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        candidate_workspace_path: PathBuf,
        prototype_root: &Path,
        submitted_result_path: &Path,
    ) -> Self {
        Self::prototype1_workspace_with_graph_limit(
            parent_node_id,
            source_repository,
            child_budget,
            candidate_workspace_path,
            prototype_root,
            submitted_result_path,
            DEFAULT_GRAPH_NEAREST_ITEMS,
        )
    }

    pub(crate) fn prototype1_workspace_with_graph_limit(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        candidate_workspace_path: PathBuf,
        prototype_root: &Path,
        submitted_result_path: &Path,
        nearest_items: usize,
    ) -> Self {
        let evaluations_root = prototype_root.join("evaluations");
        let nodes_root = prototype_root.join("nodes");
        let mut evidence_roots = vec![
            EvidenceRoot {
                kind: EvidenceRootKind::SubmittedResultOutput,
                location: EvidenceRootLocation::File {
                    path: submitted_result_path.to_path_buf(),
                },
                role: EvidenceRole::OutputBox,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::HistoryBlocks,
                location: EvidenceRootLocation::Directory {
                    path: prototype_root.join("history/blocks"),
                },
                role: EvidenceRole::SealedHistory,
            },
        ];
        if directory_has_entries(&evaluations_root) {
            evidence_roots.push(EvidenceRoot {
                kind: EvidenceRootKind::Evaluations,
                location: EvidenceRootLocation::Directory {
                    path: evaluations_root,
                },
                role: EvidenceRole::EvaluationPayloads,
            });
        }
        evidence_roots.extend([
            EvidenceRoot {
                kind: EvidenceRootKind::Nodes,
                location: EvidenceRootLocation::Directory {
                    path: nodes_root.clone(),
                },
                role: EvidenceRole::RuntimeEvidence,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::ProtocolArtifacts,
                location: EvidenceRootLocation::NodeScopedDirectory {
                    nodes_root,
                    child_relpath: PathBuf::from("protocol-artifacts"),
                },
                role: EvidenceRole::GuidanceOnly,
            },
            EvidenceRoot {
                kind: EvidenceRootKind::Oracle,
                location: EvidenceRootLocation::AttachedReport {
                    report: AttachedReport::FinalReportJson,
                },
                role: EvidenceRole::OracleSummary,
            },
        ]);
        Self {
            schema: BroadHarnessRequestSchema::V1,
            parent_node_id: ParentNodeRef::new(parent_node_id),
            workspace: HarnessWorkspace::new(source_repository, candidate_workspace_path),
            edit_policy: SurfacePolicy::workspace_except_core(),
            graph_restriction: GraphRestriction::tool_neighborhood(nearest_items),
            child_budget,
            protected_core: ProtectedCorePointer {
                anchor: ProtectedCoreAnchor::AuthorityConstant {
                    code_path: PathBuf::from(
                        "crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs",
                    ),
                    symbol: ProtectedCoreSymbol::EvalCoreSurfaceRoot,
                },
                policy: ProtectedCorePolicy::WorkspaceExceptPlokeEvalAuthoritySet,
                consequence: ProtectedCoreConsequence::RejectBeforeAdmissionOrInvalidDescendant,
            },
            evaluation: EvaluationBrief {
                scope: EvaluationScope::Prototype1DescendantPerformance,
                selection: SelectionAuthority::HistoryBackedSuccessorSelection,
                guidance: GuidancePolicy::ProtocolDiagnosticsAreContext,
            },
            planning: PlanningGuidance::prototype1(&evidence_roots),
            contract: contract::Bundle::prototype1(prototype_root),
            evidence_roots,
            return_evidence: ReturnEvidenceContract {
                authority_boundary: SubmissionAuthorityBoundary::submitted_evidence_only(),
                fields: vec![
                    ReturnEvidenceField::ChangeSummary,
                    ReturnEvidenceField::GuidingEvidence,
                    ReturnEvidenceField::ImprovementRationale,
                    ReturnEvidenceField::SuggestedChecks,
                ],
            },
            instructions: vec![
                HarnessInstruction::InspectRepositoryAndEvidence,
                HarnessInstruction::TreatProtocolDiagnosticsAsGuidance,
                HarnessInstruction::EditBroadSurfaceOutsideProtectedCore,
                HarnessInstruction::ChooseLikelyDescendantImprovement,
                HarnessInstruction::StageCandidateChange,
            ],
        }
    }

    pub(crate) fn render_prompt(&self) -> String {
        let mut prompt = String::new();
        prompt.push_str(&format!(
            "Modify the candidate checkout at `{}` to improve performance on the `{}` benchmark. Edits must stay outside the protected core.\n\n",
            self.workspace.display_candidate_workspace(),
            self.evaluation.scope.benchmark_name()
        ));
        prompt.push_str(
            "Treat the current direcory as the write target and command root.
            Resolve ordinary file paths there; read only the explicitly listed
            evidence paths outside it. When running cargo, prefer root-level
            `cargo check` or `cargo test` unless you have confirmed a package
            name in the candidate checkout.\n",
        );

        prompt.push_str("All files in this directory may be read.\n");
        if let Some(evaluations) = self.evidence_root(EvidenceRootKind::Evaluations) {
            prompt.push_str(&format!(
                "Past benchmark results live under `{}`.\n",
                evaluations.location.render()
            ));
        }
        if let Some(nodes) = self.evidence_root(EvidenceRootKind::Nodes) {
            prompt.push_str(&format!(
                "If prior attempt or conversation history is useful, inspect `{}`.\n",
                nodes.location.render()
            ));
        }
        if let Some(protected_core) = self.protected_core_definition() {
            prompt.push_str(&format!(
                "Protected core: see `{}` and `WORKSPACE_EXCEPT_AUTHORITY_*`. Ordinary edits touching that surface will be rejected.\n",
                protected_core
            ));
        }
        prompt.push_str(&format!(
            "Before editing, produce and follow a structured planning review with target_pipeline, evidence_citations, pipeline_scope, and edit_intent. Use the `{}` planner route for that review.\n",
            self.planning.planner.route.render()
        ));
        if let Some(path) = self.planning.artifact_path.as_ref() {
            prompt.push_str(&format!(
                "A parent-side pre-child planning review is expected at `{}` before this request is executed; read it as guidance, but do not treat it as submitted child evidence.\n",
                path.display()
            ));
        }
        prompt.push_str(&format!(
            "Mutable target policy: use the code graph Cozo DB to restrict edits to the {} nearest code items seeded from {}. Treat protocol output and detected tool failures as ranking guidance inside that neighborhood.\n",
            self.graph_restriction.nearest_items,
            self.graph_restriction
                .seed_modules
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        prompt.push_str(
            "Inspect the repository and evidence. Choose the change you think is most likely to improve future evaluated descendants. The provided evaluations are guidance, not hard edit targets.\n",
        );
        prompt
    }

    fn evidence_root(&self, kind: EvidenceRootKind) -> Option<&EvidenceRoot> {
        self.evidence_roots.iter().find(|root| root.kind == kind)
    }

    fn protected_core_definition(&self) -> Option<String> {
        match &self.protected_core.anchor {
            ProtectedCoreAnchor::AuthorityConstant { code_path, symbol } => Some(format!(
                "{}::{symbol_name}",
                code_path.display(),
                symbol_name = symbol.render()
            )),
        }
    }
}

const DEFAULT_GRAPH_NEAREST_ITEMS: usize = 24;

impl PlannerRoute {
    fn render(self) -> &'static str {
        match self {
            Self::DirectGoogle => "direct-google",
        }
    }
}

fn directory_has_entries(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

impl ProtectedCoreSymbol {
    fn render(self) -> &'static str {
        match self {
            Self::EvalCoreSurfaceRoot => "EVAL_CORE_SURFACE_ROOT",
        }
    }
}

impl request::Request<request::Broad, request::Published> {
    pub(crate) fn prototype1_workspace(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        prototype_root: &Path,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
        admission_binding: request::Binding<surface::SurfacePolicyId>,
    ) -> Self {
        Self::prototype1_workspace_with_graph_limit(
            parent_node_id,
            source_repository,
            child_budget,
            prototype_root,
            request_path,
            prompt_path,
            submitted_result_path,
            admission_binding,
            DEFAULT_GRAPH_NEAREST_ITEMS,
        )
    }

    pub(crate) fn prototype1_workspace_with_graph_limit(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        prototype_root: &Path,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
        admission_binding: request::Binding<surface::SurfacePolicyId>,
        nearest_items: usize,
    ) -> Self {
        let workspace_path = prototype_root
            .join("workspaces/edit-harness")
            .join(&parent_node_id);
        let publication = Publication::allocate(
            request_path.as_path(),
            prompt_path.as_path(),
            submitted_result_path.as_path(),
            workspace_path.as_path(),
        );
        let request_path = publication.request_path(request_path.as_path());
        let prompt_path = publication.prompt_path(prompt_path.as_path());
        let submitted_result_path =
            publication.submitted_result_path(submitted_result_path.as_path());
        let candidate_workspace_path = publication.workspace_path(workspace_path.as_path());
        let request = BroadHarnessRequest::prototype1_workspace_with_graph_limit(
            parent_node_id.clone(),
            source_repository,
            child_budget,
            candidate_workspace_path,
            prototype_root,
            &submitted_result_path,
            nearest_items,
        );
        let mut published = Self::new_published(
            RequestSchema::V1,
            request::Identity::new(publication.request_id(&parent_node_id)),
            request::Hash::empty(),
            request_path,
            prompt_path,
            submitted_result_path,
            admission_binding,
            request,
        );
        published.request_hash = request::Hash::new(published.compute_request_hash());
        published
    }

    pub(crate) fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub(crate) fn request_hash(&self) -> &str {
        self.request_hash.as_str()
    }

    pub(crate) fn reference(&self) -> request::Reference<request::Broad, request::Published> {
        request::Reference::new(self.request_id.clone(), self.request_hash.clone())
    }

    pub(crate) fn request_path(&self) -> &Path {
        &self.request_path
    }

    pub(crate) fn prompt_path(&self) -> &Path {
        &self.prompt_path
    }

    pub(crate) fn submitted_result_path(&self) -> &Path {
        &self.submitted_result_path
    }

    pub(crate) fn admission_binding(&self) -> &request::Binding<surface::SurfacePolicyId> {
        &self.admission_binding
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        self.request.workspace.candidate_workspace_path()
    }

    pub(crate) fn request(&self) -> &BroadHarnessRequest {
        &self.request
    }

    pub(crate) fn with_admission_binding(
        mut self,
        admission_binding: request::Binding<surface::SurfacePolicyId>,
    ) -> Self {
        self.admission_binding = admission_binding;
        self.request_hash = request::Hash::new(self.compute_request_hash());
        self
    }

    pub(crate) fn with_planning_artifact_path(mut self, path: PathBuf) -> Self {
        self.request.planning.artifact_path = Some(path);
        self.request_hash = request::Hash::new(self.compute_request_hash());
        self
    }

    fn compute_request_hash(&self) -> String {
        let preimage = RequestPreimage {
            request_id: self.request_id(),
            request_path: &self.request_path,
            prompt_path: &self.prompt_path,
            submitted_result_path: &self.submitted_result_path,
            admission_binding: self.admission_binding(),
            request: &self.request,
        };
        let bytes = serde_json::to_vec(&preimage)
            .expect("published broad harness request preimage should serialize");
        format!("{:x}", Sha256::digest(bytes))
    }
}

impl From<&request::Request<request::Broad, request::Published>>
    for request::Reference<request::Broad, request::Published>
{
    fn from(value: &request::Request<request::Broad, request::Published>) -> Self {
        value.reference()
    }
}

#[derive(Serialize)]
struct RequestPreimage<'a> {
    request_id: &'a str,
    request_path: &'a PathBuf,
    prompt_path: &'a PathBuf,
    submitted_result_path: &'a PathBuf,
    admission_binding: &'a request::Binding<surface::SurfacePolicyId>,
    request: &'a BroadHarnessRequest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::prototype1_state::backend::EditSurfaceAdmission;
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
            let evaluations_dir = prototype_root.join("evaluations");
            fs::create_dir_all(&prompt_dir).expect("create prompt dir");
            fs::create_dir_all(&result_dir).expect("create result dir");
            fs::create_dir_all(&evaluations_dir).expect("create evaluations dir");
            fs::write(evaluations_dir.join("branch-sample.json"), "{}\n")
                .expect("write evaluation sample");
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
                "artifact:/repo/live-parent",
                "workspace except ploke-eval",
            );
            PublishedBroadHarnessRequest::prototype1_workspace(
                "parent-node-7".to_string(),
                PathBuf::from("/repo/live-parent"),
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

        fn request_admission_binding(
            &self,
            artifact_id: &str,
            policy_id: &str,
        ) -> RequestAdmissionBinding {
            let _ = self;
            let coordinate = Coordinate {
                runtime_id: RuntimeId::new(),
                target: OperationTarget::Artifact {
                    artifact_id: ArtifactId::new(artifact_id),
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
    fn published_request_binds_isolated_workspace_and_submitted_result() {
        let fixture = Fixture::new();
        let published = fixture.published_request();

        let json =
            serde_json::to_string(&published).expect("serialize published broad harness request");
        let decoded = serde_json::from_str::<PublishedBroadHarnessRequest>(&json)
            .expect("deserialize published broad harness request");

        assert_eq!(decoded, published);
        assert_eq!(
            decoded.request.workspace.source_repository_path(),
            Path::new("/repo/live-parent")
        );
        assert_eq!(
            decoded.workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7")
                .as_path()
        );
        assert_eq!(decoded.request_path(), fixture.request_path.as_path());
        assert_eq!(
            decoded.submitted_result_path(),
            fixture.submitted_result_path.as_path()
        );
        assert_eq!(
            decoded.request.return_evidence.authority_boundary,
            SubmissionAuthorityBoundary::submitted_evidence_only()
        );
        assert_eq!(
            decoded.request.return_evidence.fields,
            vec![
                ReturnEvidenceField::ChangeSummary,
                ReturnEvidenceField::GuidingEvidence,
                ReturnEvidenceField::ImprovementRationale,
                ReturnEvidenceField::SuggestedChecks,
            ]
        );
        assert_eq!(
            decoded.request.graph_restriction.mode,
            GraphRestrictionMode::ToolNeighborhood
        );
        assert_eq!(
            decoded.request.graph_restriction.source,
            GraphRestrictionSource::CodeGraphCozo
        );
        assert_eq!(
            decoded.request.graph_restriction.nearest_items,
            DEFAULT_GRAPH_NEAREST_ITEMS
        );
        assert_eq!(
            decoded.request.graph_restriction.seed_modules,
            vec![PathBuf::from("crates/ploke-tui/src/tools/mod.rs")]
        );
        assert_eq!(
            decoded.request.planning.planner.role,
            PlannerRole::HighCapacityReview
        );
        assert_eq!(
            decoded.request.planning.planner.route,
            PlannerRoute::DirectGoogle
        );
        assert_eq!(
            decoded.request.planning.response_contract,
            vec![
                PlanResponseField::TargetPipeline,
                PlanResponseField::EvidenceCitations,
                PlanResponseField::PipelineScope,
                PlanResponseField::EditIntent,
            ]
        );
        assert_eq!(decoded.request.contract.attempt.max_attempts, 4);
        assert_eq!(decoded.request.contract.validation.commands.len(), 1);
        assert_eq!(
            decoded.request.contract.validation.commands[0].args,
            vec![
                "check".to_string(),
                "-p".to_string(),
                "ploke-eval".to_string()
            ]
        );
        assert!(
            decoded
                .request
                .contract
                .digest
                .paths
                .iter()
                .any(|path| path.path.ends_with("evaluations"))
        );
        assert_eq!(
            decoded.admission_binding().base_artifact_id(),
            &ArtifactId::new("artifact:/repo/live-parent")
        );
        assert_eq!(
            decoded.admission_binding().policy_id().as_str(),
            "workspace except ploke-eval"
        );
    }

    #[test]
    fn graph_limited_publication_carries_planning_prompt_contract() {
        let fixture = Fixture::new();
        let admission_binding = fixture
            .request_admission_binding("artifact:/repo/live-parent", "workspace except ploke-eval");
        let published = PublishedBroadHarnessRequest::prototype1_workspace_with_graph_limit(
            "parent-node-7".to_string(),
            PathBuf::from("/repo/live-parent"),
            HarnessChildBudget {
                min_children: 1,
                max_children: 3,
            },
            &fixture.prototype_root,
            fixture.request_path.clone(),
            fixture.prompt_path.clone(),
            fixture.submitted_result_path.clone(),
            admission_binding,
            7,
        );

        let prompt = published.request.render_prompt();

        assert_eq!(published.request.graph_restriction.nearest_items, 7);
        assert!(prompt.contains("structured planning review"));
        assert!(
            prompt.contains("target_pipeline, evidence_citations, pipeline_scope, and edit_intent")
        );
        assert!(prompt.contains("direct-google"));
        assert!(prompt.contains("7 nearest code items"));
        assert!(prompt.contains("crates/ploke-tui/src/tools/mod.rs"));
        assert!(prompt.contains("protocol output and detected tool failures"));
    }

    #[test]
    fn prompt_omits_evaluations_when_no_reports_exist() {
        let temp = tempfile::tempdir().expect("tempdir");
        let prototype_root = temp.path().join("prototype1");
        fs::create_dir_all(prototype_root.join("nodes")).expect("create nodes");
        let request = BroadHarnessRequest::prototype1_workspace(
            "parent-node-7".to_string(),
            PathBuf::from("/repo/live-parent"),
            HarnessChildBudget {
                min_children: 1,
                max_children: 3,
            },
            prototype_root.join("workspaces/edit-harness/parent-node-7"),
            &prototype_root,
            &prototype_root.join("messages/edit-harness-result/parent-node-7.json"),
        );

        let prompt = request.render_prompt();

        assert!(!prompt.contains("Past benchmark results live under"));
        assert!(
            !request
                .evidence_roots
                .iter()
                .any(|root| root.kind == EvidenceRootKind::Evaluations)
        );
        assert!(prompt.contains("If prior attempt or conversation history is useful"));
    }

    #[test]
    fn repeated_publication_for_same_parent_gets_request_scoped_identity() {
        let fixture = Fixture::new();
        let first = fixture.published_request();
        fs::write(first.prompt_path(), "published prompt").expect("write first prompt");

        let second = fixture.published_request();

        assert_eq!(first.request_id(), "broad-harness-request:parent-node-7");
        assert_eq!(
            second.request_id(),
            "broad-harness-request:parent-node-7:r2"
        );
        assert_eq!(
            second.request_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.json")
                .as_path()
        );
        assert_ne!(first.request_hash(), second.request_hash());
        assert_ne!(first.workspace_path(), second.workspace_path());
        assert_ne!(
            first.submitted_result_path(),
            second.submitted_result_path()
        );
        assert_eq!(
            second.prompt_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.md")
                .as_path()
        );
        assert_eq!(
            second.submitted_result_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-result/parent-node-7-r2.json")
                .as_path()
        );
        assert_eq!(
            second.workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7-r2")
                .as_path()
        );
    }

    #[test]
    fn request_json_alone_reserves_publication_identity() {
        let fixture = Fixture::new();
        fs::write(&fixture.request_path, "existing request").expect("write existing request");

        let published = fixture.published_request();

        assert_eq!(
            published.request_id(),
            "broad-harness-request:parent-node-7:r2"
        );
        assert_eq!(
            published.request_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.json")
                .as_path()
        );
    }

    #[test]
    fn admission_binding_round_trips_through_published_request() {
        let fixture = Fixture::new();
        let binding =
            fixture.request_admission_binding("artifact:broad-base", "policy:broad-boundary");
        let published = fixture
            .published_request()
            .with_admission_binding(binding.clone());

        let json =
            serde_json::to_string(&published).expect("serialize published broad harness request");
        let decoded = serde_json::from_str::<PublishedBroadHarnessRequest>(&json)
            .expect("deserialize published broad harness request");

        assert_eq!(decoded.admission_binding(), &binding);
        assert_eq!(
            decoded.admission_binding().base_artifact_id(),
            &ArtifactId::new("artifact:broad-base")
        );
        assert_eq!(
            decoded.admission_binding().policy_id().as_str(),
            "policy:broad-boundary"
        );
    }

    #[test]
    fn published_request_rejects_missing_admission_binding() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let mut json = serde_json::to_value(&published).expect("serialize published request");

        json.as_object_mut()
            .expect("published request is a JSON object")
            .remove("admission_binding");

        assert!(
            serde_json::from_value::<PublishedBroadHarnessRequest>(json).is_err(),
            "missing authority binding should not deserialize"
        );
    }

    #[test]
    fn request_hash_changes_when_admission_binding_changes() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let first = published.clone().with_admission_binding(
            fixture.request_admission_binding("artifact:broad-base", "policy:first"),
        );
        let second = published.with_admission_binding(
            fixture.request_admission_binding("artifact:other-base", "policy:first"),
        );

        assert_ne!(first.request_hash(), second.request_hash());
    }
}
