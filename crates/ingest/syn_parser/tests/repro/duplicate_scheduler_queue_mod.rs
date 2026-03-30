//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `ankitects/anki`
//! - Target crate: `rslib`
//! - Saved failing member: `rslib`
//! - Saved hotspot file: `src/scheduler/queue/mod.rs`
//!
//! The original corpus failure was a merge-stage error:
//! `Internal state error: Failed to build module tree: Feature not implemented:
//! Duplicate definition path 'crate::scheduler::queue' found in module tree.`
//!
//! The minimized shape is a crate root with `mod scheduler;`, `scheduler/mod.rs`
//! with `mod queue;`, and `scheduler/queue/mod.rs` with `mod main;`, plus an
//! empty `scheduler/queue/main.rs`. That is small enough to verify while still
//! reproducing the duplicate module-tree path.

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_scheduler_queue_repro")
}

#[test]
fn fixture_duplicate_scheduler_queue_mod_is_valid_rust() {
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
fn repro_duplicate_scheduler_queue_mod_merge_error() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_member_root();

    assert!(
        member_root.is_dir(),
        "fixture crate must exist: {}",
        member_root.display()
    );

    let selected = [member_root.as_path()];
    let err = match parse_workspace(&fixture_root, Some(&selected)) {
        Ok(_) => panic!("workspace unexpectedly parsed successfully"),
        Err(err) => err,
    };
    let err_msg = err.to_string();

    assert!(
        err_msg.contains("Failed to build module tree"),
        "error should preserve module-tree context, got: {err_msg}"
    );
    assert!(
        err_msg.contains("Duplicate definition path 'crate::scheduler::queue'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}
