use ploke_core::NodeId;
use syn_parser::parser::{graph::CodeGraph, nodes::GraphNode};

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
            import.path == import_path
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
    use syn_parser::parser::relations::{GraphId, RelationKind};

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
