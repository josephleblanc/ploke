use ploke_core::TypeKind;

use crate::schema::types::*;

use super::*;

pub(super) fn transform_types(
    db: &Db<MemStorage>,
    type_nodes: Vec<TypeNode>,
) -> Result<(), cozo::Error> {
    for type_node in type_nodes {
        let cozo_type_id = type_node.id.to_cozo_uuid();
        let (script, params) = match &type_node.kind {
            TypeKind::Named {
                path,
                is_fully_qualified,
            } => {
                let schema = NamedTypeSchema::SCHEMA;
                let cozo_path =
                    DataValue::List(path.iter().map(|s| DataValue::Str(s.into())).collect());

                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.path().to_string(), cozo_path),
                    (
                        schema.is_fully_qualified().to_string(),
                        DataValue::Bool(*is_fully_qualified),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Reference {
                lifetime,
                is_mutable,
            } => {
                let schema = ReferenceTypeSchema::SCHEMA;
                let cozo_lifetime = lifetime.as_ref().map(|s| DataValue::Str(s.into()));
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.lifetime().to_string(),
                        cozo_lifetime.unwrap_or(DataValue::Null),
                    ),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(*is_mutable),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Slice {} => {
                let schema = SliceTypeSchema::SCHEMA;
                let cozo_element_type = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.element_type().to_string(), cozo_element_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Array { size } => {
                let schema = ArrayTypeSchema::SCHEMA;
                let cozo_size = size.as_ref().map_or(DataValue::Null, |s| {
                    s.parse::<i64>()
                        .map_or(DataValue::Null, |i| DataValue::Num(Num::Int(i)))
                }); // Size may or may not exist.
                let cozo_element_type = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.size().to_string(), cozo_size),
                    (schema.element_type().to_string(), cozo_element_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Tuple {} => {
                let schema = TupleTypeSchema::SCHEMA;
                let cozo_element_type = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.type_id().to_string(), cozo_element_type),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Function {
                is_unsafe,
                is_extern,
                abi,
            } => {
                let schema = FunctionTypeSchema::SCHEMA;
                let cozo_abi = abi.as_ref().map(|s| DataValue::Str(s.into()));
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.is_unsafe().to_string(), DataValue::Bool(*is_unsafe)),
                    (schema.is_extern().to_string(), DataValue::Bool(*is_extern)),
                    (
                        schema.abi().to_string(),
                        cozo_abi.unwrap_or(DataValue::Null),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Never => {
                let schema = NeverTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Inferred => {
                let schema = InferredTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::RawPointer { is_mutable } => {
                let schema = RawPointerTypeSchema::SCHEMA;
                let cozo_points_to = process_element_type(&type_node);
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.is_mutable().to_string(),
                        DataValue::Bool(*is_mutable),
                    ),
                    (schema.points_to().to_string(), cozo_points_to),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::TraitObject { dyn_token } => {
                let schema = TraitObjectTypeSchema::SCHEMA;
                let cozo_trait_bounds = DataValue::List(
                    type_node
                        .related_types
                        .iter()
                        .map(|t| t.to_cozo_uuid())
                        .collect(),
                );
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.dyn_token().to_string(), DataValue::Bool(*dyn_token)),
                    (schema.trait_bounds().to_string(), cozo_trait_bounds),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Macro { name, tokens } => {
                let schema = MacroTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.name().to_string(), DataValue::Str(name.into())),
                    (schema.tokens().to_string(), DataValue::Str(tokens.into())),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Unknown { type_str } => {
                let schema = UnknownTypeSchema::SCHEMA;
                let params = BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (
                        schema.type_str().to_string(),
                        DataValue::Str(type_str.into()),
                    ),
                ]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::Paren {} => {
                let schema = ParenTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
            TypeKind::ImplTrait {} => {
                let schema = ImplTraitTypeSchema::SCHEMA;
                let params = BTreeMap::from([(schema.type_id().to_string(), cozo_type_id)]);
                let script = schema.script_put(&params);

                (script, params)
            }
        };

        db.run_script(&script, params, ScriptMutability::Mutable)?;
    }
    Ok(())
}

fn process_element_type(type_node: &TypeNode) -> DataValue {
    let cozo_element_type = type_node
        .related_types
        .first()
        .expect("Invariant Violated: All slices must have an inner type.")
        .to_cozo_uuid();
    cozo_element_type
}

#[cfg(test)]
mod tests {

    use std::collections::BTreeMap;

    use cozo::{Db, MemStorage};
    use ploke_test_utils::run_phases_and_collect;
    use syn_parser::parser::ParsedCodeGraph;

    use crate::{
        schema::{primary_nodes::ConstNodeSchema, types::create_and_insert_types},
        test_utils::{create_attribute_schema, log_db_result},
    };

    use super::{transform_consts, transform_types};
    #[test]
    fn test_transform_types() -> Result<(), Box<cozo::Error>> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();

        // Setup printable nodes
        let successful_graphs = run_phases_and_collect("fixture_nodes");
        let merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");

        let db = Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        create_and_insert_types(&db)?;

        // transform and insert impls into cozo
        transform_types(&db, merged.graph.type_graph)?;

        Ok(())
    }
}
