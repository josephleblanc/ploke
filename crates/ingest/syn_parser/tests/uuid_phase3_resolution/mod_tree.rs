use std::path::Path;

use colored::Colorize;
// Removed unused: use ploke_core::NodeId;
use syn_parser::CodeGraph;

use crate::common::{debug_printers::print_module_tree, uuid_ids_utils::run_phases_and_collect};

#[test]
fn test_mod_paths() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_name = "file_dir_detection";
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();

    for parsed in results {
        graphs.push(parsed.graph);
    }

    let merged = CodeGraph::merge_new(graphs)?;
    // Prefix with underscore to silence warning, as it's only used for its side effects (building)
    let _module_tree = merged.build_module_tree()?;

    println!("File paths in merged modules:");
    let base_path: &Path = Path::new(
        "/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/file_dir_detection",
    );

    let root_modules = merged.modules.iter().filter(|m| m.is_file_based());
    println!("{}", "Module Tree:".bold().underline());
    for module in root_modules {
        print_module_tree(&merged, module.id, "", false, base_path)?;
    }

    merged.find_module_by_path_checked(&["crate".to_string()])?;

    Ok(())
}

#[test]
fn test_import_merging() -> Result<(), Box<dyn std::error::Error>> {
    let fixture_name = "fixture_nodes";
    let results = run_phases_and_collect(fixture_name);
    let mut graphs: Vec<CodeGraph> = Vec::new();

    for parsed in results {
        graphs.push(parsed.graph);
    }

    let merged = CodeGraph::merge_new(graphs)?;

    println!("File paths in merged modules:");
    let base_path: &Path =
        Path::new("/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/fixture_nodes");

    let root_modules = merged.modules.iter().filter(|m| m.is_file_based());
    println!("{}", "Module Tree:".bold().underline());
    for module in root_modules {
        print_module_tree(&merged, module.id, "", false, base_path)?;
    }

    Ok(())
}
