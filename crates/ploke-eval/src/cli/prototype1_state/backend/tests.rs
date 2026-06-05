use super::edit_surface::harness_request::{
    EvidenceRootKind, EvidenceRootLocation, HarnessChildBudget, PublishedBroadHarnessRequest,
    RequestAdmissionBinding, SubmissionAuthorityBoundary,
};
use super::edit_surface::harness_result::{
    SubmittedBroadHarnessResult, SubmittedBroadHarnessResultError, SubmittedChangeSummary,
    SubmittedCheckRecommendation, SubmittedEvidenceCitation, SubmittedFileChange,
    SubmittedHarnessReturnEvidence, SubmittedImprovementRationale,
};
use super::edit_surface::{request_policy, surface, tui};
use super::{
    AdmittedBroadHarnessResult, BackendError, EditSurfaceAdmission, GitWorktreeBackend,
    TuiAttemptOutcome, WorkspaceBackend, WorktreeEntry,
    describe_submitted_broad_harness_result_error, parse_dirty_paths, parse_worktree_list,
};
use crate::cli::prototype1_state::identity::{
    PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity, ParentIdentityRecord,
    parent_identity_commit_message, parent_identity_relpath, write_parent_identity,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

fn admission_for(artifact_id: crate::loop_graph::ArtifactId) -> EditSurfaceAdmission {
    EditSurfaceAdmission::new(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId(Uuid::nil()),
            target: crate::loop_graph::OperationTarget::Artifact { artifact_id },
        },
        surface::SurfacePolicyId::new("policy:test-boundary"),
    )
}

fn run_git_test(repo_root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo_root = tmp.path();
    run_git_test(repo_root, &["init"]);
    run_git_test(
        repo_root,
        &["config", "user.email", "prototype1@example.com"],
    );
    run_git_test(repo_root, &["config", "user.name", "Prototype 1 Test"]);
    fs::write(repo_root.join("README.md"), "base\n").expect("write base");
    run_git_test(repo_root, &["add", "README.md"]);
    run_git_test(repo_root, &["commit", "--no-gpg-sign", "-m", "base commit"]);
    tmp
}

struct BroadHarnessFixture {
    _temp: tempfile::TempDir,
    source_root: PathBuf,
    prototype_root: PathBuf,
    request_path: PathBuf,
    prompt_path: PathBuf,
    submitted_result_path: PathBuf,
}

