use crate::{
    schema::{
        primary_nodes::EnumNodeSchema,
        secondary_nodes::{FieldNodeSchema, VariantNodeSchema},
    },
    traits::CommonFields,
};

use super::{secondary_nodes::process_fields, *};

pub(super) fn transform_structs(
    db: &Db<MemStorage>,
    structs: Vec<EnumNode>,
) -> Result<(), cozo::Error> {
    for enm in structs.into_iter() {
        let enm_any_id = enm.any_id();
        // let schema = &FUNCTION_NODE_SCHEMA;
        let schema = &EnumNodeSchema::SCHEMA;
        let enm_params = enm.cozo_btree();

        let script = schema.script_put(&enm_params);
        db.run_script(&script, enm_params, ScriptMutability::Mutable)?;

        let variant_schema = &VariantNodeSchema::SCHEMA;
        let field_schema = &FieldNodeSchema::SCHEMA;
        // Add enum variants
        for (i, variant) in enm.variants.into_iter().enumerate() {
            for (i, field) in variant.fields.iter().enumerate() {
                let field_params = process_fields(variant.id.as_any(), field_schema, i, field);
                let script = field_schema.script_put(&field_params);

                db.run_script(&script, field_params, ScriptMutability::Mutable)?;
            }
            let variant_params = process_variants(enm_any_id, variant_schema, i, variant);
            let script = variant_schema.script_put(&variant_params);

            db.run_script(&script, variant_params, ScriptMutability::Mutable)?;
        }

        // Add generic parameters
        for (i, generic_param) in enm.generic_params.into_iter().enumerate() {
            let (params, script) = process_generic_params(enm_any_id, i as i64, generic_param);
            db.run_script(&script, params, ScriptMutability::Mutable)?;
        }

        // Add attributes
        let attr_schema = AttributeNodeSchema::SCHEMA;
        for (i, attr) in enm.attributes.iter().enumerate() {
            let attr_params = process_attributes(enm.id.as_any(), i, attr);

            let script = attr_schema.script_put(&attr_params);
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
mod tests {}
