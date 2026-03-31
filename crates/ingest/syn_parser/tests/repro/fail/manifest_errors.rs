use std::fs;
use std::path::Path;

use syn_parser::parse_workspace;
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, contents).expect("write fixture file");
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `bevyengine/bevy`
// - Target crate: `benches`
// - Saved failing member: `benches`
// - Saved hotspot file: `benches/Cargo.toml`
//
// The original corpus failure was a discovery-stage error:
// `Failed to parse manifest ... missing field 'version'`.
//
// The minimized shape is a workspace with `workspace.package.version` defined,
// while a member `Cargo.toml` omits the `package.version` field. Cargo accepts
// the workspace default, but the manifest parser currently requires an explicit
// `package.version` field, producing the same parse error.
#[test]
fn repro_workspace_package_missing_version_manifest_parse_error() {
    let td = tempdir().expect("create tempdir");
    let workspace_root = td.path();

    write_file(
        &workspace_root.join("Cargo.toml"),
        r#"[workspace]
members = ["member_missing_version"]

[workspace.package]
version = "0.1.0"
edition = "2024"
"#,
    );

    let member_root = workspace_root.join("member_missing_version");
    write_file(
        &member_root.join("Cargo.toml"),
        r#"[package]
name = "member_missing_version"
edition = "2024"
"#,
    );
    write_file(&member_root.join("src/lib.rs"), "pub fn ping() {}\n");

    let selected = [member_root.as_path()];
    let err = match parse_workspace(workspace_root, Some(&selected)) {
        Ok(_) => panic!("workspace unexpectedly parsed successfully"),
        Err(err) => err,
    };
    let err_msg = err.to_string();

    assert!(
        err_msg.contains("Failed to parse manifest"),
        "error should preserve manifest context, got: {err_msg}"
    );
    assert!(
        err_msg.contains("missing field `version`"),
        "error should mention missing version field, got: {err_msg}"
    );
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `linera-io/linera-protocol`
// - Target crate: `linera-bridge`
// - Saved failing member: `linera-bridge`
// - Saved hotspot file: `linera-bridge/Cargo.toml`
//
// The original corpus failure was a discovery-stage error:
// `Failed to parse manifest ... missing field 'path'` in a `[[bin]]` entry.
//
// The minimized shape is a workspace member that declares a `[[bin]]` with a
// name but omits the `path`. Cargo defaults to `src/bin/<name>.rs`, but the
// manifest parser currently requires the `path` field and fails with the same
// parse error.
#[test]
fn repro_bin_target_missing_path_manifest_parse_error() {
    let td = tempdir().expect("create tempdir");
    let workspace_root = td.path();

    write_file(
        &workspace_root.join("Cargo.toml"),
        r#"[workspace]
members = ["member_missing_bin_path"]
"#,
    );

    let member_root = workspace_root.join("member_missing_bin_path");
    write_file(
        &member_root.join("Cargo.toml"),
        r#"[package]
name = "member_missing_bin_path"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "helper"
"#,
    );
    write_file(
        &member_root.join("src/bin/helper.rs"),
        "fn main() {}\n",
    );

    let selected = [member_root.as_path()];
    let err = match parse_workspace(workspace_root, Some(&selected)) {
        Ok(_) => panic!("workspace unexpectedly parsed successfully"),
        Err(err) => err,
    };
    let err_msg = err.to_string();

    assert!(
        err_msg.contains("Failed to parse manifest"),
        "error should preserve manifest context, got: {err_msg}"
    );
    assert!(
        err_msg.contains("missing field `path`"),
        "error should mention missing bin path, got: {err_msg}"
    );
}
