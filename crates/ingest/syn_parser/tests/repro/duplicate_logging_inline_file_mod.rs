//! TEST_NOTE:2026-03-30
//!
//! Provenance:
//! - Corpus run: `run-1774867607815`
//! - Target repo: `leptos-rs/leptos`
//! - Target crate: `leptos`
//! - Saved failing member: `leptos`
//! - Saved hotspot file: `src/lib.rs`
//!
//! The original corpus failure was a merge-stage error:
//! `Internal state error: Failed to build module tree: Feature not implemented:
//! Duplicate definition path 'crate::logging' found in module tree.`
//!
//! The minimized shape is a crate root with an inline `pub mod logging { ... }`
//! body plus a same-named `src/logging.rs` file. That is enough to reproduce the
//! duplicate module-tree path while still remaining valid Rust.

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_logging_inline_file_repro")
}

#[test]
fn repro_duplicate_logging_inline_file_mod_merge_error() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_member_root();

    assert!(
        member_root.is_dir(),
        "fixture crate must exist: {}",
        member_root.display()
    );

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(&member_root)
        .output()
        .expect("run cargo check for committed fixture");

    assert!(
        output.status.success(),
        "fixture must compile successfully.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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
        err_msg.contains("Duplicate definition path 'crate::logging'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}
