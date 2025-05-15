use ploke_core::TypeKind;

use crate::schema::types::*;

use super::*;

pub(super) fn transform_unions(
    db: &Db<MemStorage>,
    type_nodes: Vec<TypeNode>,
) -> Result<(), cozo::Error> {
    for type_node in type_nodes {
        let cozo_type_id = type_node.id.to_cozo_uuid();
        let params = match &type_node.kind {
            // AI: You can already see the completed example of what we want in the
            // `TypeKind::Named`
            TypeKind::Named { path, is_fully_qualified  } => {
                let schema = NamedTypeSchema::SCHEMA;
                let cozo_path = DataValue::List( path.into_iter().map(|s|  DataValue::Str(s.into()) ).collect() );

                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.path().to_string(), cozo_path),
                    (schema.is_fully_qualified().to_string(), DataValue::Bool(*is_fully_qualified))
                ])
            }
            TypeKind::Reference { lifetime, is_mutable } => {
                let schema = ReferenceTypeSchema::SCHEMA;
                let cozo_lifetime = lifetime.as_ref().map(|s| DataValue::Str(s.into()));
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.lifetime().to_string(), cozo_lifetime.unwrap_or(DataValue::Null)),
                    (schema.is_mutable().to_string(), DataValue::Bool(*is_mutable)),
                ])
            }
            TypeKind::Array { size } => {
                let schema = ArrayTypeSchema::SCHEMA;
                let cozo_size = size.as_ref().map(|s| DataValue::Int(*s as i64));
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.size().to_string(), cozo_size.unwrap_or(DataValue::Null)),
                ])
            }
            TypeKind::RawPointer { is_mutable } => {
                let schema = RawPointerTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.is_mutable().to_string(), DataValue::Bool(*is_mutable)),
                ])
            }
            TypeKind::TraitObject { dyn_token } => {
                let schema = TraitObjectTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.dyn_token().to_string(), DataValue::Bool(*dyn_token)),
                ])
            }
            TypeKind::Macro { name, tokens } => {
                let schema = MacroTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.name().to_string(), DataValue::Str(name.into())),
                    (schema.tokens().to_string(), DataValue::Str(tokens.into())),
                ])
            }
            TypeKind::Unknown { type_str } => {
                let schema = UnknownTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.type_str().to_string(), DataValue::Str(type_str.into())),
                ])
            }
            TypeKind::Function { is_unsafe, is_extern, abi } => {
                let schema = FunctionTypeSchema::SCHEMA;
                let cozo_abi = abi.as_ref().map(|s| DataValue::Str(s.into()));
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                    (schema.is_unsafe().to_string(), DataValue::Bool(*is_unsafe)),
                    (schema.is_extern().to_string(), DataValue::Bool(*is_extern)),
                    (schema.abi().to_string(), cozo_abi.unwrap_or(DataValue::Null)),
                ])
            }
            TypeKind::Tuple { } => {
                let schema = TupleTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
            TypeKind::Never => {
                let schema = NeverTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
            TypeKind::Inferred => {
                let schema = InferredTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
            TypeKind::Paren { } => {
                let schema = ParenTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
            TypeKind::Slice { } => {
                let schema = SliceTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
            TypeKind::ImplTrait { } => {
                let schema = ImplTraitTypeSchema::SCHEMA;
                BTreeMap::from([
                    (schema.type_id().to_string(), cozo_type_id),
                ])
            }
        }
        // AI: fill out the other match arms to provide the completed `BTreeMap` for the other
        // variants AI!
    }
    todo!()
}
