use std::path::Path;

use colored::Colorize;
use ploke_core::NodeId;
use syn_parser::{
    parser::{nodes::{GraphNode, ImportKind, ModuleDef}, types::VisibilityKind},
    CodeGraph,
};

use crate::common::uuid_ids_utils::run_phases_and_collect;

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

fn print_module_tree(
    graph: &CodeGraph,
    module_id: NodeId,
    prefix: &str,
    is_last: bool,
    base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let module = graph.get_module(module_id).ok_or("Module not found")?;

    // Print current module with tree characters
    print!("{}{}", prefix, if is_last { "└── " } else { "├── " });
    print!("{}", module.name());

    // Elegant path formatting
    let path_display_matched = match &module.module_def {
        ModuleDef::FileBased { file_path, .. } => {
            let rel_path = file_path
                .as_path()
                .strip_prefix(base_path)
                .unwrap_or(file_path.as_path())
                .display()
                .to_string()
                .replace('\\', "/"); // Normalize path separators

            if rel_path.contains('/') {
                let parts: Vec<&str> = rel_path.split('/').collect();
                let dir = parts[..parts.len() - 1].join("/");
                let file = parts.last().unwrap();
                format!("{} › {}", dir.dimmed(), file)
            } else {
                rel_path
            }
        }
        ModuleDef::Inline { 
            ..
            // items, span 
        } => {
            let vis_printable = format!("{}", module.visibility);
            format!("{} {}", "inline".dimmed(), vis_printable.dimmed())
        }
        ModuleDef::Declaration {
            ..
            // declaration_span,
            // resolved_definition,
        } => {
            let vis_printable = format!("{}", module.visibility);
            format!("{} {}", "decl".dimmed(), vis_printable.dimmed())
        }
    };
    print!(" {}", "•".dimmed());
    println!(" {}", path_display_matched);

    // -- print imports -- 
    for import in &module.imports {
        print!("{}{}", prefix, if is_last { "       " } else { "│      " });
        print!(" -> ");
        match &import.kind {
            ImportKind::ImportNode => { print!("quoia? ") },
            ImportKind::ExternCrate => { print!("extern crate ") },
            ImportKind::UseStatement(vis_kind) => {
                match vis_kind {
                    VisibilityKind::Inherited => { print!("imports ") },
                    _ => { print!("re-exports {} ", vis_kind) }
                }
            },
        }
        print!("{}", import.path.join("::"));
        if import.original_name.is_some() {
        print!(" as {}", import.visible_name)
        } else if import.is_glob { print!("::*") }
        println!();
    }
    

    // Prepare new prefix for children
    let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

    // Print child modules (filtering out non-module items)
    if let Some(item_ids) = module.items() {
        // First collect all child modules
        let child_modules: Vec<_> = item_ids
            .iter()
            .filter_map(|&id| graph.get_module(id))
            .collect();

        // Then print them with proper tree structure
        let count = child_modules.len();
        for (i, child) in child_modules.iter().enumerate() {
            print_module_tree(graph, child.id, &new_prefix, i == count - 1, base_path)?;
        }
    }

    Ok(())
}
