//! Tests for `parse debug` path diagnostics (logical-paths, modules-premerge, path-collisions).

use std::path::PathBuf;

use xtask::commands::parse::ParseOutput;
use xtask::commands::parse_debug::{
    DebugLogicalPaths, DebugModulesPremerge, DebugPathCollisions, DebugOutput, ParseDebugCli,
    ParseDebugCmd,
};
use xtask::executor::Command;
use xtask::expect_command_ok;
use xtask::test_harness::CommandTestHarness;

const FIXTURE_NODES: &str = "tests/fixture_crates/fixture_nodes";

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
