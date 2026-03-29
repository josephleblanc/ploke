//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `HigherOrderCO/Bend`
//! - Target crate: `bend`
//! - Saved failing member: none
//! - Saved hotspot file: none
//!
//! The original corpus failure was a resolve-stage panic:
//! `Expected unique relations, found invalid duplicate with error: Duplicate node
//! found for ID AnyNodeId::Function(...) when only one was expected.`
//!
//! The persisted triage and live replay pointed to one inherent `impl` method
//! containing two sibling inner blocks, each defining a local `fn go_term`. This
//! fixture keeps that shape valid Rust and small enough to verify:
//! - the crate compiles under `cargo check`
//! - `syn_parser` skips executable-body local items instead of panicking on duplicate
//!   function IDs

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::try_run_phases_and_resolve;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_local_function_repro")
}

#[test]
fn fixture_duplicate_local_functions_is_valid_rust() {
    let fixture_root = fixture_workspace_root();

    assert!(
        fixture_root.is_dir(),
        "fixture workspace must exist: {}",
        fixture_root.display()
    );

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(&fixture_root)
        .output()
        .expect("run cargo check for committed fixture");

    assert!(
        output.status.success(),
        "fixture must compile successfully.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repro_duplicate_local_functions_resolves_without_executable_local_item_nodes() {
    let fixture_root = fixture_member_root();

    assert!(
        fixture_root.is_dir(),
        "fixture crate must exist: {}",
        fixture_root.display()
    );

    try_run_phases_and_resolve(&fixture_root)
        .expect("parser should skip executable-body local function items instead of panicking");
}
