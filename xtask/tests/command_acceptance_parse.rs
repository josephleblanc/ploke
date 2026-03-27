//! Command-level acceptance tests for `parse` subcommands (PRIMARY_TASK_SPEC A.1).
//!
//! Each test names `acceptance_parse_*` and documents the fixture and expected outputs.
//! Success paths assert concrete fields on [`ParseOutput`](xtask::commands::parse::ParseOutput).

use std::path::PathBuf;

use xtask::commands::parse::{ParseOutput, PhasesResolve};
use xtask::context::CommandContext;
use xtask::executor::Command as _;

/// `tests/fixture_crates/fixture_nodes`: small crate used elsewhere for phases-merge acceptance.
const FIXTURE_NODES: &str = "tests/fixture_crates/fixture_nodes";

/// **Command:** `parse phases-resolve`  
/// **Fixture:** [`FIXTURE_NODES`] (pinned path relative to workspace root).  
/// **Expect:** `PhaseResult` with success, positive `nodes_parsed` and `relations_found` (resolve path runs without merge).
#[test]
fn acceptance_parse_phases_resolve_success_fixture_nodes() {
    let cmd = PhasesResolve {
        path: PathBuf::from(FIXTURE_NODES),
        detailed: false,
        output: None,
    };

    let ctx = CommandContext::new().expect("CommandContext");
    let output = cmd
        .execute(&ctx)
        .expect("parse phases-resolve must succeed for fixture_nodes");

    match output {
        ParseOutput::PhaseResult {
            success,
            nodes_parsed,
            relations_found,
            duration_ms,
        } => {
            assert!(success, "phases-resolve should report success");
            assert!(
                nodes_parsed > 0,
                "expected parsed nodes > 0, got {nodes_parsed}"
            );
            assert!(
                relations_found > 0,
                "expected relations > 0, got {relations_found}"
            );
            assert!(duration_ms < 120_000, "sanity: duration_ms={duration_ms}");
        }
        other => panic!("expected PhaseResult, got {other:?}"),
    }
}

/// **Command:** `parse phases-resolve`  
/// **Expect:** validation-style error for non-existent path + recovery (§D).
#[test]
fn acceptance_parse_phases_resolve_rejects_missing_path() {
    let cmd = PhasesResolve {
        path: PathBuf::from("/nonexistent/xtask_parse_phases_resolve_path"),
        detailed: false,
        output: None,
    };

    let ctx = CommandContext::new().expect("CommandContext");
    let err = cmd
        .execute(&ctx)
        .expect_err("phases-resolve must fail for missing path");

    let msg = err.to_string();
    assert!(
        msg.contains("exist") || msg.contains("Path") || msg.contains("path"),
        "error should mention path: {msg}"
    );
    assert!(
        err.recovery_suggestion().is_some(),
        "PRIMARY_TASK_SPEC §D expects recovery: {err:?}"
    );
}
