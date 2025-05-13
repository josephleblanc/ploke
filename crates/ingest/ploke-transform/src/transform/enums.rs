use crate::{
    schema::{
        primary_nodes::EnumNodeSchema,
        secondary_nodes::{FieldNodeSchema, VariantNodeSchema},
    },
    traits::CommonFields,
};

use super::{secondary_nodes::process_fields, *};

pub(super) fn transform_enums(
    db: &Db<MemStorage>,
    structs: Vec<EnumNode>,
) -> Result<(), cozo::Error> {
    for enm in structs.into_iter() {
        let enm_any_id = enm.any_id();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &EnumNodeSchema::SCHEMA;
        let mut enm_params = enm.cozo_btree();

        let variant_schema = &VariantNodeSchema::SCHEMA;
        let field_schema = &FieldNodeSchema::SCHEMA;

        // Add enum variants
        // using vec for now, switch to iterator later.
        let mut variant_ids: Vec<DataValue> = Vec::new();
        for (i, variant) in enm.variants.into_iter().enumerate() {
            variant_ids.push(variant.id.as_any().to_cozo_uuid());

            for (i, field) in variant.fields.iter().enumerate() {
                let field_params = process_fields(variant.id.as_any(), field_schema, i, field);
                let script = field_schema.script_put(&field_params);

                log::trace!(
                    "  {} {} {:?}",
                    "field put:".log_step(),
                    script,
                    field_params
                );
                db.run_script(&script, field_params, ScriptMutability::Mutable)?;
            }
            let variant_params = process_variants(enm_any_id, variant_schema, i, variant);
            let script = variant_schema.script_put(&variant_params);

            log::trace!(
                "  {} {} {:?}",
                "variant put:".log_step(),
                script,
                variant_params
            );
            db.run_script(&script, variant_params, ScriptMutability::Mutable)?;
        }

        enm_params.insert(schema.variants().to_string(), DataValue::List(variant_ids));
        let script = schema.script_put(&enm_params);
        log::trace!("  {} {}", "enum put:".log_step(), script);
        db.run_script(&script, enm_params, ScriptMutability::Mutable)?;

        // Add generic parameters
        for (i, generic_param) in enm.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(enm_any_id, i as i64, generic_param);
            log::trace!(
                "  {} {} {:?}",
                "generic_param put:".log_step(),
                script,
                params
            );
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in enm.attributes.iter().enumerate() {
            let attr_params = process_attributes(enm.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
            log::trace!("  {} {} {:?}", "attr put:".log_step(), script, attr_params);
            db.run_script(&script, attr_params, ScriptMutability::Mutable)?;
        }
    }

    Ok(())
}

fn process_variants(
    enm: AnyNodeId,
    schema: &VariantNodeSchema,
    i: usize,
    variant: VariantNode,
) -> BTreeMap<String, DataValue> {
    let variant_cozo_id: DataValue = variant.id.as_any().to_cozo_uuid();

    let cozo_cfgs: Vec<DataValue> = variant
        .cfgs
        .iter()
        .map(|s| DataValue::from(s.as_str()))
        .collect();

    // TODO: Change discriminant type to Int
    let cozo_disc: DataValue = variant
        .discriminant
        .map_or(DataValue::Null, |d| DataValue::Str(d.into()));

    let variant_params: BTreeMap<String, DataValue> = BTreeMap::from([
        (schema.id().to_string(), variant_cozo_id),
        (schema.name().to_string(), variant.name.into()),
        (schema.owner_id().to_string(), enm.to_cozo_uuid()),
        (schema.discriminant().to_string(), cozo_disc),
        (schema.cfgs().to_string(), DataValue::List(cozo_cfgs)),
        (
            schema.index().to_string(),
            DataValue::Num(Num::Int(i as i64)),
        ),
    ]);
    variant_params
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        schema::primary_nodes::EnumNodeSchema,
        test_utils::{create_variant_schema, log_db_result},
        transform::enums::transform_enums,
    };
    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::{
        nodes::{EnumNode, TypeDefNode},
        ParsedCodeGraph,
    };

    use crate::test_utils::{create_attribute_schema, create_field_schema, create_generic_schema};
    #[test]
    fn test_transform_enums() -> Result<(), Box<dyn std::error::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
        // let tree = merged.build_module_tree().unwrap_or_else(|e| {
        //     log::error!(target: "transform_function",
        //         "Error building tree: {}",
        //         e
        //     );
        //     panic!()
        // });

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        pub(crate) fn create_enum_schema(
            db: &Db<MemStorage>,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let enum_schema = EnumNodeSchema::SCHEMA;
            let script_create = enum_schema.script_create();
            enum_schema.log_create_script();
            let db_result = db.run_script(
                &script_create,
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )?;
            log_db_result(db_result);
            Ok(())
        }
        // create and insert enum schema
        create_enum_schema(&db)?;
        // create and insert enum schema
        create_variant_schema(&db)?;
        // create and insert attribute schema
        create_attribute_schema(&db)?;
        // create and insert generic schema
        create_generic_schema(&db)?;
        // create and insert field schema
        create_field_schema(&db)?;

        let mut enum_nodes: Vec<EnumNode> = Vec::new();
        for type_def_node in merged.graph.defined_types.into_iter() {
            if let TypeDefNode::Enum(enm) = type_def_node {
                // log::info!(target: "db",
                //     "{} {}",
                //     "Processing Node:".log_step(),
                //     enm.name.log_name(),
                // );
                // let enm_params = enm.cozo_btree();
                //
                // let script = enum_schema.script_put(&enm_params);
                // db.run_script(&script, enm_params, ScriptMutability::Mutable)
                //     .inspect_err(|_| {
                //         log::error!(target: "db",
                //             "{} {}\n{:#?}\n{} {:#?}\n{} {:#?}",
                //             "Error inserting enum".log_error(),
                //             enm.name.log_name(),
                //             enm,
                //             "Enum Variants:".log_orange(),
                //             enm.variants,
                //             "Variant structs:".log_foreground_primary(),
                //             enm.variants.iter().flat_map(|v| &v.fields).collect::<Vec<_>>()
                //         );
                //     })?;
                enum_nodes.push(enm);
            }
        }
        transform_enums(&db, enum_nodes)?;
        // transform_structs(&db, );
        Ok(())
    }
}