impl BroadHarnessFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_root = temp.path().join("source-repo");
        fs::create_dir_all(&source_root).expect("create source repo dir");
        run_git_test(&source_root, &["init"]);
        run_git_test(
            &source_root,
            &["config", "user.email", "prototype1@example.com"],
        );
        run_git_test(&source_root, &["config", "user.name", "Prototype 1 Test"]);
        fs::write(source_root.join("README.md"), "base\n").expect("write readme");
        let protected = source_root.join("crates/ploke-eval/src");
        fs::create_dir_all(&protected).expect("create protected dir");
        fs::write(protected.join("lib.rs"), "pub fn protected() {}\n")
            .expect("write protected file");
        let agents = source_root.join(".agents");
        fs::create_dir_all(&agents).expect("create agents dir");
        fs::write(
            agents.join("hyper-agents.txt"),
            "protected operator context\n",
        )
        .expect("write protected agent context");
        let allowed = source_root.join("src");
        fs::create_dir_all(&allowed).expect("create allowed dir");
        fs::write(allowed.join("feature.rs"), "pub fn feature() {}\n").expect("write allowed file");
        for relpath in super::ploke_tui_tool_files() {
            let path = source_root.join(relpath);
            fs::create_dir_all(path.parent().expect("tool file parent"))
                .expect("create tool file parent");
            fs::write(path, "tool surface\n").expect("write tool surface file");
        }
        for relpath in super::tool_description_paths() {
            let path = source_root.join(relpath);
            fs::create_dir_all(path.parent().expect("description file parent"))
                .expect("create description file parent");
            fs::write(path, "tool description\n").expect("write tool description file");
        }
        run_git_test(&source_root, &["add", "."]);
        run_git_test(
            &source_root,
            &["commit", "--no-gpg-sign", "-m", "broad harness base"],
        );

        let prototype_root = temp.path().join("prototype1");
        let prompt_dir = prototype_root.join("messages/edit-harness-request");
        let result_dir = prototype_root.join("messages/edit-harness-result");
        fs::create_dir_all(&prompt_dir).expect("create prompt dir");
        fs::create_dir_all(&result_dir).expect("create result dir");

        Self {
            _temp: temp,
            source_root,
            prototype_root,
            request_path: prompt_dir.join("parent-node-7.json"),
            prompt_path: prompt_dir.join("parent-node-7.md"),
            submitted_result_path: result_dir.join("parent-node-7.json"),
        }
    }

    fn published_request(&self) -> PublishedBroadHarnessRequest {
        self.published_request_with_source(self.source_root.clone())
    }

    fn published_request_with_source(
        &self,
        source_repository: PathBuf,
    ) -> PublishedBroadHarnessRequest {
        let admission_binding = RequestAdmissionBinding::from_admission(&admission_for(
            crate::loop_graph::ArtifactId::new("artifact:broad-base"),
        ))
        .expect("construct published request admission binding");
        PublishedBroadHarnessRequest::prototype1_workspace(
            "parent-node-7".to_string(),
            source_repository,
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

    fn clone_candidate_workspace(&self, published: &PublishedBroadHarnessRequest) {
        let candidate_root = published.workspace_path();
        fs::create_dir_all(candidate_root.parent().expect("candidate workspace parent"))
            .expect("create candidate workspace parent");
        run_git_test(
            self._temp.path(),
            &[
                "clone",
                self.source_root
                    .to_str()
                    .expect("source root is valid utf-8"),
                candidate_root
                    .to_str()
                    .expect("candidate root is valid utf-8"),
            ],
        );
        run_git_test(
            candidate_root,
            &["config", "user.email", "prototype1@example.com"],
        );
        run_git_test(candidate_root, &["config", "user.name", "Prototype 1 Test"]);
    }
}

fn submitted_broad_harness_result(
    published: &PublishedBroadHarnessRequest,
    changed_paths: &[PathBuf],
) -> SubmittedBroadHarnessResult {
    SubmittedBroadHarnessResult::bind(
        published,
        SubmittedHarnessReturnEvidence {
            authority_boundary: SubmissionAuthorityBoundary::submitted_evidence_only(),
            change_summary: SubmittedChangeSummary {
                changed_files: changed_paths
                    .iter()
                    .cloned()
                    .map(|workspace_relpath| SubmittedFileChange {
                        workspace_relpath,
                        summary: "Candidate broad harness change".to_string(),
                    })
                    .collect(),
            },
            guiding_evidence: vec![SubmittedEvidenceCitation {
                kind: EvidenceRootKind::HistoryBlocks,
                location: EvidenceRootLocation::Directory {
                    path: PathBuf::from("/tmp/prototype1/history/blocks"),
                },
                summary: "History evidence suggested a broad harness update.".to_string(),
            }],
            rationale: SubmittedImprovementRationale {
                hypothesis:
                    "Editing the allowed workspace surface should improve future descendants."
                        .to_string(),
                expected_descendant_effect:
                    "The admitted descendant should carry the candidate workspace improvement."
                        .to_string(),
            },
            checks: vec![SubmittedCheckRecommendation {
                label: "backend tests".to_string(),
                command: "cargo test -p ploke-eval backend".to_string(),
                success_signal: "Backend admission tests pass.".to_string(),
            }],
        },
    )
    .expect("bind submitted broad harness result")
}

fn assert_candidate_clean(admitted: &AdmittedBroadHarnessResult) {
    let dirty = super::dirty_paths(admitted.workspace_root()).expect("candidate dirty paths");
    assert!(
        dirty.is_empty(),
        "candidate workspace should be clean after admission"
    );
}

fn init_surface_repo(eval_text: &str, tool_text: &str) -> tempfile::TempDir {
    let tmp = init_git_repo();
    let repo_root = tmp.path();
    let eval_path = repo_root.join("crates/ploke-eval/src");
    fs::create_dir_all(&eval_path).expect("create eval dir");
    fs::write(eval_path.join("lib.rs"), eval_text).expect("write eval file");
    let tui_tool_path = repo_root.join("crates/ploke-tui/src/tools");
    fs::create_dir_all(&tui_tool_path).expect("create tui tools dir");
    fs::write(tui_tool_path.join("code_edit.rs"), tool_text).expect("write tui tool file");
    for relpath in super::ploke_tui_tool_files() {
        let path = repo_root.join(relpath);
        fs::create_dir_all(path.parent().expect("tui file has parent"))
            .expect("create tui file dir");
        fs::write(path, tool_text).expect("write tui file");
    }
    for relpath in super::tool_description_paths() {
        let path = repo_root.join(relpath);
        fs::create_dir_all(path.parent().expect("tool file has parent")).expect("create tool dir");
        fs::write(path, tool_text).expect("write tool file");
    }
    run_git_test(repo_root, &["add", "crates"]);
    run_git_test(
        repo_root,
        &["commit", "--no-gpg-sign", "-m", "surface files"],
    );
    tmp
}

fn write_tui_target(repo_root: &std::path::Path, content: &str) -> PathBuf {
    let relpath = PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs");
    let path = repo_root.join(&relpath);
    fs::create_dir_all(path.parent().expect("target has parent")).expect("create target dir");
    fs::write(path, content).expect("write tui target");
    run_git_test(
        repo_root,
        &["add", relpath.to_str().expect("test relpath is utf-8")],
    );
    relpath
}

fn proposal_for(
    repo_root: &std::path::Path,
    relpath: PathBuf,
    source: &str,
) -> super::EditProposal {
    let hash = super::content_hash(source);
    let touches = vec![super::ProposedTouch {
        target: "code_edit".to_string(),
        relpath: relpath.clone(),
        start: 4,
        end: 7,
        expected_file_hash: hash,
        replacement: "new".to_string(),
    }];
    let generator_surface = GitWorktreeBackend
        .generator_surface_for_proposed_touches(&relpath, source, &touches)
        .expect("generator surface");
    let _ = repo_root;
    super::EditProposal {
        surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
        proposal_id: "proposal-1".to_string(),
        run_id: "run-1".to_string(),
        proposal_producer: request_policy::ProposalProducer::NonRouter,
        generator_surface,
        reported_after_file_hash: None,
        touches,
    }
}

fn identity(generation: u32, parent_id: &str, artifact_branch: &str) -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
        campaign_id: "campaign-1".to_string(),
        parent_id: parent_id.to_string(),
        node_id: parent_id.to_string(),
        generation,
        instance_id: Some("instance-1".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: format!("branch-{parent_id}"),
        artifact_branch: Some(artifact_branch.to_string()),
        created_at: "2026-04-26T00:00:00Z".to_string(),
    })
}

#[test]
fn parses_worktree_list_porcelain_output() {
    let stdout = "\
worktree /repo
HEAD abcdef
branch refs/heads/main

worktree /repo/node/worktree
HEAD 123456
branch refs/heads/prototype1-node-1
";

    let entries = parse_worktree_list(stdout);
    assert_eq!(
        entries,
        vec![
            WorktreeEntry {
                root: PathBuf::from("/repo"),
                branch: Some(super::GitBranchRef("refs/heads/main".to_string())),
            },
            WorktreeEntry {
                root: PathBuf::from("/repo/node/worktree"),
                branch: Some(super::GitBranchRef(
                    "refs/heads/prototype1-node-1".to_string(),
                )),
            },
        ]
    );
}

