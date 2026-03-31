use std::path::PathBuf;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

use crate::repro::validate_fixture;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `bytecodealliance/rustix`
// - Target crate: `rustix`
// - Saved failing member: none
// - Saved hotspot file: `src/backend/libc/c.rs`
//
// The original corpus failure was a merge-stage error:
// `Internal state error: Failed to build module tree: Feature not implemented:
// Duplicate definition path 'crate::backend::libc::c::readwrite_pv64v2' found in module tree.`
//
// The minimized shape is a nested module path ending in a single source file
// that declares the same `mod readwrite_pv64v2` twice under mutually exclusive
// `#[cfg(...)]` gates. That keeps the Rust valid while reproducing the duplicate
// module-tree path for the parser.
#[test]
fn repro_duplicate_cfg_gated_module_merge_error() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_root.join("member_cfg_duplicate_mods_repro");

    validate_fixture(&member_root);

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
        err_msg.contains("Duplicate definition path 'crate::backend::libc::c::readwrite_pv64v2'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `jj-vcs/jj`
// - Target crate: `lib`
// - Saved failing member: `lib`
// - Saved hotspot file: `lib/src/protos/mod.rs`
//
// The original corpus failure was a merge-stage error:
// `Internal state error: Failed to build module tree: Feature not implemented:
// Duplicate definition path 'crate::protos::default_index' found in module tree.`
//
// The minimized shape is a `protos/mod.rs` that defines `pub mod default_index`
// inline via `include!`, plus a sibling `protos/default_index.rs` file on disk.
// This mirrors the prost-build layout and reproduces the duplicate module-tree
// path while remaining valid Rust.
#[test]
fn repro_duplicate_inline_protos_module_merge_error() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_root.join("member_protos_default_index_repro");

    validate_fixture(&member_root);

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
        err_msg.contains("Duplicate definition path 'crate::protos::default_index'"),
        "error should mention the duplicate module path, got: {err_msg}"
    );
}
