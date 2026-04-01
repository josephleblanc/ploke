use std::path::PathBuf;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;
use syn_parser::parser::nodes::ModuleNodeId;

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

/// After `#[path]` reindexing, `crate::collision::dup` must still map to the **inline** module in
/// `path_index` (stem collision with the scan-root `dup.rs` file consumed by `not_dup`).
#[test]
fn path_stem_collision_path_index_keeps_inline_dup_after_path_reindex() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_path_stem_collision_repro");
    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    let parsed = parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse");

    let crate0 = parsed
        .crates
        .first()
        .expect("selected crate should be parsed");
    let tree = crate0
        .parser_output
        .module_tree
        .as_ref()
        .expect("module tree should be built");

    let path_index = tree.path_index();
    let dup_segments = [
        "crate".to_string(),
        "collision".to_string(),
        "dup".to_string(),
    ];
    let not_dup_segments = [
        "crate".to_string(),
        "collision".to_string(),
        "not_dup".to_string(),
    ];

    let dup_mod_id = ModuleNodeId::try_from(
        *path_index
            .get(&dup_segments[..])
            .expect("crate::collision::dup must be in path_index"),
    )
    .expect("dup path should map to ModuleNodeId");

    let dup_mod = tree.modules().get(&dup_mod_id).expect("dup module node");
    assert!(
        dup_mod.is_inline(),
        "crate::collision::dup must remain the inline module in path_index"
    );

    let not_dup_mod_id = ModuleNodeId::try_from(
        *path_index
            .get(&not_dup_segments[..])
            .expect("crate::collision::not_dup must be in path_index"),
    )
    .expect("not_dup path should map to ModuleNodeId");

    let not_dup_mod = tree
        .modules()
        .get(&not_dup_mod_id)
        .expect("not_dup module node");
    assert!(
        not_dup_mod.is_file_based(),
        "crate::collision::not_dup must map to the file-backed definition from dup.rs via #[path]"
    );
}