#[test]
fn allocates_flat_child_branch_names() {
    let backend = super::GitWorktreeBackend;
    let branch = backend.branch_name("node-1");

    assert_eq!(branch.0, "prototype1-node-1".to_string());
    assert!(!branch.0.contains('/'));
}

#[test]
fn parses_dirty_paths_from_status_output() {
    let stdout = "\
 M src/lib.rs
?? notes.txt
R  old.rs -> new.rs
";

    let paths = parse_dirty_paths(stdout);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("src/lib.rs"),
            PathBuf::from("notes.txt"),
            PathBuf::from("new.rs"),
        ]
    );
}

#[test]
fn edit_surface_bridge_accepts_single_file_proposal() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let checked = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:base-surface")),
            proposal_for(tmp.path(), relpath.clone(), "let old = 1;\n"),
        )
        .expect("single-file checked edit");

    assert_eq!(checked.target_relpath(), relpath.as_path());
    assert_eq!(
        checked.coordinate().target,
        crate::loop_graph::OperationTarget::Artifact {
            artifact_id: crate::loop_graph::ArtifactId::new("artifact:base-surface"),
        }
    );
    assert_eq!(checked.policy().as_str(), "policy:test-boundary");
    assert_eq!(checked.source_content(), "let old = 1;\n");
    assert_eq!(checked.proposed_content(), "let new = 1;\n");
    assert_eq!(checked.delta().touches().len(), 1);
    assert_ne!(checked.base_artifact_id(), checked.derived_artifact_id());

    let evidence = checked
        .surface_evidence("producer-test")
        .expect("surface evidence");
    let grant = evidence.grant.expect("checked grant evidence");
    assert_eq!(grant.policy.as_str(), "policy:test-boundary");
    assert_eq!(grant.writable.target_relpath, relpath);
    match grant.coordinate {
        crate::cli::prototype1_state::history::grant::AnyCoordinate::Checked(coordinate) => {
            assert_eq!(
                coordinate.runtime_id(),
                crate::loop_graph::RuntimeId(Uuid::nil())
            );
            assert_eq!(
                coordinate.target_artifact_id(),
                &crate::loop_graph::ArtifactId::new("artifact:base-surface")
            );
        }
        crate::cli::prototype1_state::history::grant::AnyCoordinate::Admitted(_) => {
            panic!("checked edit should keep checked grant coordinate")
        }
    }
}

#[test]
fn edit_surface_bridge_rejects_zero_touches() {
    let tmp = init_git_repo();
    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:zero-touch")),
            super::EditProposal {
                surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
                proposal_id: "proposal-1".to_string(),
                run_id: "run-1".to_string(),
                proposal_producer: request_policy::ProposalProducer::NonRouter,
                generator_surface: tui::GeneratorSurfaceVersion {
                    projection_id: "projection".to_string(),
                    projection_hash: "hash".to_string(),
                    bounds_digest: "bounds".to_string(),
                    source_kind: tui::GeneratorSourceKind::Derived,
                    source_id: "source".to_string(),
                    source_version: "v1".to_string(),
                },
                touches: Vec::new(),
                reported_after_file_hash: None,
            },
        )
        .expect_err("zero touches must reject");

    assert!(matches!(err, BackendError::EmptyEditTouches { .. }));
}

#[test]
fn edit_surface_bridge_rejects_multi_file_proposal() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let hash = super::content_hash("let old = 1;\n");
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.touches.push(super::ProposedTouch {
        target: "rag_tools".to_string(),
        relpath: PathBuf::from("crates/ploke-tui/src/rag/tools.rs"),
        start: 0,
        end: 0,
        expected_file_hash: hash,
        replacement: "x".to_string(),
    });

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:multi-file")),
            proposal,
        )
        .expect_err("multi-file proposal must reject");

    assert!(matches!(err, BackendError::MultiFileEdit { .. }));
}

#[test]
fn edit_surface_bridge_rejects_overlapping_spans() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let hash = super::content_hash("let old = 1;\n");
    let proposal = super::EditProposal {
        surface: crate::cli::Prototype1EditSurface::PlokeTuiTools,
        proposal_id: "proposal-1".to_string(),
        run_id: "run-1".to_string(),
        proposal_producer: request_policy::ProposalProducer::NonRouter,
        generator_surface: GitWorktreeBackend
            .generator_surface_for_proposed_touches(
                &relpath,
                "let old = 1;\n",
                &[
                    super::ProposedTouch {
                        target: "first".to_string(),
                        relpath: relpath.clone(),
                        start: 4,
                        end: 8,
                        expected_file_hash: hash.clone(),
                        replacement: "new".to_string(),
                    },
                    super::ProposedTouch {
                        target: "second".to_string(),
                        relpath: relpath.clone(),
                        start: 7,
                        end: 10,
                        expected_file_hash: hash.clone(),
                        replacement: "other".to_string(),
                    },
                ],
            )
            .expect("generator surface"),
        reported_after_file_hash: None,
        touches: vec![
            super::ProposedTouch {
                target: "first".to_string(),
                relpath: relpath.clone(),
                start: 4,
                end: 8,
                expected_file_hash: hash.clone(),
                replacement: "new".to_string(),
            },
            super::ProposedTouch {
                target: "second".to_string(),
                relpath,
                start: 7,
                end: 10,
                expected_file_hash: hash,
                replacement: "other".to_string(),
            },
        ],
    };

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:overlap")),
            proposal,
        )
        .expect_err("overlapping spans must reject");

    assert!(matches!(err, BackendError::OverlappingEditSpans { .. }));
}

