use std::path::Path;

use colored::Colorize;
// Removed unused: use ploke_core::NodeId;
use syn_parser::parser::{graph::GraphAccess as _, ParsedCodeGraph};

use crate::common::{debug_printers::print_module_tree, uuid_ids_utils::run_phases_and_collect};

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_mod_paths() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_name = "file_dir_detection";

    let results = run_phases_and_collect(fixture_name);
    let merged = ParsedCodeGraph::merge_new(results).expect("Failed to merge graphs");
    let _tree = merged
        .build_module_tree() // dirty, placeholder
        .expect("Failed to build module tree for edge cases fixture");

    let (_module_tree, pruned_items) = merged.build_tree_and_prune()?;

    println!("File paths in merged modules:");
    let base_path: &Path = Path::new(
        "/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/file_dir_detection",
    );

    let root_modules = merged.graph.modules.iter().filter(|m| m.is_file_based());
    println!("{}", "Module Tree:".bold().underline());
    for module in root_modules {
        print_module_tree(&merged, module.id, "", false, base_path)?;
    }

    merged.find_mods_by_path_iter(&["crate".to_string()])?;

    Ok(())
}

#[test]
#[cfg(not(feature = "type_bearing_ids"))]
fn test_import_merging() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_name = "fixture_nodes";
    let results = run_phases_and_collect(fixture_name);

    let merged = ParsedCodeGraph::merge_new(results)?;

    println!("File paths in merged modules:");
    let base_path: &Path =
        Path::new("/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/fixture_nodes");

    let root_modules = merged.modules().iter().filter(|m| m.is_file_based());
    println!("{}", "Module Tree:".bold().underline());
    for module in root_modules {
        print_module_tree(&merged, module.id, "", false, base_path)?;
    }

    Ok(())
}
