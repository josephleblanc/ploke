//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `TabbyML/tabby`
//! - Target crate: `aim-downloader`
//! - Saved failing member: `aim-downloader`
//! - Saved hotspot file: `src/untildify.rs`
//!
//! The original corpus failure was a merge-stage error:
//! `Internal state error: Failed to build module tree: Feature not implemented:
//! Duplicate definition path 'crate::untildify::tests' found in module tree.`
//!
//! The persisted source reduces to one crate root module plus a single submodule
//! file that declares two sibling `#[cfg(test)] mod tests` blocks under different
//! platform gates. This fixture keeps that shape valid Rust and small enough to
//! verify:
//! - the crate compiles under `cargo check`
//! - `syn_parser` still reports the duplicate module-tree path

use std::path::PathBuf;
use std::process::Command;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

fn fixture_member_root() -> PathBuf {
    fixture_workspace_root().join("member_cfg_test_mods_repro")
}

#[test]
fn fixture_duplicate_cfg_test_mods_is_valid_rust() {
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
fn repro_duplicate_cfg_test_mods_merge_error() {
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
        err_msg.contains("Duplicate definition path 'crate::untildify::tests'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}
