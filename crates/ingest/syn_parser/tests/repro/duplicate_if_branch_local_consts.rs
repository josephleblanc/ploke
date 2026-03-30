//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `RustPython/RustPython`
//! - Target crate: `stdlib`
//! - Saved failing member: `stdlib`
//! - Saved hotspot file: `src/resource.rs`
//!
//! The original corpus failure was a resolve-stage panic:
//! `Expected unique relations, found invalid duplicate with error: Duplicate node
//! found for ID AnyNodeId::Const(...) when only one was expected.`
//!
//! The persisted source minimized down to one inherent `impl` method with a plain
//! `if/else` and the same local `const Y` in both branches. This fixture keeps that
//! shape valid Rust and small enough to verify:
//! - the crate compiles under `cargo check`
//! - `syn_parser` skips executable-body local items instead of panicking on duplicate
//!   local const IDs
//!
//! If this test starts passing, re-check whether the original corpus failure
//! depended on additional surrounding scope details from `resource.rs`.

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::try_run_phases_and_resolve;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_if_branch_local_consts_repro")
}

#[test]
fn fixture_duplicate_if_branch_local_consts_is_valid_rust() {
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
fn repro_duplicate_if_branch_local_consts_resolves_without_executable_local_item_nodes() {
    let fixture_root = fixture_member_root();

    assert!(
        fixture_root.is_dir(),
        "fixture crate must exist: {}",
        fixture_root.display()
    );

    try_run_phases_and_resolve(&fixture_root)
        .expect("parser should skip executable-body local const items instead of panicking");
}
