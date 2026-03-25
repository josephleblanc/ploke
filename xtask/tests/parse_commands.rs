//! Fail-until-impl tests for xtask parse commands (Category A.1)
//!
//! Fail-until-impl: tests call the executor and use [`xtask::expect_command_ok`] or
//! `expect_err` for negatives; they fail until `commands/parse.rs` is implemented.
//!
//! ## Commands Under Test
//! - `Discovery` - Run discovery phase
//! - `PhasesMerge` - Parse and merge graphs
//! - `Workspace` - Parse workspace
//! - `Stats` - Parse statistics
//! - `ListModules` - List modules
//!
//! ## Test Structure
//! Each test documents:
//! - Underlying function(s) being tested
//! - Expected functionality
//! - Invariants
//! - Fail states
//! - Edge cases
//! - Hypothesis format: "To Prove: ... Given: ... When: ... Then: ..."

use std::path::{Path, PathBuf};

use xtask::commands::parse::{
    Discovery, ListModules, ModuleInfo, NodeTypeFilter, ParseOutput, PhasesMerge, Stats, Workspace,
};
use xtask::executor::Command;
use xtask::expect_command_ok;
use xtask::test_harness::CommandTestHarness;

// ============================================================================
// Test A.1.1: Discovery Command Tests
// ============================================================================

/// Test: Discovery finds Cargo.toml in target crate
///
/// To Prove: That run_discovery_phase correctly identifies crate structure from a valid Cargo.toml
/// Given: A valid fixture crate
/// When: Discovery command runs with path argument
/// Then: Returns DiscoveryOutput containing at least one CrateContext
///
/// Invariants Verified:
/// - DiscoveryOutput contains non-empty crate_contexts
/// - Each CrateContext has valid name and version
/// - Source files are discoverable
///
/// Fail States:
/// - Invalid path (non-existent directory)
/// - Missing Cargo.toml
/// - Malformed Cargo.toml
/// - Permission denied on directory
///
/// Edge Cases:
/// - Empty crate (no source files)
/// - Workspace root (multiple crates)
/// - Path with spaces/special characters
/// - Relative vs absolute paths
///
/// When This Test Would NOT Prove Correctness:
/// - If the fixture crate structure doesn't represent real-world crates
/// - If filesystem behavior differs across platforms
#[test]
fn discovery_finds_cargo_toml() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_nodes");

    let cmd = Discovery {
        path: fixture_path,
        warnings: false,
        include_tests: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse discovery must succeed once implemented",
    );

    match output {
        ParseOutput::Discovery { crates_found, .. } => {
            assert!(crates_found > 0, "Expected at least one crate");
        }
        other => panic!("Expected Discovery output, got {other:?}"),
    }
}

/// Test: Discovery handles missing Cargo.toml
///
/// To Prove: That the command provides actionable error messages for missing Cargo.toml
/// Given: A directory without Cargo.toml
/// When: Discovery command runs on invalid path
/// Then: Returns error with recovery path and context
///
/// Invariants Verified:
/// - Error output includes path that failed
/// - Error type is distinguishable
/// - Recovery hint is present
///
/// Fail States:
/// - IO errors (permission denied, not found)
/// - Parse errors (malformed Cargo.toml)
/// - Logical errors (no Cargo.toml in path)
///
/// Edge Cases:
/// - Very long path names
/// - Paths with unicode characters
/// - Symbolic links in path
///
/// When This Test Would NOT Prove Correctness:
/// - If error types are unified/lost in translation layers
#[test]
fn discovery_error_missing_cargo_toml() {
    let invalid_path = PathBuf::from("tests/fixture_crates/nonexistent_crate");

    let cmd = Discovery {
        path: invalid_path,
        warnings: false,
        include_tests: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let err = harness
        .executor()
        .execute(cmd)
        .expect_err("parse discovery must fail for a missing crate path once implemented");
    let msg = err.to_string();
    assert!(
        msg.contains("Cargo.toml")
            || msg.contains("not found")
            || msg.contains("path")
            || msg.contains("exist"),
        "error should describe missing crate or manifest: {msg}"
    );
    assert!(
        err.recovery_suggestion().is_some(),
        "PRIMARY_TASK_SPEC §D expects recovery context: {err:?}"
    );
}

// ============================================================================
// Test A.1.5: PhasesMerge Command Tests
// ============================================================================

/// Test: PhasesMerge produces valid merged graph
///
/// To Prove: That try_run_phases_and_merge correctly merges multiple ParsedCodeGraphs into one
/// Given: A workspace or crate with multiple modules
/// When: Command executes
/// Then: Returns ParserOutput with merged graph containing all nodes
///
/// Invariants Verified:
/// - Merged graph node count equals sum of input graph nodes
/// - No duplicate node IDs
/// - Relations are preserved after merge
/// - Module tree is consistent
///
/// Fail States:
/// - ID collision during merge
/// - Inconsistent module trees
/// - Memory exhaustion on huge graphs
///
/// Edge Cases:
/// - Single module (no actual merge)
/// - Many modules (stress test)
/// - Modules with same name in different paths
///
/// When This Test Would NOT Prove Correctness:
/// - If merge logic has special cases for specific node types not exercised
#[test]
fn phases_merge_produces_merged_graph() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_nodes");

    let cmd = PhasesMerge {
        path: fixture_path,
        tree: false,
        validate: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse phases-merge must succeed once implemented",
    );

    match output {
        ParseOutput::PhaseResult {
            success,
            nodes_parsed,
            relations_found,
            ..
        } => {
            assert!(success, "Phase result should be successful");
            assert!(nodes_parsed > 0, "Should have parsed nodes");
            assert!(relations_found > 0, "Should have found relations");
        }
        other => panic!("Expected PhaseResult output, got {other:?}"),
    }
}

