use std::path::PathBuf;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;

use crate::repro::validate_fixture;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

// ADR-025 regression: `#[path = "dup.rs"] mod not_dup`, inline `mod dup {}`, and scan-root
// `collision/dup.rs` — merge and `#[path]` reindex must succeed without dropping the attributed
// definition before custom-path linking.
#[test]
fn repro_path_stem_collision_with_path_attr_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_path_stem_collision_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}
