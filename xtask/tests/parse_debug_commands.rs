//! Tests for `parse debug` path diagnostics (logical-paths, modules-premerge, path-collisions).

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use tempfile::TempDir;
use xtask::commands::parse::ParseOutput;
use xtask::commands::parse_debug::{
    DebugCargoTargets, DebugCorpus, DebugDiscoveryRules, DebugLogicalPaths, DebugModulesPremerge,
    DebugOutput, DebugPathCollisions, DebugWorkspaceMembers, ParseDebugCli, ParseDebugCmd,
};
use xtask::context::CommandContext;
use xtask::executor::Command as _;

const FIXTURE_NODES: &str = "tests/fixture_crates/fixture_nodes";
const FIXTURE_WORKSPACE: &str = "tests/fixture_workspace/fixture_mock_serde";
const FIXTURE_GITHUB_SERDE_WORKSPACE: &str = "tests/fixture_github_clones/serde";

#[test]
fn parse_debug_logical_paths_succeeds_on_fixture() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::LogicalPaths(DebugLogicalPaths {
            path: PathBuf::from(FIXTURE_NODES),
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug logical-paths");
    match out {
        ParseOutput::Debug(DebugOutput::LogicalPaths(o)) => {
            assert!(!o.crate_root.is_empty());
            assert!(o.file_count > 0, "expected discovered files: {o:#?}");
            assert_eq!(o.files.len(), o.file_count);
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_modules_premerge_succeeds_on_fixture() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::ModulesPremerge(DebugModulesPremerge {
            path: PathBuf::from(FIXTURE_NODES),
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug modules-premerge");
    match out {
        ParseOutput::Debug(DebugOutput::ModulesPremerge(o)) => {
            assert!(o.graph_count > 0);
            assert_eq!(o.graphs.len(), o.graph_count);
            assert!(o.total_module_nodes > 0);
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_path_collisions_succeeds_on_fixture() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::PathCollisions(DebugPathCollisions {
            path: PathBuf::from(FIXTURE_NODES),
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug path-collisions");
    match out {
        ParseOutput::Debug(DebugOutput::PathCollisions(o)) => {
            assert!(o.merged_module_count > 0);
            for g in &o.collisions {
                assert!(
                    g.modules.len() > 1,
                    "collision group must have 2+ modules: {g:#?}"
                );
            }
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_cargo_targets_succeeds_on_fixture_workspace() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::CargoTargets(DebugCargoTargets {
            path: PathBuf::from(FIXTURE_WORKSPACE),
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug cargo-targets");
    match out {
        ParseOutput::Debug(DebugOutput::CargoTargets(o)) => {
            assert!(!o.workspace_root.is_empty());
            assert!(o.package_count > 0, "expected one or more packages");
            assert!(!o.packages.is_empty(), "expected package summaries");
            assert!(
                o.packages.iter().any(|p| !p.targets.is_empty()),
                "expected at least one package with targets"
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_workspace_members_classify_reports_tests_only() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::WorkspaceMembers(DebugWorkspaceMembers {
            path: PathBuf::from(FIXTURE_GITHUB_SERDE_WORKSPACE),
            classify: true,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug workspace-members --classify");
    match out {
        ParseOutput::Debug(DebugOutput::WorkspaceMembers(o)) => {
            assert!(o.member_count > 0, "workspace should expose members");
            assert_eq!(o.members.len(), o.member_count);
            assert!(
                o.members
                    .iter()
                    .any(|m| m.classification.as_deref() == Some("tests_only")),
                "expected at least one tests_only member in serde fixture workspace"
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_discovery_rules_reports_src_and_manifest_targets() {
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::DiscoveryRules(DebugDiscoveryRules {
            path: PathBuf::from(FIXTURE_NODES),
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug discovery-rules");
    match out {
        ParseOutput::Debug(DebugOutput::DiscoveryRules(o)) => {
            assert!(
                o.candidate_sources.iter().any(|c| c.source == "src_walk"),
                "expected src_walk candidate"
            );
            assert!(!o.include_rules.is_empty(), "expected include rules");
            assert!(!o.exclusion_rules.is_empty(), "expected exclusion rules");
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_clones_and_parses_local_git_repo() {
    let temp = TempDir::new().expect("tempdir");
    let source_repo = temp.path().join("mini_repo");
    init_local_git_crate(&source_repo);

    let list_file = temp.path().join("targets.txt");
    std::fs::write(&list_file, format!("{}\n", source_repo.display())).expect("write list file");

    let checkout_dir = temp.path().join("checkouts");
    let artifact_dir = temp.path().join("artifacts");
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::Corpus(DebugCorpus {
            list_files: vec![list_file],
            checkout_dir,
            artifact_dir,
            limit: 0,
            skip_clone: false,
            skip_merge: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug corpus");
    match out {
        ParseOutput::Debug(DebugOutput::Corpus(o)) => {
            assert_eq!(o.processed_targets, 1, "expected one processed target");
            assert_eq!(o.single_crate_targets, 1, "expected one crate target");
            assert_eq!(o.workspace_targets, 0, "did not expect workspace targets");
            assert_eq!(o.clone_failures, 0, "unexpected clone failure: {o:#?}");
            assert_eq!(
                o.discovery_failures, 0,
                "unexpected discovery failure: {o:#?}"
            );
            assert_eq!(o.resolve_failures, 0, "unexpected resolve failure: {o:#?}");
            assert_eq!(o.merge_failures, 0, "unexpected merge failure: {o:#?}");
            let target = o.targets.first().expect("one target result");
            assert!(target.clone.ok, "clone status should be ok: {target:#?}");
            assert!(!o.artifact_root.is_empty());
            assert!(!target.artifact_dir.is_empty());
            assert_eq!(target.repository_kind, "single_crate");
            assert_eq!(target.recommended_parser, "try_run_phases_and_merge");
            assert!(
                target.discovery.as_ref().is_some_and(|d| d.ok),
                "discovery should succeed: {target:#?}"
            );
            assert!(
                target.resolve.as_ref().is_some_and(|r| r.ok),
                "resolve should succeed: {target:#?}"
            );
            assert!(
                target.merge.as_ref().is_some_and(|m| m.ok),
                "merge should succeed: {target:#?}"
            );
            assert!(target.commit_sha.is_some(), "expected HEAD commit SHA");
            assert!(Path::new(&o.artifact_root).join("summary.json").is_file());
            assert!(
                target
                    .summary_path
                    .as_deref()
                    .is_some_and(|p| Path::new(p).is_file())
            );
            assert!(
                target
                    .discovery
                    .as_ref()
                    .and_then(|stage| stage.artifact_path.as_deref())
                    .is_some_and(|p| Path::new(p).is_file())
            );
            assert!(
                target
                    .resolve
                    .as_ref()
                    .and_then(|stage| stage.artifact_path.as_deref())
                    .is_some_and(|p| Path::new(p).is_file())
            );
            assert!(
                target
                    .merge
                    .as_ref()
                    .and_then(|stage| stage.artifact_path.as_deref())
                    .is_some_and(|p| Path::new(p).is_file())
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_classifies_workspace_repo_without_running_single_crate_pipeline() {
    let temp = TempDir::new().expect("tempdir");
    let source_repo = temp.path().join("mini_workspace");
    init_local_git_workspace(&source_repo);

    let list_file = temp.path().join("targets_workspace.txt");
    std::fs::write(&list_file, format!("{}\n", source_repo.display())).expect("write list file");

    let checkout_dir = temp.path().join("checkouts");
    let artifact_dir = temp.path().join("artifacts");
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::Corpus(DebugCorpus {
            list_files: vec![list_file],
            checkout_dir,
            artifact_dir,
            limit: 0,
            skip_clone: false,
            skip_merge: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug corpus workspace");
    match out {
        ParseOutput::Debug(DebugOutput::Corpus(o)) => {
            assert_eq!(o.processed_targets, 1, "expected one processed target");
            assert_eq!(o.single_crate_targets, 0, "did not expect crate targets");
            assert_eq!(o.workspace_targets, 1, "expected one workspace target");
            assert_eq!(o.clone_failures, 0, "unexpected clone failure: {o:#?}");
            assert_eq!(
                o.discovery_failures, 0,
                "workspace should not hit discovery"
            );
            assert_eq!(o.resolve_failures, 0, "workspace should not hit resolve");
            assert_eq!(o.merge_failures, 0, "workspace should not hit merge");
            let target = o.targets.first().expect("one target result");
            assert_eq!(target.repository_kind, "workspace");
            assert_eq!(target.recommended_parser, "parse_workspace_with_config");
            assert_eq!(target.workspace_member_count, Some(1));
            assert!(target.classification_error.is_none(), "{target:#?}");
            assert!(target.discovery.is_none(), "{target:#?}");
            assert!(target.resolve.is_none(), "{target:#?}");
            assert!(target.merge.is_none(), "{target:#?}");
            assert!(
                target
                    .summary_path
                    .as_deref()
                    .is_some_and(|p| Path::new(p).is_file())
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_classifies_implicit_workspace_members_via_metadata_fallback() {
    let temp = TempDir::new().expect("tempdir");
    let source_repo = temp.path().join("implicit_workspace");
    init_local_git_implicit_workspace(&source_repo);

    let list_file = temp.path().join("targets_implicit_workspace.txt");
    std::fs::write(&list_file, format!("{}\n", source_repo.display())).expect("write list file");

    let checkout_dir = temp.path().join("checkouts");
    let artifact_dir = temp.path().join("artifacts");
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::Corpus(DebugCorpus {
            list_files: vec![list_file],
            checkout_dir,
            artifact_dir,
            limit: 0,
            skip_clone: false,
            skip_merge: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug corpus implicit workspace");
    match out {
        ParseOutput::Debug(DebugOutput::Corpus(o)) => {
            assert_eq!(o.processed_targets, 1, "expected one processed target");
            assert_eq!(o.single_crate_targets, 0, "did not expect crate targets");
            assert_eq!(o.workspace_targets, 1, "expected one workspace target");
            assert_eq!(o.clone_failures, 0, "unexpected clone failure: {o:#?}");
            assert_eq!(
                o.discovery_failures, 0,
                "workspace should not hit discovery"
            );
            assert_eq!(o.resolve_failures, 0, "workspace should not hit resolve");
            assert_eq!(o.merge_failures, 0, "workspace should not hit merge");
            let target = o.targets.first().expect("one target result");
            assert_eq!(target.repository_kind, "workspace");
            assert_eq!(target.recommended_parser, "parse_workspace_with_config");
            assert_eq!(target.workspace_member_count, Some(2));
            assert!(target.classification_error.is_none(), "{target:#?}");
            assert!(target.discovery.is_none(), "{target:#?}");
            assert!(target.resolve.is_none(), "{target:#?}");
            assert!(target.merge.is_none(), "{target:#?}");
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

fn init_local_git_crate(repo_dir: &Path) {
    std::fs::create_dir_all(repo_dir.join("src")).expect("create src dir");
    std::fs::write(
        repo_dir.join("Cargo.toml"),
        r#"[package]
name = "mini_repo"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"
"#,
    )
    .expect("write Cargo.toml");
    std::fs::write(
        repo_dir.join("src/lib.rs"),
        "pub fn answer() -> usize { 42 }\n",
    )
    .expect("write lib.rs");

    run_git(repo_dir, &["init"]);
    run_git(repo_dir, &["add", "."]);
    run_git(
        repo_dir,
        &[
            "-c",
            "user.name=ploke-test",
            "-c",
            "user.email=ploke@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );
}

fn init_local_git_workspace(repo_dir: &Path) {
    std::fs::create_dir_all(repo_dir.join("member/src")).expect("create workspace member src");
    std::fs::write(
        repo_dir.join("Cargo.toml"),
        r#"[workspace]
members = ["member"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");
    std::fs::write(
        repo_dir.join("member/Cargo.toml"),
        r#"[package]
name = "member"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write member Cargo.toml");
    std::fs::write(
        repo_dir.join("member/src/lib.rs"),
        "pub fn workspace_member() -> usize { 7 }\n",
    )
    .expect("write member lib.rs");

    run_git(repo_dir, &["init"]);
    run_git(repo_dir, &["add", "."]);
    run_git(
        repo_dir,
        &[
            "-c",
            "user.name=ploke-test",
            "-c",
            "user.email=ploke@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );
}

fn init_local_git_implicit_workspace(repo_dir: &Path) {
    std::fs::create_dir_all(repo_dir.join("helper/src")).expect("create helper src");
    std::fs::create_dir_all(repo_dir.join("excluded/src")).expect("create excluded src");
    std::fs::create_dir_all(repo_dir.join("src")).expect("create root src");
    std::fs::write(
        repo_dir.join("Cargo.toml"),
        r#"[package]
name = "implicit_workspace_root"
version = "0.1.0"
edition = "2024"

[dependencies]
helper = { path = "helper" }

[workspace]
exclude = ["excluded"]
"#,
    )
    .expect("write implicit workspace Cargo.toml");
    std::fs::write(
        repo_dir.join("src/lib.rs"),
        "pub fn root_answer() -> usize { helper::helper_answer() }\n",
    )
    .expect("write root lib.rs");
    std::fs::write(
        repo_dir.join("helper/Cargo.toml"),
        r#"[package]
name = "helper"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write helper Cargo.toml");
    std::fs::write(
        repo_dir.join("helper/src/lib.rs"),
        "pub fn helper_answer() -> usize { 5 }\n",
    )
    .expect("write helper lib.rs");
    std::fs::write(
        repo_dir.join("excluded/Cargo.toml"),
        r#"[package]
name = "excluded"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write excluded Cargo.toml");
    std::fs::write(
        repo_dir.join("excluded/src/lib.rs"),
        "pub fn excluded_answer() -> usize { 9 }\n",
    )
    .expect("write excluded lib.rs");

    run_git(repo_dir, &["init"]);
    run_git(repo_dir, &["add", "."]);
    run_git(
        repo_dir,
        &[
            "-c",
            "user.name=ploke-test",
            "-c",
            "user.email=ploke@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );
}

fn run_git(repo_dir: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {:?} failed with {status}", args);
}
