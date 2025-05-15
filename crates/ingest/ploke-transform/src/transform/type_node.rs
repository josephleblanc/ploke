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
            TypeKind::Reference { lifetime, is_mutable  } => {
                BTreeMap::from([])
            }
            TypeKind::Array { size  } => {
                BTreeMap::from([])
            }
            TypeKind::RawPointer { is_mutable  } => {
                BTreeMap::from([])
            }
            TypeKind::TraitObject { dyn_token  } => {
                BTreeMap::from([])
            }
            TypeKind::Macro { name, tokens  } => {
                BTreeMap::from([])
            }
            TypeKind::Unknown { type_str  } => {
                BTreeMap::from([])
            }
            TypeKind::Function { is_unsafe, is_extern, abi  } => {
                BTreeMap::from([])
            }
            TypeKind::Tuple {  } => {
                BTreeMap::from([])
            }
            TypeKind::Never => {
                BTreeMap::from([])
            }
            TypeKind::Inferred => {
                BTreeMap::from([])
            }
            TypeKind::Paren {  } => {
                BTreeMap::from([])
            }
            TypeKind::Slice {  } => {
                BTreeMap::from([])
            }
            TypeKind::ImplTrait {  } => {
                BTreeMap::from([])
            }
        }
        // AI: fill out the other match arms to provide the completed `BTreeMap` for the other
        // variants AI!
    }
    todo!()
}
