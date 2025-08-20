use itertools::Itertools;
use syn_parser::GraphAccess;
use syn_parser::parser::nodes::AsAnyNodeId;
use syn_parser::{
    ModuleTree,
    parser::nodes::{AnyNodeId, GraphNode, ModuleNodeId},
};

pub fn print_module_set(
    merged: &syn_parser::ParsedCodeGraph,
    tree: &ModuleTree,
    module_set: &std::collections::HashSet<ModuleNodeId>,
) {
    let item_map_printable = module_set
        .iter()
        .filter_map(|id| {
            tree.modules()
                .get(id)
                .filter(|m| m.items().is_some())
                .map(|m| {
                    let module = format!(
                        "name: {} | is_file: {} | id: {}",
                        m.name,
                        m.id.as_any(),
                        m.is_file_based()
                    );
                    let items = m
                        .items()
                        .unwrap()
                        .iter()
                        .filter_map(|item_id| {
                            merged
                                .find_any_node(item_id.as_any())
                                .map(|n| format!("\tname: {} | id: {}", n.name(), n.any_id()))
                        })
                        .join("\n");
                    format!("{}\n{}", module, items)
                })
        })
        .join("\n");
    tracing::info!("--- items by module ---\n{}", item_map_printable);
}

pub fn printable_nodes<'a>(
    merged: &syn_parser::ParsedCodeGraph,
    union: impl Iterator<Item = &'a AnyNodeId>,
) -> String {
    let mut printable_union_items = String::new();
    for id in union.into_iter() {
        if let Some(node) = merged.find_any_node(*id) {
            let printable_node = format!("name: {} | id: {}\n", node.name(), id);
            printable_union_items.push_str(&printable_node);
        }
    }
    printable_union_items
}
