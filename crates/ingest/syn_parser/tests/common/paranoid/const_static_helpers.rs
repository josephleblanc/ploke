use itertools::Itertools;
use ploke_common::fixtures_crates_dir;
use ploke_core::{ItemKind, NodeId};
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
use syn_parser::TestIds;

use super::*;

/// Finds the specific ParsedCodeGraph for the target file, then finds the ValueNode
/// (const or static) within that graph corresponding to the given module path and name,
/// performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness or ID checks fail.
pub fn find_value_node_paranoid<'a>(args: ParanoidArgs) -> Result<PrimaryNodeId, SynParserError> {
    // 1. Construct the absolute expected file path
    let fixture_root = fixtures_crates_dir().join(args.fixture_name);
    let target_file_path = fixture_root.join(args.relative_file_path);
    let item_kind = ItemKind::Const;

    // 2. Find the specific ParsedCodeGraph for the target file
    let target_data = args
        .parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });
    let graph = &target_data.graph;
    let exp_path_string = args
        .expected_path
        .iter()
        .copied()
        .map(|s| s.to_string())
        .collect_vec();

    let parent_module = graph.find_module_by_path_checked(&exp_path_string)?;
    let cfg_string = strs_to_strings(args.expected_cfg);
    let cfgs = calculate_cfg_hash_bytes(&cfg_string);
    let item_name = args
        .expected_path
        .last()
        .expect("Must use name as last element of path for paranoid test helper.");
    let name_as_vec = vec![item_name.to_string()];

    let generated_id = NodeId::generate_synthetic(
        target_data.crate_namespace,
        &target_file_path,
        &name_as_vec,
        args.ident,
        item_kind,
        Some(parent_module.id.base_tid()),
        cfgs.as_deref(),
    );

    let pid = match args.item_kind {
        ItemKind::Function => FunctionNodeId::new_test(generated_id).into(),
        ItemKind::Struct => StructNodeId::new_test(generated_id).into(),
        ItemKind::Enum => EnumNodeId::new_test(generated_id).into(),
        ItemKind::Union => UnionNodeId::new_test(generated_id).into(),
        ItemKind::TypeAlias => TypeAliasNodeId::new_test(generated_id).into(),
        ItemKind::Trait => TraitNodeId::new_test(generated_id).into(),
        ItemKind::Impl => ImplNodeId::new_test(generated_id).into(),
        ItemKind::Module => ModuleNodeId::new_test(generated_id).into(),
        ItemKind::Const => ConstNodeId::new_test(generated_id).into(),
        ItemKind::Static => StaticNodeId::new_test(generated_id).into(),
        ItemKind::Macro => MacroNodeId::new_test(generated_id).into(),
        ItemKind::Import => ImportNodeId::new_test(generated_id).into(),
        // TODO: Decide what to do about handling ExternCrate. We kind of do want everything to
        // have a NodeId of some kind, and this will do for now, but we also want to
        // distinguish between an ExternCrate statement and something else... probably.
        ItemKind::ExternCrate => ImportNodeId::new_test(generated_id).into(),
        _ => panic!("You can't use this test helper on Secondary/Assoc nodes, at least not yet."),
    };
    Ok(pid)
}

fn strs_to_strings(strs: &[&str]) -> Vec<String> {
    strs.iter().copied().map(String::from).collect()
}