#[test]
fn edit_surface_bridge_rejects_out_of_surface_path() {
    let tmp = init_git_repo();
    let relpath = PathBuf::from("crates/ploke-eval/src/cli.rs");
    let path = tmp.path().join(&relpath);
    fs::create_dir_all(path.parent().expect("target has parent")).expect("create target dir");
    fs::write(path, "let old = 1;\n").expect("write target");

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new(
                "artifact:out-of-surface",
            )),
            proposal_for(tmp.path(), relpath, "let old = 1;\n"),
        )
        .expect_err("out-of-surface path must reject");

    assert!(matches!(err, BackendError::OutOfEditSurface { .. }));
}

#[test]
fn workspace_except_surface_excludes_runtime_authority_paths() {
    let tmp = init_surface_repo("pub fn policy() {}\n", "same\n");
    let repo_root = tmp.path();
    for relpath in [
        ".ploke/prototype1/parent_identity.json",
        ".agents/operator.md",
        ".codex/config.md",
        ".codex-skill-staging/staged.md",
        ".tmp/scratch.md",
        ".symlinks/CatColab-symlink",
        ".cargo/config.toml",
        "docs/archive/agents/old-plan.md",
        "docs/active/bugs/alive-bug.md",
        "crates/ploke-tui/Cargo.toml",
        "target/debug/build-note.md",
        "dist/bundle.md",
    ] {
        let path = repo_root.join(relpath);
        fs::create_dir_all(path.parent().expect("authority path has parent"))
            .expect("create authority parent");
        fs::write(path, "authority\n").expect("write authority fixture");
        run_git_test(repo_root, &["add", relpath]);
    }
    let paths = super::edit_surface_paths(
        repo_root,
        crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval,
    )
    .expect("workspace surface paths");

    assert!(paths.iter().any(|path| *path == PathBuf::from("README.md")));
    assert!(
        paths
            .iter()
            .any(|path| *path == PathBuf::from("crates/ploke-tui/src/tools/code_edit.rs"))
    );
    for relpath in [
        ".ploke/prototype1/parent_identity.json",
        ".agents/operator.md",
        ".codex/config.md",
        ".codex-skill-staging/staged.md",
        ".tmp/scratch.md",
        ".symlinks/CatColab-symlink",
        ".cargo/config.toml",
        "docs/archive/agents/old-plan.md",
        "docs/active/bugs/alive-bug.md",
        "crates/ploke-tui/Cargo.toml",
        "target/debug/build-note.md",
        "dist/bundle.md",
    ] {
        assert!(
            !paths.iter().any(|path| *path == PathBuf::from(relpath)),
            "{relpath} must not be editable surface"
        );
    }
}

#[test]
fn workspace_except_surface_excludes_root_capability_files() {
    let tmp = init_surface_repo("pub fn policy() {}\n", "same\n");
    let repo_root = tmp.path();
    for relpath in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        fs::write(repo_root.join(relpath), "capability\n").expect("write capability fixture");
        run_git_test(repo_root, &["add", relpath]);
    }

    let paths = super::edit_surface_paths(
        repo_root,
        crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval,
    )
    .expect("workspace surface paths");

    for relpath in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        assert!(
            !paths.iter().any(|path| *path == PathBuf::from(relpath)),
            "{relpath} must not be editable surface"
        );
    }
}

#[test]
fn workspace_except_validation_rejects_archive_doc() {
    let tmp = init_surface_repo("pub fn policy() {}\n", "same\n");
    let relpath = PathBuf::from("docs/archive/agents/old-plan.md");
    let path = tmp.path().join(&relpath);
    fs::create_dir_all(path.parent().expect("archive path has parent"))
        .expect("create archive parent");
    fs::write(&path, "let old = 1;\n").expect("write archive fixture");
    run_git_test(
        tmp.path(),
        &["add", relpath.to_str().expect("test relpath is utf-8")],
    );
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.surface = crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval;

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:archive-doc")),
            proposal,
        )
        .expect_err("archive docs are not parent mutation surface");

    assert!(matches!(err, BackendError::OutOfEditSurface { .. }));
}

#[test]
fn workspace_except_validation_rejects_untracked_path() {
    let tmp = init_surface_repo("pub fn policy() {}\n", "same\n");
    let relpath = PathBuf::from("scratch.toml");
    fs::write(tmp.path().join(&relpath), "let old = 1;\n").expect("write untracked scratch");
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.surface = crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval;

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:untracked")),
            proposal,
        )
        .expect_err("untracked workspace file must not validate");

    assert!(matches!(err, BackendError::OutOfEditSurface { .. }));
}

#[test]
fn workspace_except_validation_rejects_parent_identity() {
    let tmp = init_surface_repo("pub fn policy() {}\n", "same\n");
    let relpath = PathBuf::from(".ploke/prototype1/parent_identity.json");
    let path = tmp.path().join(&relpath);
    fs::create_dir_all(path.parent().expect("identity path has parent"))
        .expect("create identity parent");
    fs::write(&path, "let old = 1;\n").expect("write identity fixture");
    run_git_test(
        tmp.path(),
        &["add", relpath.to_str().expect("test relpath is utf-8")],
    );
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.surface = crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval;

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new(
                "artifact:parent-identity",
            )),
            proposal,
        )
        .expect_err("parent identity is runtime authority, not editable surface");

    assert!(matches!(err, BackendError::OutOfEditSurface { .. }));
}