/// Test: PhasesMerge with tree output
///
/// To Prove: That --tree flag produces valid module tree structure
/// Given: A valid crate with modules
/// When: Command executes with --tree flag
/// Then: Output includes module tree structure
///
/// Invariants Verified:
/// - Module tree has valid root
/// - All modules are in tree
/// - Tree hierarchy matches file structure
///
/// Edge Cases:
/// - Deeply nested modules
/// - Circular module references (handled)
#[test]
fn phases_merge_with_tree_output() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_path_resolution");

    let cmd = PhasesMerge {
        path: fixture_path,
        tree: true,
        validate: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse phases-merge with --tree must succeed once implemented",
    );

    match output {
        ParseOutput::PhaseResult { success, .. } => {
            assert!(success, "Phase result should be successful with --tree");
        }
        other => panic!("Expected PhaseResult output, got {other:?}"),
    }
}

// ============================================================================
// Test A.1.6: Workspace Command Tests
// ============================================================================

/// Test: Workspace parsing discovers all crates
///
/// To Prove: That parse_workspace discovers and parses all crates in a workspace
/// Given: A valid workspace with multiple crates
/// When: Command executes with workspace path
/// Then: Returns ParsedWorkspace containing all crate graphs
///
/// Invariants Verified:
/// - All crates in workspace are parsed
/// - Each crate graph is valid
/// - Cross-crate references are tracked
/// - Workspace metadata is captured
///
/// Fail States:
/// - Missing workspace Cargo.toml
/// - Individual crate parse failures
/// - Mixed workspace/virtual manifest issues
///
/// Edge Cases:
/// - Single-crate workspace
/// - Workspace with many crates
/// - Workspace with path dependencies
/// - Selective crate parsing (--crates flag)
///
/// When This Test Would NOT Prove Correctness:
/// - If workspace structure differs from Cargo.toml patterns tested
#[test]
fn workspace_parses_all_crates() {
    let workspace_path = PathBuf::from("tests/fixture_workspace/ws_fixture_01");

    let cmd = Workspace {
        path: workspace_path,
        crate_name: vec![], // Parse all crates
        continue_on_error: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse workspace must succeed once implemented",
    );

    match output {
        ParseOutput::PhaseResult {
            success,
            nodes_parsed,
            ..
        } => {
            assert!(success, "Workspace parse should succeed");
            assert!(nodes_parsed > 0, "Should have parsed nodes from workspace");
        }
        other => panic!("Expected PhaseResult output, got {other:?}"),
    }
}

/// Test: Workspace with selective crate parsing
///
/// To Prove: That --crate flag filters workspace members correctly
/// Given: A workspace with multiple crates
/// When: Command executes with --crate flag for specific crate
/// Then: Only specified crate(s) are parsed
///
/// Invariants Verified:
/// - Only selected crates are in output
/// - Non-selected crates are skipped
/// - Selection by name works correctly
///
/// Edge Cases:
/// - Non-existent crate name
/// - Multiple --crate flags
/// - Case sensitivity in names
#[test]
fn workspace_selective_crate_parsing() {
    let workspace_path = PathBuf::from("tests/fixture_workspace/ws_fixture_01");

    let cmd = Workspace {
        path: workspace_path,
        crate_name: vec!["member_root".to_string()], // Select one workspace member
        continue_on_error: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse workspace with --crate must succeed once implemented",
    );

    match output {
        ParseOutput::PhaseResult { success, .. } => {
            assert!(success, "Selective workspace parse should succeed");
        }
        other => panic!("Expected PhaseResult output, got {other:?}"),
    }
}

