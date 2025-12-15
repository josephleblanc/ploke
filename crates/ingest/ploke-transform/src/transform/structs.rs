use crate::schema::primary_nodes_multi::StructNodeSchema;

use crate::{macro_traits::CommonFields, schema::secondary_nodes::FieldNodeSchema};

use super::{secondary_nodes::process_fields, *};

pub(super) fn transform_structs(
    db: &Db<MemStorage>,
    structs: Vec<StructNode>,
) -> Result<(), TransformError> {
    for strukt in structs.into_iter() {
        let struct_any_id = strukt.any_id();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &StructNodeSchema::SCHEMA;
        let strukt_params = strukt.cozo_btree();

        let script = schema.script_put(&strukt_params);
        db.run_script(&script, strukt_params, ScriptMutability::Mutable)?;

        let field_schema = &FieldNodeSchema::SCHEMA;
        // Add function parameters
        for (i, field) in strukt.fields.iter().enumerate() {
            let field_params = process_fields(struct_any_id, field_schema, i, field);
            let script = field_schema.script_put(&field_params);

            db.run_script(&script, field_params, ScriptMutability::Mutable)?;
        }

        // Add generic parameters
        for (i, generic_param) in strukt.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(struct_any_id, i as i64, generic_param);
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in strukt.attributes.iter().enumerate() {
            let attr_params = process_attributes(strukt.id.as_any(), i, attr);

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
        nodes::{StructNode, TypeDefNode},
        ParsedCodeGraph,
    };

    use crate::{
        error::TransformError,
        schema::{
            create_and_insert_generic_schema,
            primary_nodes::StructNodeSchema,
            secondary_nodes::{AttributeNodeSchema, FieldNodeSchema},
        },
    };

    use super::transform_structs;

    #[test]
    fn test_transform_structs() -> Result<(), Box<TransformError>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = test_run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        // create and insert struct schema
        StructNodeSchema::create_and_insert_schema(&db)?;
        // create and insert attribute schema
        AttributeNodeSchema::create_and_insert_schema(&db)?;
        // create and insert generic schema (wants special handler for three-part split of gener
        // types)
        create_and_insert_generic_schema(&db)?;
        // create and insert field schema
        let field_schema = FieldNodeSchema::SCHEMA;
        field_schema.create_and_insert(&db)?;

        let mut struct_nodes: Vec<StructNode> = Vec::new();
        for struct_node in merged.graph.defined_types.into_iter() {
            if let TypeDefNode::Struct(strukt) = struct_node {
                struct_nodes.push(strukt);
            }
        }
        transform_structs(&db, struct_nodes)?;
        Ok(())
    }
}
