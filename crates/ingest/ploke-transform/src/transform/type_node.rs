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
            // AI: Change the following to also be todo! instead of str
            ploke_core::TypeKind::Slice { .. } => "Slice",
            ploke_core::TypeKind::Array { .. } => "Array",
            ploke_core::TypeKind::Tuple { .. } => "Tuple",
            ploke_core::TypeKind::Never => "Never",
            ploke_core::TypeKind::Inferred => "Inferred",
            ploke_core::TypeKind::RawPointer { .. } => "RawPointer",
            ploke_core::TypeKind::ImplTrait { .. } => "ImplTrait",
            ploke_core::TypeKind::TraitObject { .. } => "TraitObject",
            ploke_core::TypeKind::Macro { .. } => "Macro",
            ploke_core::TypeKind::Unknown { .. } => "Unknown",
            ploke_core::TypeKind::Function { .. } => "Function",
            ploke_core::TypeKind::Paren { .. } => "Paren",
            // AI!
        }
    }
    todo!()
}
