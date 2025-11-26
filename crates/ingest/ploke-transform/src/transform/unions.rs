#[cfg(not(feature = "multi_embedding_schema"))]
use crate::schema::primary_nodes::UnionNodeSchema;
#[cfg(feature = "multi_embedding_schema")]
use crate::schema::primary_nodes_multi::UnionNodeSchema;

use crate::{macro_traits::CommonFields, schema::secondary_nodes::FieldNodeSchema};

use super::{secondary_nodes::process_fields, *};

pub(super) fn transform_unions(
    db: &Db<MemStorage>,
    unions: Vec<UnionNode>,
) -> Result<(), TransformError> {
    // union->onion (rust keywords)
    for onion in unions.into_iter() {
        let union_any_id = onion.any_id();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &UnionNodeSchema::SCHEMA;
        let onion_params = onion.cozo_btree();

        let script = schema.script_put(&onion_params);
        db.run_script(&script, onion_params, ScriptMutability::Mutable)?;

        let field_schema = &FieldNodeSchema::SCHEMA;
        // Add function parameters
        for (i, field) in onion.fields.iter().enumerate() {
            let field_params = process_fields(union_any_id, field_schema, i, field);
            let script = field_schema.script_put(&field_params);

            db.run_script(&script, field_params, ScriptMutability::Mutable)?;
        }

        // Add generic parameters
        for (i, generic_param) in onion.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(union_any_id, i as i64, generic_param);
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in onion.attributes.iter().enumerate() {
            let attr_params = process_attributes(onion.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use cozo::{Db, MemStorage};
    use ploke_test_utils::test_run_phases_and_collect;
    use syn_parser::parser::{
        nodes::{TypeDefNode, UnionNode},
        ParsedCodeGraph,
    };

    use crate::schema::{
        create_and_insert_generic_schema,
        primary_nodes::UnionNodeSchema,
        secondary_nodes::{AttributeNodeSchema, FieldNodeSchema},
    };

    use super::transform_unions;

    #[test]
    fn test_transform_unions() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert union schema
        UnionNodeSchema::create_and_insert_schema(&db)?;
        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        // create and insert generic schema
        create_and_insert_generic_schema(&db)?;
        // create and insert field schema
        FieldNodeSchema::create_and_insert_schema(&db)?;

        let mut union_nodes: Vec<UnionNode> = Vec::new();
        for union_node in merged.graph.defined_types.into_iter() {
            if let TypeDefNode::Union(onion) = union_node {
                union_nodes.push(onion);
            }
        }
        transform_unions(&db, union_nodes)?;
        Ok(())
    }
}