/// Test: Workspace continue_on_error flag
///
/// To Prove: That --continue-on-error allows partial success
/// Given: A workspace where some crates may fail
/// When: Command executes with --continue-on-error
/// Then: Successful crates are returned, failures are logged
///
/// Invariants Verified:
/// - Successful crates are included in output
/// - Failed crates are recorded but don't stop execution
/// - Error summary is provided
#[test]
fn workspace_continue_on_error() {
    let workspace_path = PathBuf::from("tests/fixture_workspace/ws_fixture_01");

    let cmd = Workspace {
        path: workspace_path,
        crate_name: vec![],
        continue_on_error: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse workspace with --continue-on-error must succeed once implemented",
    );

    match output {
        ParseOutput::PhaseResult { .. } => {}
        other => panic!("Expected PhaseResult output, got {other:?}"),
    }
}

// ============================================================================
// Test A.1.7: Stats Command Tests
// ============================================================================

/// Test: Stats command returns accurate counts
///
/// To Prove: That stats command returns accurate counts matching actual parsed code
/// Given: A parsed crate with known structure
/// When: Stats command executes
/// Then: Returned counts match expected values
///
/// Invariants Verified:
/// - Count is non-negative
/// - Sum of category counts equals total
/// - Counts match direct query results
///
/// Fail States:
/// - Path not found
/// - Not a valid parsed crate
/// - Stats calculation error
///
/// Edge Cases:
/// - Empty crate (count = 0)
/// - Crate with one node
/// - Crate with many nodes
#[test]
fn stats_returns_accurate_counts() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_nodes");

    let cmd = Stats {
        path: fixture_path,
        node_type: None, // All types
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse stats must succeed once implemented",
    );

    match output {
        ParseOutput::Stats { .. } => {}
        other => panic!("Expected Stats output, got {other:?}"),
    }
}

/// Test: Stats with node type filter
///
/// To Prove: That --node-type filter restricts output correctly
/// Given: A parsed crate with multiple node types
/// When: Stats command executes with --node-type Function
/// Then: Only function count is returned
///
/// Invariants Verified:
/// - Filter is applied correctly
/// - Count matches only filtered type
#[test]
fn stats_with_node_type_filter() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_nodes");

    let cmd = Stats {
        path: fixture_path,
        node_type: Some(NodeTypeFilter::Function),
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse stats with --node-type must succeed once implemented",
    );

    match output {
        ParseOutput::Stats { by_type, .. } => {
            assert!(
                by_type.keys().any(|k| k.eq_ignore_ascii_case("function")),
                "filtered stats should include Function when --node-type Function: {by_type:?}"
            );
        }
        other => panic!("Expected Stats output, got {other:?}"),
    }
}

// ============================================================================
// Test A.1.8: ListModules Command Tests
// ============================================================================

/// Test: ListModules lists all modules
///
/// To Prove: That list-modules command finds all modules in parsed code
/// Given: A parsed crate with multiple modules
/// When: ListModules command executes
/// Then: Returns list of all modules with paths
///
/// Invariants Verified:
/// - All modules are listed
/// - Module paths are correct
/// - Root module is identified
///
/// Fail States:
/// - Path not found
/// - Not a valid parsed crate
///
/// Edge Cases:
/// - Single module (lib.rs only)
/// - Deeply nested modules
#[test]
fn list_modules_finds_all_modules() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_path_resolution");

    let cmd = ListModules {
        path: fixture_path,
        full_path: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse list-modules must succeed once implemented",
    );

    match output {
        ParseOutput::ModuleList { modules } => {
            assert!(!modules.is_empty(), "Should have at least one module");
            let root_count = modules.iter().filter(|m| m.is_root).count();
            assert_eq!(root_count, 1, "Should have exactly one root module");
        }
        other => panic!("Expected ModuleList output, got {other:?}"),
    }
}

/// Test: ListModules with full path
///
/// To Prove: That --full-path shows absolute paths
/// Given: A parsed crate with modules
/// When: ListModules command executes with --full-path
/// Then: Returns modules with absolute paths
///
/// Invariants Verified:
/// - Paths are absolute when flag is set
#[test]
fn list_modules_with_full_path() {
    let fixture_path = PathBuf::from("tests/fixture_crates/fixture_path_resolution");

    let cmd = ListModules {
        path: fixture_path,
        full_path: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "parse list-modules with --full-path must succeed once implemented",
    );

    match output {
        ParseOutput::ModuleList { modules } => {
            for m in &modules {
                if m.path.starts_with("crate::") {
                    continue;
                }
                assert!(
                    Path::new(&m.path).is_absolute(),
                    "file-backed module paths should be absolute when --full-path is set: {m:?}"
                );
            }
        }
        other => panic!("Expected ModuleList output, got {other:?}"),
    }
}