#[test]
fn edit_surface_bridge_rejects_prefix_path_escape() {
    let tmp = init_git_repo();
    let escaped = PathBuf::from("crates/ploke-tui/src/tools/../../../../ploke-eval/src/lib.rs");
    let resolved = tmp.path().join(&escaped);
    fs::create_dir_all(resolved.parent().expect("target has parent"))
        .expect("create escaped target dir");
    fs::write(resolved, "let old = 1;\n").expect("write escaped target");

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:path-escape")),
            proposal_for(tmp.path(), escaped, "let old = 1;\n"),
        )
        .expect_err("path escape must reject before surface prefix check");

    assert!(matches!(err, BackendError::InvalidEditSurfacePath { .. }));
}

#[test]
fn edit_surface_bridge_rejects_stale_base_hash() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.touches[0].expected_file_hash = "stale".to_string();

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:stale-base")),
            proposal,
        )
        .expect_err("stale expected hash must reject");

    assert!(matches!(err, BackendError::StaleEditBaseHash { .. }));
}

#[test]
fn edit_surface_bridge_rejects_wrong_reported_after_hash() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.reported_after_file_hash = Some("wrong-after".to_string());

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:wrong-after")),
            proposal,
        )
        .expect_err("wrong executor after hash must reject");

    assert!(matches!(err, BackendError::EditSurfaceCheck { .. }));
    assert!(err.to_string().contains("after artifact hash mismatch"));
}

#[test]
fn edit_surface_bridge_rejects_mutated_generator_surface_provenance() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let mut proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");
    proposal.generator_surface.source_version = "forged-version".to_string();

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            admission_for(crate::loop_graph::ArtifactId::new(
                "artifact:generator-surface",
            )),
            proposal,
        )
        .expect_err("mutated generator surface must reject");

    assert!(matches!(err, BackendError::EditSurfaceCheck { .. }));
    assert!(
        err.to_string()
            .contains("proposal generator surface mismatch")
    );
}

#[test]
fn edit_surface_bridge_requires_artifact_operation_target() {
    let tmp = init_git_repo();
    let relpath = write_tui_target(tmp.path(), "let old = 1;\n");
    let proposal = proposal_for(tmp.path(), relpath, "let old = 1;\n");

    let err = GitWorktreeBackend
        .validate_edit_surface_candidate(
            tmp.path(),
            EditSurfaceAdmission::new(
                crate::loop_graph::Coordinate {
                    runtime_id: crate::loop_graph::RuntimeId(Uuid::nil()),
                    target: crate::loop_graph::OperationTarget::PatchSet {
                        base_artifact_id: crate::loop_graph::ArtifactId::new("artifact:base"),
                        patch_ids: vec![crate::loop_graph::PatchId::new("patch:1")],
                    },
                },
                surface::SurfacePolicyId::new("policy:test-boundary"),
            ),
            proposal,
        )
        .expect_err("non-artifact coordinate must reject");

    assert!(matches!(err, BackendError::EditSurfaceCheck { .. }));
    assert!(
        err.to_string()
            .contains("requires OperationTarget::Artifact")
    );
}

#[test]
fn validates_tui_attempt_diff_for_allowed_candidate_change() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("src/feature.rs");
    fs::write(
        published.workspace_path().join(&changed),
        "pub fn feature() { println!(\"candidate\") }\n",
    )
    .expect("write candidate change");

    let outcome = GitWorktreeBackend
        .validate_tui_attempt(fixture.source_root.as_path(), &published)
        .expect("validate attempt diff");

    let TuiAttemptOutcome::Accepted(diff) = outcome else {
        panic!("allowed candidate change should validate");
    };
    assert_eq!(diff.source_root(), fixture.source_root.as_path());
    assert_eq!(diff.candidate_root(), published.workspace_path());
    assert_eq!(
        diff.surface(),
        crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval
    );
    assert_eq!(diff.changed_paths(), &[changed]);
    assert_eq!(
        diff.base_head(),
        &GitWorktreeBackend
            .head_commit(fixture.source_root.as_path())
            .expect("source head")
    );
}

#[test]
fn stash_to_workspace_moves_source_change_to_linked_candidate() {
    let fixture = BroadHarnessFixture::new();
    let candidate_root = fixture._temp.path().join("linked-candidate");
    run_git_test(
        fixture.source_root.as_path(),
        &[
            "worktree",
            "add",
            "--detach",
            candidate_root.to_str().expect("candidate path utf-8"),
            "HEAD",
        ],
    );

    let changed = PathBuf::from("src/feature.rs");
    fs::write(
        fixture.source_root.join(&changed),
        "pub fn feature() { println!(\"source proposal\") }\n",
    )
    .expect("write source proposal");

    let relpaths = GitWorktreeBackend
        .stash_to_workspace(
            fixture.source_root.as_path(),
            candidate_root.as_path(),
            &[fixture.source_root.join(&changed)],
            "test source proposal transfer",
        )
        .expect("stash source proposal into candidate");

    assert_eq!(relpaths, vec![changed.clone()]);
    assert_eq!(
        fs::read_to_string(fixture.source_root.join(&changed)).expect("source feature"),
        "pub fn feature() {}\n"
    );
    assert_eq!(
        fs::read_to_string(candidate_root.join(&changed)).expect("candidate feature"),
        "pub fn feature() { println!(\"source proposal\") }\n"
    );
    assert!(
        super::dirty_paths(fixture.source_root.as_path())
            .expect("source dirty paths")
            .is_empty()
    );
    assert_eq!(
        super::dirty_paths(candidate_root.as_path()).expect("candidate dirty paths"),
        vec![changed]
    );
}

