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
// - Target repo: `leptos-rs/leptos`
// - Target crate: `leptos`
//
// Inline `pub mod logging { ... }` plus `src/logging.rs` (valid Rust). Previously duplicate
// path at merge; resolved via ADR-025 file-vs-inline staging.
#[test]
fn repro_duplicate_logging_inline_file_mod_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_logging_inline_file_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `iced-rs/iced`
// - Target crate: `wgpu`
//
// Inline `pub mod image { ... }` plus `src/image.rs`.
#[test]
fn repro_duplicate_image_inline_file_mod_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_image_inline_file_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}

// TEST_NOTE:2026-03-30
//
// Provenance:
// - Corpus run: `run-1774867607815`
// - Target repo: `jj-vcs/jj`
// - Target crate: `lib`
//
// `protos/mod.rs` with inline `default_index` via `include!` plus `default_index.rs` on disk.
#[test]
fn repro_duplicate_inline_protos_module_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_root.join("member_protos_default_index_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}