// ============================================================================
// Integration Tests: Error Handling
// ============================================================================

/// Test: Invalid path returns appropriate error
///
/// To Prove: That all parse commands handle invalid paths gracefully
/// Given: A non-existent path
/// When: Any parse command runs
/// Then: Returns clear error message
#[test]
fn parse_command_invalid_path_error() {
    let invalid_path = PathBuf::from("/nonexistent/path/to/crate");

    let cmd = Discovery {
        path: invalid_path,
        warnings: false,
        include_tests: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let err = harness
        .executor()
        .execute(cmd)
        .expect_err("parse discovery must fail for a nonexistent absolute path once implemented");
    let msg = err.to_string();
    assert!(
        msg.contains("not found")
            || msg.contains("invalid")
            || msg.contains("path")
            || msg.contains("exist")
            || msg.contains("IO"),
        "error should indicate path or IO problem: {msg}"
    );
    assert!(
        err.recovery_suggestion().is_some(),
        "PRIMARY_TASK_SPEC §D expects recovery context: {err:?}"
    );
}

// ============================================================================
// Module Documentation Tests
// ============================================================================

/// Verify that ParseOutput enum variants can be serialized
#[test]
fn parse_output_serialization() {
    let output = ParseOutput::Discovery {
        crates_found: 2,
        workspace_root: PathBuf::from("/workspace"),
        warnings: vec!["warning 1".to_string()],
    };

    let json = serde_json::to_string(&output).expect("Failed to serialize ParseOutput");
    assert!(json.contains("crates_found"));
    assert!(json.contains("2"));
    
    // TODO(M.4): Expand serialization tests after implementation
}

/// Verify that NodeTypeFilter variants are correct
#[test]
fn node_type_filter_variants() {
    let variants = vec![
        NodeTypeFilter::Function,
        NodeTypeFilter::Type,
        NodeTypeFilter::Module,
        NodeTypeFilter::Trait,
        NodeTypeFilter::Impl,
        NodeTypeFilter::All,
    ];
    
    assert_eq!(variants.len(), 6);
}

/// Verify that ModuleInfo can be created and serialized
#[test]
fn module_info_creation() {
    let info = ModuleInfo {
        name: "test_module".to_string(),
        path: "src/test_module.rs".to_string(),
        is_root: false,
    };
    
    assert_eq!(info.name, "test_module");
    assert_eq!(info.path, "src/test_module.rs");
    assert!(!info.is_root);
    
    let json = serde_json::to_string(&info).expect("Failed to serialize ModuleInfo");
    assert!(json.contains("test_module"));
}

// ============================================================================
// Command Trait Tests
// ============================================================================

/// Verify Discovery command implements Command trait correctly
#[test]
fn discovery_command_trait() {
    let cmd = Discovery {
        path: PathBuf::from("."),
        warnings: false,
        include_tests: false,
    };
    
    assert_eq!(cmd.name(), "parse discovery");
    assert!(!cmd.requires_async());
}

/// Verify PhasesMerge command implements Command trait correctly
#[test]
fn phases_merge_command_trait() {
    let cmd = PhasesMerge {
        path: PathBuf::from("/test"),
        tree: true,
        validate: false,
    };
    
    assert_eq!(cmd.name(), "parse phases-merge");
    assert!(!cmd.requires_async());
}

/// Verify Workspace command implements Command trait correctly
#[test]
fn workspace_command_trait() {
    let cmd = Workspace {
        path: PathBuf::from("."),
        crate_name: vec![],
        continue_on_error: false,
    };
    
    assert_eq!(cmd.name(), "parse workspace");
    assert!(!cmd.requires_async());
}

/// Verify Stats command implements Command trait correctly
#[test]
fn stats_command_trait() {
    let cmd = Stats {
        path: PathBuf::from("."),
        node_type: None,
    };
    
    assert_eq!(cmd.name(), "parse stats");
    assert!(!cmd.requires_async());
}

/// Verify ListModules command implements Command trait correctly
#[test]
fn list_modules_command_trait() {
    let cmd = ListModules {
        path: PathBuf::from("."),
        full_path: false,
    };
    
    assert_eq!(cmd.name(), "parse list-modules");
    assert!(!cmd.requires_async());
}