#[test]
fn validates_tui_attempt_resolves_nested_source_path_to_worktree_root() {
    let fixture = BroadHarnessFixture::new();
    let source_repository = fixture.source_root.join("crates/ploke-eval");
    let published = fixture.published_request_with_source(source_repository.clone());
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("src/feature.rs");
    fs::write(
        published.workspace_path().join(&changed),
        "pub fn feature() { println!(\"candidate\") }\n",
    )
    .expect("write candidate change");

    let outcome = GitWorktreeBackend
        .validate_tui_attempt(source_repository.as_path(), &published)
        .expect("validate attempt diff from nested source path");

    let TuiAttemptOutcome::Accepted(diff) = outcome else {
        panic!("nested source path should validate against the source worktree root");
    };
    assert_eq!(diff.source_root(), fixture.source_root.as_path());
    assert_eq!(diff.candidate_root(), published.workspace_path());
    assert_eq!(diff.changed_paths(), &[changed]);
}

#[test]
fn admits_submitted_broad_harness_result_outside_protected_core() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("README.md");
    fs::write(
        published.workspace_path().join(&changed),
        "improved broad harness\n",
    )
    .expect("write candidate change");
    let submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));

    let admitted = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect("outside-protected-core change should admit");

    assert_eq!(admitted.request_id(), published.request_id());
    assert_eq!(admitted.request_hash(), published.request_hash());
    assert_eq!(admitted.changed_paths(), &[changed]);
    assert_eq!(
        admitted.workspace().source_root(),
        fixture.source_root.as_path()
    );
    assert_eq!(
        admitted.workspace().candidate_root(),
        published.workspace_path()
    );
    assert!(admitted.workspace().base_head().is_some());
    assert_eq!(
        admitted.submitted_result_path(),
        published.submitted_result_path()
    );
    assert_eq!(
        admitted.base_artifact_id(),
        &crate::loop_graph::ArtifactId::new("artifact:broad-base")
    );
    assert_ne!(admitted.base_artifact_id(), admitted.derived_artifact_id());
    assert!(
        admitted
            .derived_artifact_id()
            .as_str()
            .starts_with("artifact:git-commit:")
    );
    assert_eq!(
        admitted.coordinate(),
        admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")).coordinate()
    );
    assert_eq!(admitted.policy().as_str(), "policy:test-boundary");
    assert_eq!(
        admitted.artifact_surface(),
        &GitWorktreeBackend
            .artifact_surface(published.workspace_path())
            .expect("measure admitted artifact surface")
    );
    assert_candidate_clean(&admitted);
}

#[test]
fn admits_two_file_submitted_broad_harness_result_outside_protected_core() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let readme = PathBuf::from("README.md");
    let feature = PathBuf::from("src/feature.rs");
    fs::write(
        published.workspace_path().join(&readme),
        "improved broad harness\n",
    )
    .expect("write readme candidate change");
    fs::write(
        published.workspace_path().join(&feature),
        "pub fn feature() { println!(\"candidate\") }\n",
    )
    .expect("write feature candidate change");
    let submitted = submitted_broad_harness_result(&published, &[readme.clone(), feature.clone()]);

    let admitted = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect("two-file outside-protected-core change should admit");

    assert_eq!(admitted.request_id(), published.request_id());
    assert_eq!(admitted.request_hash(), published.request_hash());
    assert_eq!(admitted.changed_paths(), &[readme.clone(), feature.clone()]);
    assert_eq!(
        admitted.base_artifact_id(),
        &crate::loop_graph::ArtifactId::new("artifact:broad-base")
    );
    assert_ne!(admitted.base_artifact_id(), admitted.derived_artifact_id());
    assert!(
        admitted
            .derived_artifact_id()
            .as_str()
            .starts_with("artifact:git-commit:")
    );
    let evidence = admitted.child_evidence();
    assert_eq!(evidence.changed_paths(), &[readme, feature]);
    assert_eq!(
        evidence
            .artifact()
            .expect("transaction projects artifact evidence")
            .derived_artifact_id,
        admitted.derived_artifact_id().clone()
    );
    assert_eq!(
        evidence
            .workspace()
            .expect("transaction projects workspace evidence")
            .base_head
            .as_deref(),
        admitted.workspace().base_head()
    );
    assert_candidate_clean(&admitted);
}

#[test]
fn submitted_broad_harness_result_rejects_protected_core_edit() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("crates/ploke-eval/src/lib.rs");
    fs::write(
        published.workspace_path().join(&changed),
        "pub fn protected() { panic!(\"mutated\") }\n",
    )
    .expect("write protected-core mutation");
    let submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("protected-core edits must reject");

    assert!(matches!(
        err,
        BackendError::OutOfEditSurface {
            surface: crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval,
            path
        } if path == changed
    ));
    assert_eq!(
        super::dirty_paths(published.workspace_path()).expect("candidate dirty paths"),
        vec![PathBuf::from("crates/ploke-eval/src/lib.rs")]
    );
}

#[test]
fn submitted_broad_harness_result_rejects_mixed_allowed_and_protected_changes() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let allowed = PathBuf::from("README.md");
    let protected = PathBuf::from("crates/ploke-eval/src/lib.rs");
    fs::write(
        published.workspace_path().join(&allowed),
        "allowed broad harness change\n",
    )
    .expect("write allowed candidate change");
    fs::write(
        published.workspace_path().join(&protected),
        "pub fn protected() { panic!(\"mutated\") }\n",
    )
    .expect("write protected-core mutation");
    let submitted =
        submitted_broad_harness_result(&published, &[allowed.clone(), protected.clone()]);

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("mixed allowed/protected edits must reject");

    assert!(matches!(
        err,
        BackendError::OutOfEditSurface {
            surface: crate::cli::Prototype1EditSurface::WorkspaceExceptPlokeEval,
            path
        } if path == protected
    ));
    let dirty = super::dirty_paths(published.workspace_path()).expect("candidate dirty paths");
    assert!(dirty.contains(&allowed));
    assert!(dirty.contains(&protected));
}

