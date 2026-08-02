use std::path::PathBuf;

use ploke_common::workspace_root;
use syn_parser::parse_workspace;
use syn_parser::parser::graph::GraphAccess as _;
use syn_parser::parser::nodes::PrimaryNodeId;
use syn_parser::parser::relations::SyntacticRelation;

use crate::repro::validate_fixture;

fn fixture_workspace_root() -> PathBuf {
    workspace_root().join("tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids")
}

// TEST_NOTE:2026-07-08
//
// Provenance:
// - Corpus run: `run-1774765997311`
// - Target repo: `TabbyML/tabby`
// - Target crate: `aim-downloader`
// - Saved failing member: `aim-downloader`
// - Saved hotspot file: `src/untildify.rs`
//
// Previously failed at merge with duplicate definition path
// `crate::untildify::tests`. The fixture is valid Rust with two sibling
// `#[cfg(test)] mod tests` blocks under different platform gates; current
// parser cfg handling excludes the inactive test modules from the selected
// normal build domain, so `parse_workspace` should succeed.
#[test]
fn repro_duplicate_cfg_test_mods_merge_ok() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_cfg_test_mods_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse successfully");
}

#[test]
fn repro_cfg_test_inline_module_inside_private_file_module_keeps_function_owner() {
    let fixture_root = fixture_workspace_root();
    let member_root = fixture_workspace_root().join("member_cfg_test_mods_repro");

    validate_fixture(&member_root);

    let selected = [member_root.as_path()];
    let parsed = parse_workspace(&fixture_root, Some(&selected)).expect("workspace should parse");
    let crate0 = parsed.crates.first().expect("selected crate should parse");
    let graph = crate0
        .parser_output
        .merged_graph
        .as_ref()
        .expect("merged graph should be built");

    let test_module = graph
        .find_module_by_path_checked(&[
            "crate".to_string(),
            "untildify".to_string(),
            "tests".to_string(),
        ])
        .expect("active cfg(test) inline module should be retained");
    let function = graph
        .functions()
        .iter()
        .find(|function| function.name == "included_cfg_test_owner")
        .expect("function inside active cfg(test) module should be retained");

    let expected_owner = SyntacticRelation::Contains {
        source: test_module.id,
        target: PrimaryNodeId::from(function.id),
    };

    assert!(
        graph
            .relations()
            .iter()
            .any(|relation| *relation == expected_owner),
        "function should be owned by crate::untildify::tests"
    );
}
