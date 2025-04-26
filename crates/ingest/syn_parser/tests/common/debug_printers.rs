use std::path::Path;

use colored::Colorize;
use ploke_core::NodeId;
use syn_parser::parser::{graph::{CodeGraph, GraphAccess as _}, nodes::{GraphNode, ImportKind, ModuleDef}, types::VisibilityKind, ParsedCodeGraph};

pub fn find_import_id(
    graph: &CodeGraph,
    module_path: &[String], // e.g., ["crate", "outer", "inner"]
    visible_name: &str,
    import_path: &[&str],
) -> Option<NodeId> {
    eprintln!("{:=^80}", "starting search in find_import id");
    eprintln!("parameters:\n\tgraph: CodeGraph (no good way to print)");
    eprintln!("\tmodule_path: {:?}", module_path);
    eprintln!("\tvisible_name: {:?}", visible_name);
    eprintln!("\timport_path: {:?}", import_path);
    let parent_module = graph.modules.iter().find(|m| {
        // #[cfg(feature = "verbose_debug")]
        eprintln!(
            "0. SEARCHING MODULE PATH: {:?}\nm.defn_path() = {:?}\nm.path = {:?}
m.name = {}\nm.id = {}\nm.is_file_based() = {}\n",
            module_path,
            m.defn_path(),
            m.path,
            m.name,
            m.id,
            m.is_file_based(),
        );
        (m.defn_path() == module_path && m.is_inline())
            || (m.defn_path() == module_path && m.is_file_based())
    })?;
    #[allow(clippy::suspicious_map)]
    let import_id = graph
        .use_statements
        .iter()
        .find(|import| {
            eprintln!(
                "1. SEARCHING_USE_NAME: original name: {:?}, visible_name: {:?}",
                import.original_name, &import.visible_name
            );
            import.source_path == import_path
                && import
                    .original_name
                    .clone()
                    .or_else(|| {
                        eprintln!("2. ORIGINAL_NAME : {:?}", import.original_name);
                        Some(import.visible_name.clone())
                    })
                    .map(|import_name| {
                        eprintln!("2. ORIGINAL_NAME : {:?}", import.original_name);
                        eprintln!(
                            "3. VISIBLE_NAME: \n\tvisible_name: {},\n\tsearching for import_name: {}, \n\timport.visible_name: {}",
                             visible_name, import_name, import.visible_name,
                        );
                        import_name == visible_name
                    }).is_some()
                && parent_module.items().is_some_and(|items| {
                    eprintln!(
                        "4. SEARCHING PARENT ITEMS parent_module:{}",
                        parent_module.name()
                    );
                    let count = items
                        .iter()
                        .inspect(|&item| {
                            eprintln!("\t{}", item);
                        })
                        .count();
                    eprintln!("\t5. COUNT: {}", count);

                    eprintln!("6. SEARCHING PARENT ITEMS import_id: {}", import.id);
                    items.contains(&import.id)
                })
        })
        .map(|imp| imp.id);
    eprintln!("7. SEARCHING_USE_NAME: import_id {:?}\n", import_id);
    import_id
}

#[cfg(feature = "verbose_debug")]
#[allow(clippy::too_many_arguments)]
pub fn debug_print_static_info(
    graph: &CodeGraph,
    crate_namespace: uuid::Uuid,
    file_path: &std::path::PathBuf,
    node: &syn_parser::parser::nodes::ValueNode,
    type_node: &syn_parser::parser::types::TypeNode,
    type_kind: ploke_core::TypeKind,
    related_ids: &[ploke_core::TypeId],
    expected_type_id: ploke_core::TypeId,
) {
    use syn_parser::parser::{nodes::GraphId, relations::RelationKind};

    eprintln!(
        "DEBUGGING TYPENODE: type_node = find_type_node(graph, node.type_id):\n{:#?}

let expected_type_id = TypeId::generate_synthetic(
    crate_namespace,    {}
    file_path,          {:?}
    &type_kind,         {:?}
    related_ids,        {:?}
    Some(node.id()),    {:?}
); --> {}

            ",
        type_node,
        crate_namespace,
        file_path,
        &type_kind,
        related_ids,
        Some(node.id()), // Use the node's own ID as parent scope
        expected_type_id
    );
    let value_type_rel = graph
        .relations
        .iter()
        .find(|r| r.target == GraphId::Type(node.type_id) && r.kind == RelationKind::ValueType)
        .expect("Expected RelationKind::ValueType to exist with for target, none found.");
    let debug_value_node = graph
        .values
        .iter()
        .find(|v| GraphId::Node(v.id) == value_type_rel.source)
        .expect("Expected RelationKind::ValueType to exist with for source, none found.");
    let debug_value_module = graph
        .modules
        .iter()
        .find(|m| m.items().is_some_and(|i| i.contains(&debug_value_node.id)));
    eprintln!(
        "DEBUGGING VALUE NODE Found valid relation between ValueNode source: {} and TypeNode target {}
VALUENODE: {:#?}
TYPENODE: {:#?}
MODULE CONTAINING VALUENODE: {:#?}
---
",
        debug_value_node.id, node.type_id,
        debug_value_node, node, debug_value_module
    );
}


pub fn print_module_tree(
    graph: &ParsedCodeGraph,
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
            ImportKind::ExternCrate => { print!("extern crate ") },
            ImportKind::UseStatement(vis_kind) => {
                match vis_kind {
                    VisibilityKind::Inherited => { print!("imports ") },
                    _ => { print!("re-exports {} ", vis_kind) }
                }
            },
        }
        print!("{}", import.source_path.join("::"));
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