#[test]
fn submitted_broad_harness_result_rejects_stale_base() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    fs::write(fixture.source_root.join("README.md"), "source advanced\n")
        .expect("advance source repo");
    run_git_test(fixture.source_root.as_path(), &["add", "README.md"]);
    run_git_test(
        fixture.source_root.as_path(),
        &["commit", "--no-gpg-sign", "-m", "advance source"],
    );

    let changed = PathBuf::from("src/feature.rs");
    fs::write(
        published.workspace_path().join(&changed),
        "pub fn feature() { println!(\"candidate\") }\n",
    )
    .expect("write candidate change");
    let submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("stale candidate base must reject");

    assert!(
        matches!(err, BackendError::BroadHarnessStaleBase { path, .. } if path == published.workspace_path())
    );
}

#[test]
fn submitted_broad_harness_result_rejects_request_mismatch() {
    let fixture = BroadHarnessFixture::new();
    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("README.md");
    fs::write(
        published.workspace_path().join(&changed),
        "tampered request\n",
    )
    .expect("write candidate change");
    let mut submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));
    submitted.request.request_hash = "tampered-request-hash".to_string();

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("request mismatch must reject");

    assert!(matches!(
        err,
        BackendError::BroadHarnessRequestBinding { detail }
            if detail.contains("request_hash mismatch")
                && detail.contains("tampered-request-hash")
    ));
}

#[test]
fn broad_harness_admission_preflights_surface_before_commit() {
    let fixture = BroadHarnessFixture::new();
    for relpath in super::tool_description_paths() {
        fs::remove_file(fixture.source_root.join(&relpath))
            .unwrap_or_else(|err| panic!("remove tool description '{relpath:?}': {err}"));
    }
    run_git_test(fixture.source_root.as_path(), &["add", "-u"]);
    run_git_test(
        fixture.source_root.as_path(),
        &[
            "commit",
            "--no-gpg-sign",
            "-m",
            "remove tool description artifacts",
        ],
    );

    let published = fixture.published_request();
    fixture.clone_candidate_workspace(&published);
    let base_head = GitWorktreeBackend
        .head_commit(published.workspace_path())
        .expect("candidate base head");

    let changed = PathBuf::from("README.md");
    fs::write(
        published.workspace_path().join(&changed),
        "improved broad harness\n",
    )
    .expect("write candidate change");
    let submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.source_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("missing artifact-surface inputs must reject before commit");

    assert!(matches!(err, BackendError::MissingSurfaceFile { .. }));
    assert_eq!(
        GitWorktreeBackend
            .head_commit(published.workspace_path())
            .expect("candidate head after rejected admission"),
        base_head,
        "admission must not commit before all artifact-surface inputs are known good"
    );
    assert_eq!(
        super::dirty_paths(published.workspace_path()).expect("candidate dirty paths"),
        vec![changed]
    );
}

#[test]
fn submitted_broad_harness_result_rejects_live_admission_binding_mismatch_before_repo_checks() {
    let fixture = BroadHarnessFixture::new();
    let mut published = fixture.published_request();
    published.admission_binding = RequestAdmissionBinding::new(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId(Uuid::nil()),
            target: crate::loop_graph::OperationTarget::Artifact {
                artifact_id: crate::loop_graph::ArtifactId::new("artifact:published"),
            },
        },
        crate::loop_graph::ArtifactId::new("artifact:published"),
        "policy:published-boundary",
    )
    .expect("construct mismatched published binding");
    fixture.clone_candidate_workspace(&published);

    let changed = PathBuf::from("README.md");
    fs::write(
        published.workspace_path().join(&changed),
        "improved broad harness\n",
    )
    .expect("write candidate change");
    let submitted = submitted_broad_harness_result(&published, std::slice::from_ref(&changed));

    let err = GitWorktreeBackend
        .admit_submitted_broad_harness_result(
            fixture.prototype_root.as_path(),
            admission_for(crate::loop_graph::ArtifactId::new("artifact:broad-base")),
            &published,
            &submitted,
        )
        .expect_err("published binding mismatch must reject before repo checks");

    assert!(matches!(
        err,
        BackendError::BroadHarnessRequestBinding { detail }
            if detail.contains("published request admission binding mismatch")
                && detail.contains("artifact:published")
                && detail.contains("policy:published-boundary")
    ));
}

#[test]
fn describe_submitted_broad_harness_result_error_covers_request_admission_binding_mismatch() {
    let expected = RequestAdmissionBinding::new(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId(Uuid::nil()),
            target: crate::loop_graph::OperationTarget::Artifact {
                artifact_id: crate::loop_graph::ArtifactId::new("artifact:expected"),
            },
        },
        crate::loop_graph::ArtifactId::new("artifact:expected"),
        "policy:expected",
    )
    .expect("construct expected binding");
    let actual = RequestAdmissionBinding::new(
        crate::loop_graph::Coordinate {
            runtime_id: crate::loop_graph::RuntimeId(Uuid::nil()),
            target: crate::loop_graph::OperationTarget::Artifact {
                artifact_id: crate::loop_graph::ArtifactId::new("artifact:actual"),
            },
        },
        crate::loop_graph::ArtifactId::new("artifact:actual"),
        "policy:actual",
    )
    .expect("construct actual binding");

    let detail = describe_submitted_broad_harness_result_error(
        &SubmittedBroadHarnessResultError::RequestAdmissionBindingMismatch { expected, actual },
    );

    assert!(detail.contains("request_admission_binding mismatch"));
    assert!(detail.contains("artifact:expected"));
    assert!(detail.contains("artifact:actual"));
}

