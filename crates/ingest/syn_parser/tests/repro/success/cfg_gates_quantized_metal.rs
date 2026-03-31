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
// - Target repo: `huggingface/candle`
// - Target crate: `candle-core`
// - Saved failing member: `candle-core`
// - Saved hotspot file: `src/quantized/mod.rs`
//
// Previously failed at merge with duplicate definition path `crate::quantized::metal`
// (cfg-gated inline `mod metal {}` vs scan-discovered `metal.rs`). After ADR-025 staging,
// `parse_workspace` should succeed for this valid Rust shape.
#[test]
fn repro_duplicate_quantized_metal_mod_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_quantized_metal_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}
