//! Tests for `parse debug` path diagnostics (logical-paths, modules-premerge, path-collisions).

use std::path::PathBuf;

use xtask::commands::parse::ParseOutput;
use xtask::commands::parse_debug::{
    DebugCargoTargets, DebugDiscoveryRules, DebugLogicalPaths, DebugModulesPremerge,
    DebugOutput, DebugPathCollisions, DebugWorkspaceMembers, ParseDebugCli, ParseDebugCmd,
};
use xtask::executor::Command;
use xtask::expect_command_ok;
use xtask::test_harness::CommandTestHarness;

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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug logical-paths",
    );
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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug modules-premerge",
    );
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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug path-collisions",
    );
    match out {
        ParseOutput::Debug(DebugOutput::PathCollisions(o)) => {
            assert!(o.merged_module_count > 0);
            for g in &o.collisions {
                assert!(g.modules.len() > 1, "collision group must have 2+ modules: {g:#?}");
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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug cargo-targets",
    );
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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug workspace-members --classify",
    );
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
    let harness = CommandTestHarness::new().expect("harness");
    let out = expect_command_ok(
        harness.executor().execute(cmd),
        "parse debug discovery-rules",
    );
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