#[test]
fn validates_fresh_gen0_parent_checkout() {
    let tmp = init_git_repo();
    let repo_root = tmp.path();
    let backend = GitWorktreeBackend;
    let branch = "prototype1-parent-gen0";

    backend
        .checkout_fresh_parent_branch(repo_root, branch)
        .expect("fresh branch");
    let identity = identity(0, "node-0", branch);
    write_parent_identity(repo_root, &identity).expect("write identity");
    backend
        .persist_active_checkout_files(
            repo_root,
            &[parent_identity_relpath()],
            &parent_identity_commit_message(&identity),
        )
        .expect("commit identity");

    backend
        .validate_parent_checkout(repo_root, &identity)
        .expect("gen0 parent checkout");
}

#[test]
fn rejects_contaminated_gen0_parent_branch() {
    let tmp = init_git_repo();
    let repo_root = tmp.path();
    let backend = GitWorktreeBackend;
    let branch = "prototype1-parent-gen0";

    backend
        .checkout_fresh_parent_branch(repo_root, branch)
        .expect("fresh branch");
    let identity = identity(0, "node-0", branch);
    write_parent_identity(repo_root, &identity).expect("write identity");
    backend
        .persist_active_checkout_files(
            repo_root,
            &[parent_identity_relpath()],
            &parent_identity_commit_message(&identity),
        )
        .expect("commit identity");
    fs::write(repo_root.join("contamination.txt"), "not parent identity\n")
        .expect("write contamination");
    run_git_test(repo_root, &["add", "contamination.txt"]);
    run_git_test(
        repo_root,
        &["commit", "--no-gpg-sign", "-m", "unexpected follow-up"],
    );

    let err = backend
        .validate_parent_checkout(repo_root, &identity)
        .expect_err("contaminated gen0 branch should reject");
    assert!(err.to_string().contains("does not match expected"));
}

#[test]
fn validates_gen1_parent_checkout_after_artifact_commit() {
    let tmp = init_git_repo();
    let repo_root = tmp.path();
    let backend = GitWorktreeBackend;
    let branch = "prototype1-node-1";

    run_git_test(repo_root, &["switch", "-c", branch]);
    fs::write(repo_root.join("target.txt"), "artifact\n").expect("write artifact");
    run_git_test(repo_root, &["add", "target.txt"]);
    run_git_test(
        repo_root,
        &[
            "commit",
            "--no-gpg-sign",
            "-m",
            "prototype1: persist buildable artifact for node node-1",
        ],
    );
    let identity = identity(1, "node-1", branch);
    write_parent_identity(repo_root, &identity).expect("write identity");
    backend
        .persist_active_checkout_files(
            repo_root,
            &[parent_identity_relpath()],
            &parent_identity_commit_message(&identity),
        )
        .expect("commit identity");

    backend
        .validate_parent_checkout(repo_root, &identity)
        .expect("gen1 parent checkout");
}

#[test]
fn surface_commitment_allows_tool_text_mutation() {
    let before = init_surface_repo("pub fn policy() {}\n", "before\n");
    let after = init_surface_repo("pub fn policy() {}\n", "after\n");
    let backend = GitWorktreeBackend;

    backend
        .surface_commitment(before.path(), after.path())
        .expect("tool text mutation preserves immutable surface");
}

#[test]
fn surface_commitment_represents_ploke_tui_tool_mutation() {
    let before = init_surface_repo("pub fn policy() {}\n", "same\n");
    let after = init_surface_repo("pub fn policy() {}\n", "same\n");
    fs::write(
        after.path().join("crates/ploke-tui/src/tools/code_edit.rs"),
        "changed tui tool\n",
    )
    .expect("mutate tui tool file");
    let backend = GitWorktreeBackend;

    let unchanged = backend
        .surface_commitment(before.path(), before.path())
        .expect("unchanged surface");
    let changed = backend
        .surface_commitment(before.path(), after.path())
        .expect("tui tool mutation is represented");

    assert_ne!(unchanged, changed);
}

#[cfg(unix)]
#[test]
fn surface_commitment_hashes_tracked_symlink_to_directory() {
    let before = init_surface_repo("pub fn policy() {}\n", "same\n");
    let after = init_surface_repo("pub fn policy() {}\n", "same\n");
    for root in [before.path(), after.path()] {
        let symlink_dir = root.join(".symlinks");
        let linked_dir = root.join("linked-dir");
        fs::create_dir_all(&symlink_dir).expect("create symlink dir");
        fs::create_dir_all(&linked_dir).expect("create linked dir");
        std::os::unix::fs::symlink(&linked_dir, symlink_dir.join("directory-link"))
            .expect("create directory symlink");
        run_git_test(root, &["add", ".symlinks/directory-link"]);
        run_git_test(
            root,
            &["commit", "--no-gpg-sign", "-m", "tracked directory symlink"],
        );
    }
    let backend = GitWorktreeBackend;

    backend
        .surface_commitment(before.path(), after.path())
        .expect("surface commitment handles tracked directory symlink");
}

#[test]
fn surface_commitment_rejects_eval_mutation() {
    let before = init_surface_repo("pub fn policy() {}\n", "same\n");
    let after = init_surface_repo("pub fn policy_changed() {}\n", "same\n");
    let backend = GitWorktreeBackend;

    let err = backend
        .surface_commitment(before.path(), after.path())
        .expect_err("ploke-eval mutation must reject ordinary succession");
    assert!(matches!(err, BackendError::ImmutableSurfaceChanged { .. }));
}

#[test]
fn surface_commitment_rejects_missing_eval_surface() {
    let before = init_git_repo();
    let after = init_surface_repo("pub fn policy() {}\n", "same\n");
    let backend = GitWorktreeBackend;

    let err = backend
        .surface_commitment(before.path(), after.path())
        .expect_err("missing ploke-eval surface must reject ordinary succession");
    assert!(matches!(err, BackendError::EmptySurfacePathspec { .. }));
}
