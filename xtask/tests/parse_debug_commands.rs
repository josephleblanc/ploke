//! Tests for `parse debug` path diagnostics (logical-paths, modules-premerge, path-collisions).

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use tempfile::TempDir;
use xtask::commands::parse::ParseOutput;
use xtask::commands::parse_debug::{
    CorpusWorkspaceMode, DebugCargoTargets, DebugCorpus, DebugCorpusShow, DebugCorpusTriage,
    DebugDiscoveryRules, DebugLogicalPaths, DebugModulesPremerge, DebugOutput, DebugPathCollisions,
    DebugWorkspaceMembers, ParseDebugCli, ParseDebugCmd,
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
            workspace_mode: CorpusWorkspaceMode::Skip,
            resolve_timeout_minutes: 0,
            merge_timeout_minutes: 0,
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
fn parse_debug_corpus_show_filters_workspace_members() {
    let temp = TempDir::new().expect("tempdir");
    let run_dir = temp.path().join("run-123");
    std::fs::create_dir_all(&run_dir).expect("create run dir");
    let summary_path = run_dir.join("summary.json");
    let summary_json = format!(
        r#"{{
  "kind": "corpus",
  "run_id": "run-123",
  "checkout_root": "/tmp/checkouts",
  "artifact_root": "{}",
  "workspace_mode": "probe",
  "list_files": ["/tmp/list-a.txt"],
  "requested_entries": 3,
  "unique_targets": 3,
  "processed_targets": 1,
  "single_crate_targets": 0,
  "workspace_targets": 1,
  "reused_targets": 1,
  "cloned_targets": 0,
  "skipped_targets": 0,
  "clone_failures": 0,
  "discovery_failures": 0,
  "resolve_failures": 2,
  "merge_failures": 0,
  "panic_failures": 1,
  "targets": [{{
    "target": "workspace/fail",
    "normalized_repo": "workspace/fail",
    "clone_url": "https://github.com/workspace/fail.git",
    "datasets": ["list-a"],
    "checkout_path": "/tmp/checkouts/workspace__fail",
    "artifact_dir": "/tmp/artifacts/workspace__fail",
    "repository_kind": "workspace",
    "recommended_parser": "parse_workspace_with_config",
    "workspace_member_count": 2,
    "classification_error": null,
    "classification_diagnostic": null,
    "clone": {{ "ok": true, "action": "reused", "error": null }},
    "commit_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "discovery": null,
    "resolve": null,
    "merge": null,
    "summary_path": "/tmp/artifacts/workspace__fail/summary.json",
    "workspace_probe": {{
      "workspace_root": "/tmp/checkouts/workspace__fail",
      "member_count": 2,
      "failed_members": 2,
      "summary_path": "/tmp/artifacts/workspace__fail/workspace_probe/workspace_summary.json",
      "members": [
        {{
          "path": "/tmp/checkouts/workspace__fail/gamma",
          "label": "gamma",
          "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma",
          "discovery": {{ "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null }},
          "resolve": {{ "ok": false, "panic": false, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "error", "error": "resolve failed", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma/resolve/failure.json" }},
          "merge": null
        }},
        {{
          "path": "/tmp/checkouts/workspace__fail/delta",
          "label": "delta",
          "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/001_delta",
          "discovery": {{ "ok": true, "panic": false, "duration_ms": 3, "file_count": 4, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null }},
          "resolve": {{ "ok": false, "panic": true, "duration_ms": 9, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "panic", "error": "panic payload", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/001_delta/resolve/failure.json" }},
          "merge": null
        }}
      ]
    }}
  }}]
}}"#,
        run_dir.display()
    );
    std::fs::write(&summary_path, summary_json).expect("write summary");

    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusShow(DebugCorpusShow {
            run: summary_path,
            artifact_dir: temp.path().join("artifacts"),
            target: Some("workspace/fail".into()),
            member: Some("gamma".into()),
            backtrace: false,
            backtrace_full: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug corpus-show");
    match out {
        ParseOutput::Debug(DebugOutput::CorpusShow(o)) => {
            assert_eq!(o.selected_target.as_deref(), Some("workspace/fail"));
            assert_eq!(o.selected_member.as_deref(), Some("gamma"));
            assert_eq!(o.run.processed_targets, 1);
            assert_eq!(o.run.workspace_targets, 1);
            assert_eq!(o.run.resolve_failures, 1);
            assert_eq!(o.run.panic_failures, 0);
            let target = o.run.targets.first().expect("one target");
            let probe = target.workspace_probe.as_ref().expect("workspace probe");
            assert_eq!(probe.member_count, 1);
            assert_eq!(probe.failed_members, 1);
            assert_eq!(probe.members.len(), 1);
            assert_eq!(probe.members[0].label, "gamma");
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_triage_indexes_failures_and_writes_cluster_stubs() {
    let temp = TempDir::new().expect("tempdir");
    let run_dir = temp.path().join("run-555");
    std::fs::create_dir_all(&run_dir).expect("create run dir");
    let summary_path = run_dir.join("summary.json");
    let summary_json = format!(
        r#"{{
  "kind": "corpus",
  "run_id": "run-555",
  "checkout_root": "/tmp/checkouts",
  "artifact_root": "{}",
  "workspace_mode": "probe",
  "list_files": ["/tmp/list-a.txt"],
  "requested_entries": 4,
  "unique_targets": 4,
  "processed_targets": 2,
  "single_crate_targets": 1,
  "workspace_targets": 1,
  "reused_targets": 2,
  "cloned_targets": 0,
  "skipped_targets": 0,
  "clone_failures": 0,
  "discovery_failures": 0,
  "resolve_failures": 3,
  "merge_failures": 0,
  "panic_failures": 2,
  "targets": [
    {{
      "target": "single/fail",
      "normalized_repo": "single/fail",
      "clone_url": "https://github.com/single/fail.git",
      "datasets": ["list-a"],
      "checkout_path": "/tmp/checkouts/single__fail",
      "artifact_dir": "/tmp/artifacts/single__fail",
      "repository_kind": "single_crate",
      "recommended_parser": "try_run_phases_and_merge",
      "workspace_member_count": null,
      "classification_error": null,
      "classification_diagnostic": null,
      "clone": {{ "ok": true, "action": "reused", "error": null }},
      "commit_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "discovery": {{ "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null }},
      "resolve": {{ "ok": false, "panic": true, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "panic", "error": "thread 'worker' panicked at Duplicate Macro node ID at crates/somewhere.rs:9:1", "failure_artifact_path": "/tmp/artifacts/single__fail/resolve/failure.json" }},
      "merge": null,
      "summary_path": "/tmp/artifacts/single__fail/summary.json",
      "workspace_probe": null
    }},
    {{
      "target": "workspace/fail",
      "normalized_repo": "workspace/fail",
      "clone_url": "https://github.com/workspace/fail.git",
      "datasets": ["list-a"],
      "checkout_path": "/tmp/checkouts/workspace__fail",
      "artifact_dir": "/tmp/artifacts/workspace__fail",
      "repository_kind": "workspace",
      "recommended_parser": "parse_workspace_with_config",
      "workspace_member_count": 2,
      "classification_error": null,
      "classification_diagnostic": null,
      "clone": {{ "ok": true, "action": "reused", "error": null }},
      "commit_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "discovery": null,
      "resolve": null,
      "merge": null,
      "summary_path": "/tmp/artifacts/workspace__fail/summary.json",
      "workspace_probe": {{
        "workspace_root": "/tmp/checkouts/workspace__fail",
        "member_count": 2,
        "failed_members": 2,
        "summary_path": "/tmp/artifacts/workspace__fail/workspace_probe/workspace_summary.json",
        "members": [
          {{
            "path": "/tmp/checkouts/workspace__fail/gamma",
            "label": "gamma",
            "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma",
            "discovery": {{ "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null }},
            "resolve": {{ "ok": false, "panic": true, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "panic", "error": "thread 'worker' panicked at Duplicate Macro node ID at crates/elsewhere.rs:3:7", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma/resolve/failure.json" }},
            "merge": null
          }},
          {{
            "path": "/tmp/checkouts/workspace__fail/delta",
            "label": "delta",
            "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/001_delta",
            "discovery": {{ "ok": true, "panic": false, "duration_ms": 3, "file_count": 4, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null }},
            "resolve": {{ "ok": false, "panic": false, "duration_ms": 9, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "error", "error": "partial parsing: 28 succeeded, 3 failed", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/001_delta/resolve/failure.json" }},
            "merge": null
          }}
        ]
      }}
    }}
  ]
}}"#,
        run_dir.display()
    );
    std::fs::write(&summary_path, summary_json).expect("write summary");

    let expected_summary_path = summary_path.display().to_string();
    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusTriage(DebugCorpusTriage {
            run: summary_path,
            artifact_dir: temp.path().join("artifacts"),
            out_dir: None,
            no_report_stubs: false,
            watch: false,
            interval_secs: 10,
            exit_when_complete: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd.execute(&ctx).expect("parse debug corpus-triage");
    match out {
        ParseOutput::Debug(DebugOutput::CorpusTriage(o)) => {
            assert_eq!(o.run_id, "run-555");
            assert_eq!(o.snapshot_mode, "complete");
            assert_eq!(
                o.summary_path.as_deref(),
                Some(expected_summary_path.as_str())
            );
            assert_eq!(o.failure_count, 3);
            assert_eq!(o.cluster_count, 2);
            assert_eq!(o.pending_report_count, 2);
            assert!(Path::new(&o.triage_dir).join("index.json").is_file());
            assert!(Path::new(&o.failures_path).is_file());
            assert!(Path::new(&o.clusters_path).is_file());
            assert!(Path::new(&o.report_template_path).is_file());
            assert!(Path::new(&o.pending_report_dir).is_dir());

            let macro_cluster = o
                .clusters
                .iter()
                .find(|cluster| cluster.error_signature == "Duplicate Macro node ID")
                .expect("macro cluster");
            assert_eq!(macro_cluster.count, 2);
            assert!(
                macro_cluster
                    .pending_report_path
                    .as_deref()
                    .is_some_and(|p| Path::new(p).is_file())
            );

            let jsonl = std::fs::read_to_string(&o.failures_path).expect("read failures jsonl");
            assert_eq!(jsonl.lines().count(), 3);
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_triage_reads_in_progress_run_without_top_level_summary() {
    let temp = TempDir::new().expect("tempdir");
    let run_dir = temp.path().join("run-777");
    let target_dir = run_dir.join("workspace__fail");
    std::fs::create_dir_all(&target_dir).expect("create target dir");
    let target_summary_path = target_dir.join("summary.json");
    std::fs::write(
        &target_summary_path,
        r#"{
  "target": "workspace/fail",
  "normalized_repo": "workspace/fail",
  "clone_url": "https://github.com/workspace/fail.git",
  "datasets": ["list-a"],
  "checkout_path": "/tmp/checkouts/workspace__fail",
  "artifact_dir": "/tmp/artifacts/workspace__fail",
  "repository_kind": "workspace",
  "recommended_parser": "parse_workspace_with_config",
  "workspace_member_count": 1,
  "classification_error": null,
  "classification_diagnostic": null,
  "clone": { "ok": true, "action": "reused", "error": null },
  "commit_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "discovery": null,
  "resolve": null,
  "merge": null,
  "summary_path": "/tmp/artifacts/workspace__fail/summary.json",
  "workspace_probe": {
    "workspace_root": "/tmp/checkouts/workspace__fail",
    "member_count": 1,
    "failed_members": 1,
    "summary_path": "/tmp/artifacts/workspace__fail/workspace_probe/workspace_summary.json",
    "members": [
      {
        "path": "/tmp/checkouts/workspace__fail/gamma",
        "label": "gamma",
        "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma",
        "discovery": { "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null },
        "resolve": { "ok": false, "panic": false, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "error", "error": "resolve failed", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma/resolve/failure.json" },
        "merge": null
      }
    ]
  }
}"#,
    )
    .expect("write target summary");

    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusTriage(DebugCorpusTriage {
            run: run_dir,
            artifact_dir: temp.path().join("artifacts"),
            out_dir: None,
            no_report_stubs: false,
            watch: false,
            interval_secs: 10,
            exit_when_complete: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug corpus-triage partial");
    match out {
        ParseOutput::Debug(DebugOutput::CorpusTriage(o)) => {
            assert_eq!(o.run_id, "run-777");
            assert_eq!(o.snapshot_mode, "partial");
            assert!(o.summary_path.is_none());
            assert_eq!(o.failure_count, 1);
            assert_eq!(o.cluster_count, 1);
            assert!(Path::new(&o.triage_dir).join("index.json").is_file());
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_triage_reads_workspace_summary_without_target_summary() {
    let temp = TempDir::new().expect("tempdir");
    let run_dir = temp.path().join("run-778");
    let target_dir = run_dir.join("workspace__fail");
    let workspace_probe_dir = target_dir.join("workspace_probe");
    std::fs::create_dir_all(&workspace_probe_dir).expect("create workspace probe dir");
    let workspace_summary_path = workspace_probe_dir.join("workspace_summary.json");
    std::fs::write(
        &workspace_summary_path,
        r#"{
  "workspace_root": "/tmp/checkouts/workspace__fail",
  "member_count": 1,
  "failed_members": 1,
  "summary_path": null,
  "members": [
    {
      "path": "/tmp/checkouts/workspace__fail/gamma",
      "label": "gamma",
      "artifact_dir": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma",
      "discovery": { "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null },
      "resolve": { "ok": false, "panic": true, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "panic", "error": "thread 'worker' panicked at Duplicate Macro node ID at crates/elsewhere.rs:3:7", "failure_artifact_path": "/tmp/artifacts/workspace__fail/workspace_probe/members/000_gamma/resolve/failure.json" },
      "merge": null
    }
  ]
}"#,
    )
    .expect("write workspace summary");

    let cmd = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusTriage(DebugCorpusTriage {
            run: run_dir,
            artifact_dir: temp.path().join("artifacts"),
            out_dir: None,
            no_report_stubs: false,
            watch: false,
            interval_secs: 10,
            exit_when_complete: false,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug corpus-triage workspace-only partial");
    match out {
        ParseOutput::Debug(DebugOutput::CorpusTriage(o)) => {
            assert_eq!(o.run_id, "run-778");
            assert_eq!(o.snapshot_mode, "partial");
            assert!(o.summary_path.is_none());
            assert_eq!(o.failure_count, 1);
            assert_eq!(o.cluster_count, 1);
            assert_eq!(o.failures[0].normalized_repo, "workspace/fail");
            assert_eq!(o.failures[0].member_label.as_deref(), Some("gamma"));
            assert!(
                o.failures[0]
                    .workspace_summary_path
                    .as_deref()
                    .is_some_and(|p| p.ends_with("workspace_summary.json"))
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_triage_preserves_existing_pending_report() {
    let temp = TempDir::new().expect("tempdir");
    let run_dir = temp.path().join("run-779");
    std::fs::create_dir_all(&run_dir).expect("create run dir");
    let summary_path = run_dir.join("summary.json");
    std::fs::write(
        &summary_path,
        r#"{
  "run_id": "run-779",
  "checkout_root": "/tmp/checkouts",
  "artifact_root": "/tmp/artifacts",
  "list_files": [],
  "requested_entries": 1,
  "unique_targets": 1,
  "processed_targets": 1,
  "single_crate_targets": 1,
  "workspace_targets": 0,
  "skipped_targets": 0,
  "cloned_targets": 0,
  "reused_targets": 1,
  "clone_failures": 0,
  "discovery_failures": 0,
  "resolve_failures": 1,
  "merge_failures": 0,
  "panic_failures": 1,
  "workspace_mode": "skip",
  "targets": [
    {
      "target": "single/fail",
      "normalized_repo": "single/fail",
      "clone_url": "https://github.com/single/fail.git",
      "datasets": ["list-a"],
      "checkout_path": "/tmp/checkouts/single__fail",
      "artifact_dir": "/tmp/artifacts/single__fail",
      "repository_kind": "single_crate",
      "recommended_parser": "try_run_phases_and_merge",
      "workspace_member_count": null,
      "classification_error": null,
      "classification_diagnostic": null,
      "clone": { "ok": true, "action": "reused", "error": null },
      "commit_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "discovery": { "ok": true, "panic": false, "duration_ms": 2, "file_count": 3, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": null, "error": null, "failure_artifact_path": null },
      "resolve": { "ok": false, "panic": true, "duration_ms": 8, "file_count": null, "nodes_parsed": null, "relations_found": null, "artifact_path": null, "failure_kind": "panic", "error": "thread 'worker' panicked at Duplicate Macro node ID at crates/somewhere.rs:9:1", "failure_artifact_path": "/tmp/artifacts/single__fail/resolve/failure.json" },
      "merge": null,
      "summary_path": "/tmp/artifacts/single__fail/summary.json",
      "workspace_probe": null
    }
  ]
}"#,
    )
    .expect("write summary");

    let ctx = CommandContext::new().expect("CommandContext");
    let first = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusTriage(DebugCorpusTriage {
            run: summary_path.clone(),
            artifact_dir: temp.path().join("artifacts"),
            out_dir: None,
            no_report_stubs: false,
            watch: false,
            interval_secs: 10,
            exit_when_complete: false,
        }),
    }
    .execute(&ctx)
    .expect("first triage run");

    let pending_report_path = match first {
        ParseOutput::Debug(DebugOutput::CorpusTriage(o)) => o.clusters[0]
            .pending_report_path
            .clone()
            .expect("pending report path"),
        other => panic!("unexpected output: {other:?}"),
    };

    let preserved = serde_json::json!({
        "version": 1,
        "run_id": "run-779",
        "cluster_key": "resolve|panic|Duplicate Macro node ID",
        "cluster_slug": "resolve_panic_duplicate_macro_node_id",
        "status": "in_progress",
        "notes": ["keep me"]
    });
    std::fs::write(
        &pending_report_path,
        serde_json::to_string_pretty(&preserved).expect("serialize preserved report"),
    )
    .expect("overwrite pending report");

    let second = ParseDebugCli {
        cmd: ParseDebugCmd::CorpusTriage(DebugCorpusTriage {
            run: summary_path,
            artifact_dir: temp.path().join("artifacts"),
            out_dir: None,
            no_report_stubs: false,
            watch: false,
            interval_secs: 10,
            exit_when_complete: false,
        }),
    };
    second.execute(&ctx).expect("second triage run");

    let preserved_after: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&pending_report_path).expect("read pending report"),
    )
    .expect("parse pending report");
    assert_eq!(preserved_after["status"], "in_progress");
    assert_eq!(preserved_after["notes"][0], "keep me");
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
            workspace_mode: CorpusWorkspaceMode::Skip,
            resolve_timeout_minutes: 0,
            merge_timeout_minutes: 0,
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
            assert!(target.workspace_probe.is_none(), "{target:#?}");
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
            workspace_mode: CorpusWorkspaceMode::Skip,
            resolve_timeout_minutes: 0,
            merge_timeout_minutes: 0,
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
            assert!(target.workspace_probe.is_none(), "{target:#?}");
        }
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn parse_debug_corpus_probes_workspace_repo_members_when_enabled() {
    let temp = TempDir::new().expect("tempdir");
    let source_repo = temp.path().join("mini_workspace");
    init_local_git_workspace(&source_repo);

    let list_file = temp.path().join("targets_workspace_probe.txt");
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
            workspace_mode: CorpusWorkspaceMode::Probe,
            resolve_timeout_minutes: 0,
            merge_timeout_minutes: 0,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug corpus workspace probe");
    match out {
        ParseOutput::Debug(DebugOutput::Corpus(o)) => {
            assert_eq!(o.workspace_mode, "probe");
            assert_eq!(o.processed_targets, 1, "expected one processed target");
            assert_eq!(o.single_crate_targets, 0, "did not expect crate targets");
            assert_eq!(o.workspace_targets, 1, "expected one workspace target");
            assert_eq!(o.clone_failures, 0, "unexpected clone failure: {o:#?}");
            assert_eq!(
                o.discovery_failures, 0,
                "workspace member discovery should succeed"
            );
            assert_eq!(
                o.resolve_failures, 0,
                "workspace member resolve should succeed"
            );
            assert_eq!(o.merge_failures, 0, "workspace member merge should succeed");

            let target = o.targets.first().expect("one target result");
            let probe = target
                .workspace_probe
                .as_ref()
                .expect("workspace probe should be present");
            assert_eq!(probe.member_count, 1);
            assert_eq!(probe.failed_members, 0);
            assert!(
                probe
                    .summary_path
                    .as_deref()
                    .is_some_and(|p| Path::new(p).is_file())
            );

            let member = probe.members.first().expect("one member");
            assert!(member.discovery.ok, "{member:#?}");
            assert!(
                member.resolve.as_ref().is_some_and(|stage| stage.ok),
                "{member:#?}"
            );
            assert!(
                member.merge.as_ref().is_some_and(|stage| stage.ok),
                "{member:#?}"
            );
            assert!(Path::new(&member.artifact_dir).is_dir());
            assert!(
                member
                    .discovery
                    .artifact_path
                    .as_deref()
                    .is_some_and(|p| Path::new(p).is_file())
            );
            assert!(
                member
                    .resolve
                    .as_ref()
                    .and_then(|stage| stage.artifact_path.as_deref())
                    .is_some_and(|p| Path::new(p).is_file())
            );
            assert!(
                member
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
fn parse_debug_corpus_probes_implicit_workspace_members_via_metadata_fallback() {
    let temp = TempDir::new().expect("tempdir");
    let source_repo = temp.path().join("implicit_workspace");
    init_local_git_implicit_workspace(&source_repo);

    let list_file = temp.path().join("targets_implicit_workspace_probe.txt");
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
            workspace_mode: CorpusWorkspaceMode::Probe,
            resolve_timeout_minutes: 0,
            merge_timeout_minutes: 0,
        }),
    };
    let ctx = CommandContext::new().expect("CommandContext");
    let out = cmd
        .execute(&ctx)
        .expect("parse debug corpus implicit workspace probe");
    match out {
        ParseOutput::Debug(DebugOutput::Corpus(o)) => {
            assert_eq!(o.workspace_mode, "probe");
            assert_eq!(o.processed_targets, 1, "expected one processed target");
            assert_eq!(o.single_crate_targets, 0, "did not expect crate targets");
            assert_eq!(o.workspace_targets, 1, "expected one workspace target");
            assert_eq!(o.clone_failures, 0, "unexpected clone failure: {o:#?}");
            assert_eq!(
                o.discovery_failures, 0,
                "workspace member discovery should succeed"
            );
            assert_eq!(
                o.resolve_failures, 0,
                "workspace member resolve should succeed"
            );
            assert_eq!(o.merge_failures, 0, "workspace member merge should succeed");

            let target = o.targets.first().expect("one target result");
            let probe = target
                .workspace_probe
                .as_ref()
                .expect("workspace probe should be present");
            assert_eq!(probe.member_count, 2);
            assert_eq!(probe.failed_members, 0);
            assert!(
                probe.members.iter().any(|member| member.label == "helper"),
                "{probe:#?}"
            );
            assert!(
                probe
                    .members
                    .iter()
                    .any(|member| member.label == "implicit_workspace"),
                "{probe:#?}"
            );
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
