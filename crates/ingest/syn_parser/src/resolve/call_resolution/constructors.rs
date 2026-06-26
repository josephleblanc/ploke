use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{EnumNodeId, OrdinaryTypeTargetId, PathCallNode, StructNodeId},
    },
};

use super::{CallRelationResolver, ConstructorPathResolution, LocalTypeResolution};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_constructor_path(
        &self,
        call: &PathCallNode,
    ) -> Result<Option<ConstructorPathResolution>, SynParserError> {
        match call.path.as_slice() {
            [struct_name] => {
                let resolution = match self.resolve_local_type_segment(call.owner, struct_name)? {
                    LocalTypeResolution::Resolved(target) => {
                        self.resolve_tuple_struct_constructor(target, call.arg_count)?
                    }
                    LocalTypeResolution::Unresolved => return Ok(None),
                    LocalTypeResolution::Ambiguous => ConstructorPathResolution::Ambiguous,
                };
                Ok(Some(resolution))
            }
            [enum_name, variant_name] => {
                let resolution = match self.resolve_local_type_segment(call.owner, enum_name)? {
                    LocalTypeResolution::Resolved(target) => {
                        self.resolve_enum_variant_constructor(target, variant_name, call.arg_count)?
                    }
                    LocalTypeResolution::Unresolved => return Ok(None),
                    LocalTypeResolution::Ambiguous => ConstructorPathResolution::Ambiguous,
                };
                Ok(Some(resolution))
            }
            _ => Ok(None),
        }
    }

    fn resolve_tuple_struct_constructor(
        &self,
        target: OrdinaryTypeTargetId,
        arg_count: usize,
    ) -> Result<ConstructorPathResolution, SynParserError> {
        let Ok(struct_id) = StructNodeId::try_from(target) else {
            return Ok(ConstructorPathResolution::Unresolved);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;
        if struct_node.fields.is_empty()
            || !struct_node.fields.iter().all(|field| {
                field.name.as_deref().is_some_and(|name| {
                    name.starts_with(&format!("unnamed_field{}", struct_node.name))
                })
            })
            || struct_node.fields.len() != arg_count
        {
            return Ok(ConstructorPathResolution::Unresolved);
        }
        Ok(ConstructorPathResolution::TupleStruct(struct_id))
    }

    fn resolve_enum_variant_constructor(
        &self,
        target: OrdinaryTypeTargetId,
        variant_name: &str,
        arg_count: usize,
    ) -> Result<ConstructorPathResolution, SynParserError> {
        let Ok(enum_id) = EnumNodeId::try_from(target) else {
            return Ok(ConstructorPathResolution::Unresolved);
        };
        let enum_node = self.graph.get_enum_checked(enum_id)?;
        let mut candidates = enum_node
            .variants
            .iter()
            .filter(|variant| {
                variant.name == variant_name
                    && !variant.fields.is_empty()
                    && variant.fields.iter().all(|field| field.name.is_none())
                    && variant.fields.len() == arg_count
            })
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => ConstructorPathResolution::EnumVariant(*target),
            [] => ConstructorPathResolution::Unresolved,
            _ => ConstructorPathResolution::Ambiguous,
        })
    }
}
