#[cfg(not(feature = "multi_embedding_schema"))]
use crate::schema::primary_nodes::TypeAliasNodeSchema;
#[cfg(feature = "multi_embedding_schema")]
use crate::schema::primary_nodes_multi::TypeAliasNodeSchema;

use crate::macro_traits::CommonFields;

use super::*;

pub(super) fn transform_type_aliases(
    db: &Db<MemStorage>,
    type_aliases: Vec<TypeAliasNode>,
) -> Result<(), TransformError> {
    for type_alias in type_aliases.into_iter() {
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &TypeAliasNodeSchema::SCHEMA;
        let mut type_alias_params = type_alias.cozo_btree();

        let cozo_ty_id = type_alias.type_id.to_cozo_uuid();

        type_alias_params.insert(schema.ty_id().to_string(), cozo_ty_id);

        let script = schema.script_put(&type_alias_params);
        db.run_script(&script, type_alias_params, ScriptMutability::Mutable)?;

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in type_alias.attributes.iter().enumerate() {
            let attr_params = process_attributes(type_alias.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::{
        nodes::{TypeAliasNode, TypeDefNode},
        ParsedCodeGraph,
    };

    use crate::schema::{primary_nodes::TypeAliasNodeSchema, secondary_nodes::AttributeNodeSchema};

    use super::transform_type_aliases;
    #[test]
    fn test_transform_type_aliases() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        TypeAliasNodeSchema::create_and_insert_schema(&db)?;

        // transform and insert impls into cozo
        let graph = merged.graph;
        let type_alias_nodes: Vec<TypeAliasNode> = graph
            .defined_types
            .into_iter()
            .filter_map(|node| match node {
                TypeDefNode::TypeAlias(node) => Some(node),
                _ => None,
            })
            .collect();
        transform_type_aliases(&db, type_alias_nodes)?;

        Ok(())
    }
}
