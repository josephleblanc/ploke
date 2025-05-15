use super::*;

pub(super) fn transform_unions(
    db: &Db<MemStorage>,
    type_nodes: Vec<TypeNode>,
) -> Result<(), cozo::Error> {
    for type_node in type_nodes {
        match &type_node.kind {
            ploke_core::TypeKind::Named { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Reference { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Slice { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Array { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Tuple { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Never => {
                todo!()
            }
            ploke_core::TypeKind::Inferred => {
                todo!()
            }
            ploke_core::TypeKind::RawPointer { .. } => {
                todo!()
            }
            ploke_core::TypeKind::ImplTrait { .. } => {
                todo!()
            }
            ploke_core::TypeKind::TraitObject { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Macro { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Unknown { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Function { .. } => {
                todo!()
            }
            ploke_core::TypeKind::Paren { .. } => {
                todo!()
            }
        }
    }
    todo!()
}
