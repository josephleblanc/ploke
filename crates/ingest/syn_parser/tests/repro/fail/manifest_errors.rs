//! Manifest parsing uses `cargo_toml::Manifest::from_path`, matching Cargo completion and
//! workspace inheritance. Shapes that used to fail strict `toml::deserialize` in discovery are
//! exercised here as **success** cases after migration.

use std::fs;
use std::path::Path;

use syn_parser::discovery::TargetKind;
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
// - Corpus triage: failure-0006 (`docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md`)
//
// The original failure was strict `toml::deserialize` in discovery: missing `package.version`.
// Upstream `benches/Cargo.toml` has **no** `[package].version` and **no** `version.workspace = true`
// (see `tests/fixture_github_clones/corpus/bevyengine__bevy/benches/Cargo.toml`). Cargo treats that
// as version **0.0.0** — not workspace inheritance. This test uses that **same minimal shape**
// (only `name` + `edition` in `[package]`) and asserts discovery succeeds and the resolved version
// matches Cargo. Workspace `version.workspace = true` + inherited semver is covered by
// `discovery::tests::test_toml_basic` (`tests/fixture_workspace/ws_fixture_00`).
// Former name: `repro_workspace_package_missing_version_manifest_parse_error` →
// `repro_workspace_package_version_resolves_from_workspace_via_cargo_toml` → this name.
#[test]
fn repro_workspace_package_missing_version_bevy_like_accepts_like_cargo() {
    let td = tempdir().expect("create tempdir");
    let workspace_root = td.path();

    write_file(
        &workspace_root.join("Cargo.toml"),
        r#"[workspace]
members = ["member_missing_version"]
resolver = "2"
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
    let parsed = parse_workspace(workspace_root, Some(&selected)).expect("workspace should parse");

    let ctx = parsed
        .crates
        .iter()
        .find(|c| c.crate_context.root_path == member_root)
        .expect("member crate")
        .crate_context
        .clone();
    assert_eq!(
        ctx.version, "0.0.0",
        "Cargo default for omitted [package].version (Bevy benches–like shape)"
    );
    assert_eq!(ctx.name, "member_missing_version");
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `linera-io/linera-protocol`
// - Target crate: `linera-bridge`
// - Saved failing member: `linera-bridge`
// - Saved hotspot file: `linera-bridge/Cargo.toml`
// - Corpus triage: failure-0042 (same triage doc as above)
//
// The original failure was strict deserialization: `missing field 'path'` on a `[[bin]]` with only
// `name`. Cargo defaults the path (e.g. `src/bin/<name>.rs`); upstream matches that shape (see
// `tests/fixture_github_clones/corpus/linera-io__linera-protocol/linera-bridge/Cargo.toml`).
// **This** repro is only about the omitted `path`; it is not the Bevy/missing-`version` case above.
// Former name: `repro_bin_target_missing_path_manifest_parse_error` →
// `repro_bin_target_omitted_path_defaults_like_cargo`.
#[test]
fn repro_bin_target_omitted_path_defaults_like_cargo() {
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
    write_file(&member_root.join("src/bin/helper.rs"), "fn main() {}\n");

    let selected = [member_root.as_path()];
    let parsed = parse_workspace(workspace_root, Some(&selected)).expect("workspace should parse");

    let ctx = parsed
        .crates
        .iter()
        .find(|c| c.crate_context.root_path == member_root)
        .expect("member crate")
        .crate_context
        .clone();
    let bin = ctx
        .targets
        .iter()
        .find(|t| t.kind == TargetKind::Bin && t.name == "helper")
        .expect("defaulted bin target");
    assert!(
        bin.root.ends_with("src/bin/helper.rs"),
        "expected Cargo default path, got {}",
        bin.root.display()
    );
}
