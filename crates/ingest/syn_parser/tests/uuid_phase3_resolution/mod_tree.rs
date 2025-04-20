use std::path::Path;

use colored::Colorize;
use ploke_core::NodeId;
use syn_parser::{
    parser::{
        nodes::{GraphNode, ImportKind, ModuleDef},
        types::VisibilityKind,
    },
    CodeGraph,
};

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

    println!("File paths in merged modules:");
    let base_path: &Path = Path::new(
        "/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/file_dir_detection",
    );

    let root_modules = merged.modules.iter().filter(|m| m.is_file_based());
    println!("{}", "Module Tree:".bold().underline());
    for module in root_modules {
        print_module_tree(&merged, module.id, "", false, base_path)?;
    }

    Ok(())
}
